use std::{
    fs,
    io::Read,
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use wait_timeout::ChildExt;

const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 30_000;
const MAX_COMMAND_TIMEOUT_MS: u64 = 120_000;

#[derive(Deserialize)]
struct ShellArgs {
    command: String,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Deserialize)]
struct WriteArgs {
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct PingArgs {
    host: String,
    count: Option<u8>,
}

pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": "shell",
            "description": "Run a shell command on this device and return its stdout, stderr, and exit status.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The command to execute."},
                    "timeout_ms": {"type": "integer", "minimum": 1, "maximum": MAX_COMMAND_TIMEOUT_MS}
                },
                "required": ["command"],
                "additionalProperties": false
            }
        }),
        json!({
            "type": "function",
            "name": "read_file",
            "description": "Read a UTF-8 text file from this device. Large output is truncated.",
            "parameters": {
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
                "additionalProperties": false
            }
        }),
        json!({
            "type": "function",
            "name": "write_file",
            "description": "Write text to a file, replacing its existing contents.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }
        }),
        json!({
            "type": "function",
            "name": "list_dir",
            "description": "List entries in a directory on this device.",
            "parameters": {
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
                "additionalProperties": false
            }
        }),
        json!({
            "type": "function",
            "name": "ping",
            "description": "Ping a hostname or IP address using the device's ping command.",
            "parameters": {
                "type": "object",
                "properties": {
                    "host": {"type": "string"},
                    "count": {"type": "integer", "minimum": 1, "maximum": 10}
                },
                "required": ["host"],
                "additionalProperties": false
            }
        }),
    ]
}

pub fn execute(name: &str, arguments: &str) -> Result<String> {
    let arguments: Value = serde_json::from_str(arguments)
        .with_context(|| format!("model supplied invalid JSON arguments: {arguments}"))?;
    match name {
        "shell" => {
            let args: ShellArgs = serde_json::from_value(arguments)?;
            run_shell(&args)
        }
        "read_file" => {
            let args: PathArgs = serde_json::from_value(arguments)?;
            read_file(&args.path)
        }
        "write_file" => {
            let args: WriteArgs = serde_json::from_value(arguments)?;
            fs::write(&args.path, args.content)
                .with_context(|| format!("could not write {}", args.path))?;
            Ok(format!("wrote {}\n", args.path))
        }
        "list_dir" => {
            let args: PathArgs = serde_json::from_value(arguments)?;
            list_dir(&args.path)
        }
        "ping" => {
            let args: PingArgs = serde_json::from_value(arguments)?;
            ping(&args)
        }
        _ => bail!("unknown tool: {name}"),
    }
}

fn read_file(path: &str) -> Result<String> {
    let file = fs::File::open(path).with_context(|| format!("could not open {path}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("could not read {path}"))?;
    let truncated = bytes.len() as u64 > MAX_OUTPUT_BYTES;
    bytes.truncate(MAX_OUTPUT_BYTES as usize);
    let mut output = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        output.push_str("\n[output truncated]");
    }
    Ok(output)
}

fn list_dir(path: &str) -> Result<String> {
    let mut entries = fs::read_dir(path)
        .with_context(|| format!("could not list {path}"))?
        .map(|entry| {
            let entry = entry?;
            let kind = if entry.file_type()?.is_dir() {
                "dir "
            } else {
                "file"
            };
            Ok(format!("{kind}  {}", entry.file_name().to_string_lossy()))
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    let total = entries.len();
    entries.truncate(500);
    let mut output = entries.join("\n");
    if total > entries.len() {
        output.push_str(&format!("\n[{} more entries]", total - entries.len()));
    }
    if output.is_empty() {
        output.push_str("[empty directory]");
    }
    output.push('\n');
    Ok(output)
}

fn ping(args: &PingArgs) -> Result<String> {
    if args.host.is_empty() || args.host.starts_with('-') || args.host.contains(char::is_whitespace)
    {
        bail!("host must be a non-empty hostname or IP address without whitespace");
    }
    let count = args.count.unwrap_or(4).clamp(1, 10).to_string();
    let mut command = Command::new("ping");
    #[cfg(not(target_os = "windows"))]
    command.args(["-c", &count]);
    #[cfg(target_os = "windows")]
    command.args(["-n", &count]);
    command.arg(&args.host);
    run_command(command, DEFAULT_COMMAND_TIMEOUT_MS)
}

fn run_shell(args: &ShellArgs) -> Result<String> {
    if args.command.trim().is_empty() {
        bail!("command cannot be empty");
    }
    let timeout = args
        .timeout_ms
        .unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS)
        .clamp(1, MAX_COMMAND_TIMEOUT_MS);
    run_command(shell_command(&args.command), timeout)
}

#[cfg(not(target_os = "windows"))]
fn shell_command(command: &str) -> Command {
    let mut process = Command::new("sh");
    process.args(["-c", command]);
    process
}

#[cfg(target_os = "windows")]
fn shell_command(command: &str) -> Command {
    let mut process = Command::new("cmd");
    process.args(["/C", command]);
    process
}

fn run_command(mut command: Command, timeout_ms: u64) -> Result<String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().context("could not start command")?;
    let stdout = child.stdout.take().context("missing stdout pipe")?;
    let stderr = child.stderr.take().context("missing stderr pipe")?;
    let stdout_reader = read_limited(stdout);
    let stderr_reader = read_limited(stderr);

    let timed_out = match child.wait_timeout(Duration::from_millis(timeout_ms))? {
        Some(status) => {
            let mut output = command_output(status.code(), stdout_reader, stderr_reader)?;
            if !status.success() {
                output.push_str("\n[command failed]");
            }
            return Ok(truncate(output));
        }
        None => true,
    };
    if timed_out {
        child.kill().context("could not stop timed-out command")?;
        child.wait()?;
    }
    let mut output = command_output(None, stdout_reader, stderr_reader)?;
    output.push_str(&format!("\n[command timed out after {timeout_ms} ms]"));
    Ok(truncate(output))
}

fn read_limited<R: Read + Send + 'static>(
    mut reader: R,
) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take(MAX_OUTPUT_BYTES + 1)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

fn command_output(
    code: Option<i32>,
    stdout: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
    stderr: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
) -> Result<String> {
    let stdout = stdout
        .join()
        .map_err(|_| anyhow::anyhow!("stdout reader panicked"))??;
    let stderr = stderr
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader panicked"))??;
    Ok(format!(
        "exit: {}\nstdout:\n{}\nstderr:\n{}",
        code.map_or_else(|| "none".to_owned(), |code| code.to_string()),
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    ))
}

fn truncate(mut output: String) -> String {
    let limit = (MAX_OUTPUT_BYTES * 2) as usize;
    if output.len() > limit {
        let mut boundary = limit;
        while !output.is_char_boundary(boundary) {
            boundary -= 1;
        }
        output.truncate(boundary);
        output.push_str("\n[output truncated]");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{execute, truncate};

    #[test]
    fn unknown_tools_return_errors() {
        assert!(execute("not_a_tool", "{}").is_err());
    }

    #[test]
    fn shell_captures_output_and_status() {
        let result = execute("shell", r#"{"command":"printf hello"}"#).unwrap();
        assert!(result.contains("exit: 0"));
        assert!(result.contains("hello"));
    }

    #[test]
    fn output_is_bounded() {
        assert!(truncate("x".repeat(200_000)).len() < 140_000);
    }
}
