use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{config::Config, tools};

const INSTRUCTIONS: &str = "You are a concise terminal assistant. Use the available tools when they help answer the request. Report command and file results accurately; never claim a tool succeeded when it failed.";

pub struct Agent {
    client: Client,
    config: Config,
    context: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(default)]
    output: Vec<Value>,
    status: Option<String>,
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

impl Agent {
    pub fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(180))
            .user_agent(concat!("embedai/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to create HTTP client")?;
        Ok(Self {
            client,
            config,
            context: Vec::new(),
        })
    }

    pub fn clear(&mut self) {
        self.context.clear();
    }

    pub fn ask(&mut self, line: &str) -> Result<String> {
        self.context.push(json!({"role": "user", "content": line}));

        for _ in 0..self.config.max_turns {
            let response = self.call_model()?;
            if let Some(error) = response.error {
                bail!("API error: {}", error.message);
            }

            let calls: Vec<Value> = response
                .output
                .iter()
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
                .cloned()
                .collect();
            let answer = response_text(&response.output);

            // Replaying response items (including reasoning and function calls) preserves
            // the server-generated call IDs needed by subsequent Responses API calls.
            self.context.extend(response.output);

            if calls.is_empty() {
                if !answer.is_empty() {
                    return Ok(answer);
                }
                bail!(
                    "model returned no text or tool calls (response status: {})",
                    response.status.as_deref().unwrap_or("unknown")
                );
            }

            for call in calls {
                let call_id = call
                    .get("call_id")
                    .and_then(Value::as_str)
                    .context("tool call is missing call_id")?;
                let name = call
                    .get("name")
                    .and_then(Value::as_str)
                    .context("tool call is missing name")?;
                let arguments = call
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}");

                eprintln!("  tool: {name} {arguments}");
                let output = match tools::execute(name, arguments) {
                    Ok(output) => output,
                    Err(error) => format!("error: {error:#}"),
                };
                self.context.push(json!({
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": output,
                }));
            }
        }

        bail!(
            "agent exceeded {} model turns; use /clear to reset the conversation",
            self.config.max_turns
        )
    }

    fn call_model(&self) -> Result<Response> {
        let request = json!({
            "model": self.config.model,
            "instructions": INSTRUCTIONS,
            "input": self.context,
            "tools": tools::definitions(),
            "tool_choice": "auto",
            "store": false,
            "include": ["reasoning.encrypted_content"],
        });
        let response = self
            .client
            .post(self.config.responses_url())
            .bearer_auth(&self.config.api_key)
            .json(&request)
            .send()
            .context("could not reach the Responses API")?;
        let status = response.status();
        let body = response.text().context("could not read API response")?;

        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|body| body.pointer("/error/message")?.as_str().map(str::to_owned))
                .unwrap_or(body);
            bail!("API returned HTTP {status}: {message}");
        }

        serde_json::from_str(&body).context("could not decode Responses API response")
    }
}

fn response_text(output: &[Value]) -> String {
    let mut text = String::new();
    for item in output {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        for content in item
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if content.get("type").and_then(Value::as_str) == Some("output_text")
                && let Some(value) = content.get("text").and_then(Value::as_str)
            {
                text.push_str(value);
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
    };

    use serde_json::{Value, json};

    use super::{Agent, response_text};
    use crate::config::Config;

    #[test]
    fn extracts_only_output_text() {
        let output = vec![
            json!({"type": "reasoning"}),
            json!({"type": "message", "content": [
                {"type": "output_text", "text": "hello"},
                {"type": "refusal", "refusal": "no"}
            ]}),
        ];
        assert_eq!(response_text(&output), "hello");
    }

    #[test]
    fn executes_tools_and_replays_their_output() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let responses = [
                json!({"status": "completed", "output": [{
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "shell",
                    "arguments": "{\"command\":\"printf mock-ok\"}"
                }]}),
                json!({"status": "completed", "output": [{
                    "type": "message",
                    "content": [{"type": "output_text", "text": "done"}]
                }]}),
            ];
            let mut requests: Vec<Value> = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                requests.push(serde_json::from_str(&request).unwrap());
                let body = response.to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
            requests
        });

        let config = Config {
            api_key: "test-key".into(),
            model: "test-model".into(),
            base_url: format!("http://{address}"),
            max_turns: 4,
        };
        let mut agent = Agent::new(config).unwrap();
        assert_eq!(agent.ask("test the tool loop").unwrap(), "done");

        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["input"][0]["content"], "test the tool loop");
        let tool_output = requests[1]["input"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["type"] == "function_call_output")
            .unwrap();
        assert_eq!(tool_output["call_id"], "call_1");
        assert!(tool_output["output"].as_str().unwrap().contains("mock-ok"));
    }

    fn read_request(stream: &mut std::net::TcpStream) -> String {
        let mut reader = BufReader::new(stream);
        let mut content_length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = value.trim().parse().unwrap();
            }
        }
        let mut body = vec![0; content_length];
        reader.read_exact(&mut body).unwrap();
        String::from_utf8(body).unwrap()
    }
}
