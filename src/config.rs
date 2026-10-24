use anyhow::{Context, Result, bail};

const EMBEDDED_API_KEY: Option<&str> = option_env!("OPENAI_API_KEY");
const EMBEDDED_MODEL: Option<&str> = option_env!("OPENAI_MODEL");
const EMBEDDED_BASE_URL: Option<&str> = option_env!("OPENAI_BASE_URL");

#[derive(Clone, Debug)]
pub struct Config {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub max_turns: usize,
}

impl Config {
    pub fn responses_url(&self) -> String {
        if self.base_url.ends_with("/responses") {
            self.base_url.clone()
        } else {
            format!("{}/responses", self.base_url)
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| embedded(EMBEDDED_API_KEY).map(str::to_owned))
            .context("no API key: set OPENAI_API_KEY or provide it when compiling the binary")?;

        let model = std::env::var("OPENAI_MODEL")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| embedded(EMBEDDED_MODEL).map(str::to_owned))
            .unwrap_or_else(|| "gpt-5-mini".to_owned());

        let base_url = std::env::var("OPENAI_BASE_URL")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| embedded(EMBEDDED_BASE_URL).map(str::to_owned))
            .unwrap_or_else(|| "https://api.openai.com/v1".to_owned());

        let max_turns = match std::env::var("EMBEDAI_MAX_TURNS") {
            Ok(value) => {
                let turns = value
                    .parse::<usize>()
                    .with_context(|| format!("invalid EMBEDAI_MAX_TURNS: {value}"))?;
                if turns == 0 || turns > 100 {
                    bail!("EMBEDAI_MAX_TURNS must be between 1 and 100");
                }
                turns
            }
            Err(std::env::VarError::NotPresent) => 24,
            Err(error) => return Err(error).context("invalid EMBEDAI_MAX_TURNS"),
        };

        Ok(Self {
            api_key,
            model,
            base_url: base_url.trim_end_matches('/').to_owned(),
            max_turns,
        })
    }
}

fn embedded(value: Option<&'static str>) -> Option<&'static str> {
    value.filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{Config, embedded};

    #[test]
    fn empty_embedded_values_are_absent() {
        assert_eq!(embedded(None), None);
        assert_eq!(embedded(Some("")), None);
        assert_eq!(embedded(Some("secret")), Some("secret"));
    }

    #[test]
    fn accepts_base_and_full_responses_urls() {
        let mut config = Config {
            api_key: "test".into(),
            model: "test".into(),
            base_url: "http://localhost:8080/v1".into(),
            max_turns: 1,
        };
        assert_eq!(config.responses_url(), "http://localhost:8080/v1/responses");
        config.base_url = "http://localhost:8080/v1/responses".into();
        assert_eq!(config.responses_url(), "http://localhost:8080/v1/responses");
    }
}
