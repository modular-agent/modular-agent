use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use modular_agent_core::Error;
use modular_agent_core::Result;

use crate::models_config::{ModelsConfig, Provider};

// Default endpoints of the built-in providers
pub(crate) const DEFAULT_CLAUDE_API_BASE: &str = "https://api.anthropic.com";
pub(crate) const DEFAULT_OPENAI_API_BASE: &str = "https://api.openai.com/v1";
pub(crate) const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// The API a provider speaks; custom providers pick one with `api` in
/// `models.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAI,
    Ollama,
    Claude,
}

/// Prompt cache retention shared across providers (Claude cache_control,
/// OpenAI prompt_cache_key; no-op for Ollama).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheRetention {
    None,
    Short,
    Long,
}

impl CacheRetention {
    /// Unrecognized values fall back to the default ("short") so a typo in a
    /// patch degrades to normal caching instead of failing the request.
    pub(crate) fn parse(s: &str) -> Self {
        match s {
            "none" => Self::None,
            "long" => Self::Long,
            _ => Self::Short,
        }
    }
}

#[derive(Debug)]
pub struct ModelIdentifier {
    pub provider: ProviderKind,
    pub model_name: String,
    /// The provider the prefix named, as configured when the model string
    /// was parsed: connection settings and the user's model entries.
    pub(crate) config: Arc<Provider>,
}

impl ModelIdentifier {
    /// Parse a `provider/model` string. The provider is `openai`, `ollama`,
    /// `claude`, or one defined in `models.json`; only the first `/`
    /// separates it, so the model name may contain more.
    ///
    /// Fails for every provider while `models.json` is invalid, so a broken
    /// file never silently routes requests to the default endpoints.
    ///
    /// # Examples
    /// - `"ollama/llama3.2:1b"` → (Ollama, "llama3.2:1b")
    /// - `"openai/gpt-5"` → (OpenAI, "gpt-5")
    /// - `"openai/qwen/qwen3-vl-8b"` → (OpenAI, "qwen/qwen3-vl-8b")
    pub fn parse(model_str: &str) -> Result<Self> {
        Self::parse_with(model_str, crate::models_config::current())
    }

    pub(crate) fn parse_with(model_str: &str, config: Result<Arc<ModelsConfig>>) -> Result<Self> {
        let model_str = model_str.trim();

        if model_str.is_empty() {
            return Err(Error::InvalidConfig("Model name cannot be empty".into()));
        }

        let Some((provider_id, model_name)) = model_str.split_once('/') else {
            return Err(Error::InvalidConfig(format!(
                "Model '{model_str}' requires a provider prefix: 'openai/', 'ollama/', \
                 'claude/', or a provider defined in models.json."
            )));
        };

        let config = config?;
        let Some(provider) = config.provider(provider_id) else {
            return Err(Error::InvalidConfig(format!(
                "Unknown provider '{provider_id}'. Use 'openai/', 'ollama/', 'claude/', \
                 or define '{provider_id}' in models.json."
            )));
        };

        if model_name.is_empty() {
            return Err(Error::InvalidConfig(format!(
                "Model name after '{provider_id}/' prefix cannot be empty"
            )));
        }

        let (enabled, feature) = match provider.kind {
            ProviderKind::OpenAI => (cfg!(feature = "openai"), "openai"),
            ProviderKind::Ollama => (cfg!(feature = "ollama"), "ollama"),
            ProviderKind::Claude => (cfg!(feature = "claude"), "claude"),
        };
        if !enabled {
            return Err(Error::InvalidConfig(format!(
                "Provider '{provider_id}' needs the '{feature}' feature, which this build \
                 does not enable."
            )));
        }

        Ok(Self {
            provider: provider.kind,
            model_name: model_name.to_string(),
            config: provider.clone(),
        })
    }
}

/// Per-module cache of provider clients, one per provider ID, since a
/// module's `model` config can move between providers while it runs.
#[cfg_attr(
    not(any(feature = "openai", feature = "claude", feature = "ollama")),
    allow(dead_code)
)]
pub(crate) struct ClientCache<C> {
    clients: Mutex<HashMap<String, C>>,
}

#[cfg_attr(
    not(any(feature = "openai", feature = "claude", feature = "ollama")),
    allow(dead_code)
)]
impl<C: Clone> ClientCache<C> {
    pub(crate) fn new() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn get_or_build(
        &self,
        provider: &Provider,
        build: impl FnOnce(&Provider) -> Result<C>,
    ) -> Result<C> {
        // `.unwrap()`: poisoning needs a panic while holding the lock, which
        // should propagate.
        let mut clients = self.clients.lock().unwrap();
        if let Some(client) = clients.get(&provider.id) {
            return Ok(client.clone());
        }
        let client = build(provider)?;
        clients.insert(provider.id.clone(), client.clone());
        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_retention_parse() {
        assert_eq!(CacheRetention::parse("none"), CacheRetention::None);
        assert_eq!(CacheRetention::parse("short"), CacheRetention::Short);
        assert_eq!(CacheRetention::parse("long"), CacheRetention::Long);
        assert_eq!(CacheRetention::parse(""), CacheRetention::Short);
        assert_eq!(CacheRetention::parse("weird"), CacheRetention::Short);
    }

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
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unknown provider"), "{err}");
        assert!(err.contains("models.json"), "{err}");
    }

    #[cfg(feature = "openai")]
    fn lmstudio_config() -> Result<Arc<ModelsConfig>> {
        Ok(Arc::new(
            crate::models_config::test_support::config_from_json(
                r#"{ "providers": { "lmstudio": {
                    "baseUrl": "http://localhost:1234/v1", "api": "openai-completions"
                } } }"#,
            ),
        ))
    }

    #[cfg(feature = "openai")]
    #[test]
    fn test_parse_custom_provider() {
        let id = ModelIdentifier::parse_with("lmstudio/qwen/qwen3", lmstudio_config()).unwrap();
        assert_eq!(id.provider, ProviderKind::OpenAI);
        assert_eq!(id.model_name, "qwen/qwen3");
        assert_eq!(id.config.id, "lmstudio");
        assert_eq!(id.config.base_url(), "http://localhost:1234/v1");

        // Built-in prefixes keep working next to it.
        let id = ModelIdentifier::parse_with("openai/gpt-5", lmstudio_config()).unwrap();
        assert_eq!(id.config.id, "openai");
    }

    #[test]
    fn test_parse_fails_for_every_provider_while_models_json_is_invalid() {
        for model in ["openai/gpt-5", "claude/claude-opus-4-8", "lmstudio/qwen3"] {
            let err = ModelIdentifier::parse_with(
                model,
                Err(Error::InvalidConfig("Invalid models.json: boom".into())),
            )
            .unwrap_err();
            assert!(err.to_string().contains("boom"), "{model}: {err}");
        }
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
