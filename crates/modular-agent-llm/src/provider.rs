use modular_agent_core::AgentError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAI,
    Ollama,
}

#[derive(Debug)]
pub struct ModelIdentifier {
    pub provider: ProviderKind,
    pub model_name: String,
}

impl ModelIdentifier {
    /// Parse model string with optional provider prefix.
    ///
    /// # Examples
    /// - `"ollama/llama3.2:1b"` → (Ollama, "llama3.2:1b")
    /// - `"openai/gpt-5"` → (OpenAI, "gpt-5")
    /// - `"gpt-5"` → (OpenAI, "gpt-5") (backward compatibility)
    pub fn parse(model_str: &str) -> Result<Self, AgentError> {
        let model_str = model_str.trim();

        if model_str.is_empty() {
            return Err(AgentError::InvalidConfig(
                "Model name cannot be empty".into(),
            ));
        }

        // Ollama prefix
        if let Some(model_name) = model_str.strip_prefix("ollama/") {
            #[cfg(not(feature = "ollama"))]
            return Err(AgentError::InvalidConfig(
                "Ollama provider not available. Enable 'ollama' feature.".into(),
            ));

            if model_name.is_empty() {
                return Err(AgentError::InvalidConfig(
                    "Model name after 'ollama/' prefix cannot be empty".into(),
                ));
            }

            #[cfg(feature = "ollama")]
            return Ok(Self {
                provider: ProviderKind::Ollama,
                model_name: model_name.to_string(),
            });
        }

        // OpenAI prefix
        if let Some(model_name) = model_str.strip_prefix("openai/") {
            #[cfg(not(feature = "openai"))]
            return Err(AgentError::InvalidConfig(
                "OpenAI provider not available. Enable 'openai' feature.".into(),
            ));

            if model_name.is_empty() {
                return Err(AgentError::InvalidConfig(
                    "Model name after 'openai/' prefix cannot be empty".into(),
                ));
            }

            #[cfg(feature = "openai")]
            return Ok(Self {
                provider: ProviderKind::OpenAI,
                model_name: model_name.to_string(),
            });
        }

        // Unknown prefix check
        if let Some(slash_pos) = model_str.find('/') {
            let prefix = &model_str[..slash_pos];
            // Only error if it looks like a provider prefix (no dots, reasonable length)
            if prefix.len() < 20 && !prefix.contains('.') {
                return Err(AgentError::InvalidConfig(format!(
                    "Unknown provider '{}'. Use 'openai/' or 'ollama/' prefix.",
                    prefix
                )));
            }
        }

        // Default: OpenAI (backward compatibility)
        #[cfg(not(feature = "openai"))]
        return Err(AgentError::InvalidConfig(
            "OpenAI provider not available. Specify 'ollama/' prefix.".into(),
        ));

        #[cfg(feature = "openai")]
        Ok(Self {
            provider: ProviderKind::OpenAI,
            model_name: model_str.to_string(),
        })
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

    #[cfg(feature = "openai")]
    #[test]
    fn test_parse_default_to_openai() {
        let result = ModelIdentifier::parse("gpt-5-nano").unwrap();
        assert_eq!(result.provider, ProviderKind::OpenAI);
        assert_eq!(result.model_name, "gpt-5-nano");
    }

    #[test]
    fn test_parse_unknown_prefix() {
        let result = ModelIdentifier::parse("unknown/model");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown provider"));
    }

    #[cfg(feature = "openai")]
    #[test]
    fn test_parse_url_like_model() {
        // Model names that look like URLs should not be treated as unknown providers
        let result = ModelIdentifier::parse("my.company.com/model-v1");
        assert!(result.is_ok());
    }

    #[cfg(feature = "openai")]
    #[test]
    fn test_parse_preserves_whitespace_in_model() {
        let result = ModelIdentifier::parse("  gpt-5  ").unwrap();
        assert_eq!(result.model_name, "gpt-5");
    }
}
