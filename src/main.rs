mod agent;
mod config;
mod tools;

use std::io::{self, BufRead, Write};

use anyhow::{Context, Result, bail};

use crate::{agent::Agent, config::Config};

fn main() {
    if let Err(error) = run() {
        eprintln!("embedai: {error:#}");
        std::process::exit(1);
    }
}

#[derive(Default)]
struct Options {
    endpoint: Option<String>,
    model: Option<String>,
}

impl Options {
    fn parse() -> Result<Option<Self>> {
        Self::parse_from(std::env::args().skip(1))
    }

    fn parse_from(args: impl IntoIterator<Item = String>) -> Result<Option<Self>> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        while let Some(argument) = args.next() {
            let value = match argument.as_str() {
                "-h" | "--help" => {
                    print_help();
                    return Ok(None);
                }
                "-e" | "--endpoint" => &mut options.endpoint,
                "-m" | "--model" => &mut options.model,
                _ => bail!("unknown argument: {argument}\n\n{}", usage()),
            };
            let argument_value = args
                .next()
                .filter(|value| !value.is_empty())
                .with_context(|| format!("{argument} requires a value"))?;
            *value = Some(argument_value);
        }
        Ok(Some(options))
    }
}

fn usage() -> &'static str {
    "usage: embedai [--endpoint URL] [--model MODEL]"
}

fn print_help() {
    println!(
        "embedai {}\n{}\n\noptions:\n  -e, --endpoint URL  Responses API base URL or full /responses URL\n  -m, --model MODEL   Override the embedded/default model\n  -h, --help          Show this help",
        env!("CARGO_PKG_VERSION"),
        usage()
    );
}

fn run() -> Result<()> {
    let Some(options) = Options::parse()? else {
        return Ok(());
    };
    let mut config = Config::from_env()?;
    if let Some(endpoint) = options.endpoint {
        config.base_url = endpoint.trim_end_matches('/').to_owned();
    }
    if let Some(model) = options.model {
        config.model = model;
    }
    println!(
        "embedai {} · {} · /help for commands",
        env!("CARGO_PKG_VERSION"),
        config.model
    );
    let mut agent = Agent::new(config)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();

    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            println!();
            return Ok(());
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match line {
            "/quit" | "/exit" => return Ok(()),
            "/clear" => {
                agent.clear();
                println!("conversation cleared");
                continue;
            }
            "/help" => {
                println!(
                    "/clear  clear conversation context\n/help   show this help\n/quit   exit\n\nTools: shell, read_file, write_file, list_dir, ping"
                );
                continue;
            }
            _ => {}
        }

        match agent.ask(line) {
            Ok(answer) => println!(">>> {answer}\n"),
            Err(error) => eprintln!("error: {error:#}\n"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Options;

    #[test]
    fn parses_endpoint_and_model() {
        let options = Options::parse_from([
            "--endpoint".into(),
            "http://localhost:8080/v1".into(),
            "--model".into(),
            "local-model".into(),
        ])
        .unwrap()
        .unwrap();
        assert_eq!(
            options.endpoint.as_deref(),
            Some("http://localhost:8080/v1")
        );
        assert_eq!(options.model.as_deref(), Some("local-model"));
    }

    #[test]
    fn rejects_missing_option_values() {
        assert!(Options::parse_from(["--endpoint".into()]).is_err());
    }
}
