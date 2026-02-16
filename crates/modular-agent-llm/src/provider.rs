use modular_agent_core::AgentError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAI,
    Ollama,
    Claude,
}

#[derive(Debug)]
pub struct ModelIdentifier {
    pub provider: ProviderKind,
    pub model_name: String,
}

impl ModelIdentifier {
    /// Parse model string with provider prefix.
    ///
    /// A provider prefix is required. Supported prefixes: `openai/`, `ollama/`, `claude/`.
    ///
    /// # Examples
    /// - `"ollama/llama3.2:1b"` → (Ollama, "llama3.2:1b")
    /// - `"openai/gpt-5"` → (OpenAI, "gpt-5")
    /// - `"openai/qwen/qwen3-vl-8b"` → (OpenAI, "qwen/qwen3-vl-8b")
    pub fn parse(model_str: &str) -> Result<Self, AgentError> {
        let model_str = model_str.trim();

        if model_str.is_empty() {
            return Err(AgentError::InvalidConfig(
                "Model name cannot be empty".into(),
            ));
        }

        // Ollama prefix
        if let Some(model_name) = model_str.strip_prefix("ollama/") {
            if model_name.is_empty() {
                return Err(AgentError::InvalidConfig(
                    "Model name after 'ollama/' prefix cannot be empty".into(),
                ));
            }

            #[cfg(not(feature = "ollama"))]
            return Err(AgentError::InvalidConfig(
                "Ollama provider not available. Enable 'ollama' feature.".into(),
            ));

            #[cfg(feature = "ollama")]
            return Ok(Self {
                provider: ProviderKind::Ollama,
                model_name: model_name.to_string(),
            });
        }

        // OpenAI prefix
        if let Some(model_name) = model_str.strip_prefix("openai/") {
            if model_name.is_empty() {
                return Err(AgentError::InvalidConfig(
                    "Model name after 'openai/' prefix cannot be empty".into(),
                ));
            }

            #[cfg(not(feature = "openai"))]
            return Err(AgentError::InvalidConfig(
                "OpenAI provider not available. Enable 'openai' feature.".into(),
            ));

            #[cfg(feature = "openai")]
            return Ok(Self {
                provider: ProviderKind::OpenAI,
                model_name: model_name.to_string(),
            });
        }

        // Claude prefix
        if let Some(model_name) = model_str.strip_prefix("claude/") {
            if model_name.is_empty() {
                return Err(AgentError::InvalidConfig(
                    "Model name after 'claude/' prefix cannot be empty".into(),
                ));
            }

            #[cfg(not(feature = "claude"))]
            return Err(AgentError::InvalidConfig(
                "Claude provider not available. Enable 'claude' feature.".into(),
            ));

            #[cfg(feature = "claude")]
            return Ok(Self {
                provider: ProviderKind::Claude,
                model_name: model_name.to_string(),
            });
        }

        // Unknown provider prefix
        if let Some(slash_pos) = model_str.find('/') {
            let prefix = &model_str[..slash_pos];
            return Err(AgentError::InvalidConfig(format!(
                "Unknown provider '{}'. Use 'openai/', 'ollama/', or 'claude/' prefix.",
                prefix
            )));
        }

        // No prefix at all
        Err(AgentError::InvalidConfig(format!(
            "Model '{}' requires a provider prefix. Use 'openai/', 'ollama/', or 'claude/'.",
            model_str
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_model() {
        let result = ModelIdentifier::parse("");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Model name cannot be empty")
        );
    }

    #[test]
    fn test_parse_whitespace_only() {
        let result = ModelIdentifier::parse("   ");
        assert!(result.is_err());
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn test_parse_ollama_prefix() {
        let result = ModelIdentifier::parse("ollama/llama3.2:1b").unwrap();
        assert_eq!(result.provider, ProviderKind::Ollama);
        assert_eq!(result.model_name, "llama3.2:1b");
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn test_parse_ollama_empty_model() {
        let result = ModelIdentifier::parse("ollama/");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[cfg(feature = "openai")]
    #[test]
    fn test_parse_openai_prefix() {
        let result = ModelIdentifier::parse("openai/gpt-5-mini").unwrap();
        assert_eq!(result.provider, ProviderKind::OpenAI);
        assert_eq!(result.model_name, "gpt-5-mini");
    }

    #[cfg(feature = "openai")]
    #[test]
    fn test_parse_openai_empty_model() {
        let result = ModelIdentifier::parse("openai/");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_no_prefix_error() {
        let result = ModelIdentifier::parse("gpt-5-nano");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires a provider prefix")
        );
    }

    #[cfg(feature = "claude")]
    #[test]
    fn test_parse_claude_prefix() {
        let result = ModelIdentifier::parse("claude/claude-sonnet-4-5-20250514").unwrap();
        assert_eq!(result.provider, ProviderKind::Claude);
        assert_eq!(result.model_name, "claude-sonnet-4-5-20250514");
    }

    #[cfg(feature = "claude")]
    #[test]
    fn test_parse_claude_empty_model() {
        let result = ModelIdentifier::parse("claude/");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_parse_unknown_prefix() {
        let result = ModelIdentifier::parse("unknown/model");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown provider"));
    }

    #[test]
    fn test_parse_url_like_model_requires_prefix() {
        let result = ModelIdentifier::parse("my.company.com/model-v1");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown provider"));
    }

    #[cfg(feature = "openai")]
    #[test]
    fn test_parse_preserves_whitespace_in_model() {
        let result = ModelIdentifier::parse("  openai/gpt-5  ").unwrap();
        assert_eq!(result.provider, ProviderKind::OpenAI);
        assert_eq!(result.model_name, "gpt-5");
    }

    #[cfg(feature = "openai")]
    #[test]
    fn test_parse_openai_prefix_with_slash_in_model() {
        let result = ModelIdentifier::parse("openai/qwen/qwen3-vl-8b").unwrap();
        assert_eq!(result.provider, ProviderKind::OpenAI);
        assert_eq!(result.model_name, "qwen/qwen3-vl-8b");
    }
}
