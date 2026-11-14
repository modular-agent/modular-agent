//! Provider and model configuration (`models.json`).
//!
//! The format follows Pi's `models.json`: providers keyed by ID, each with its
//! connection settings and a list of models. The IDs `openai`, `claude` and
//! `ollama` are built in and always exist; any other ID defines a custom
//! provider that reaches a server through one of the built-in APIs, so
//! `lmstudio/qwen3` can be used next to `openai/gpt-5`.
//!
//! ```json
//! {
//!   "providers": {
//!     "openai": { "apiKey": "${OPENAI_API_KEY}" },
//!     "lmstudio": {
//!       "baseUrl": "http://localhost:1234/v1",
//!       "api": "openai-completions",
//!       "models": [
//!         { "id": "qwen3-30b-a3b", "reasoning": true, "input": ["text", "image"] }
//!       ]
//!     },
//!     "claude": {
//!       "apiKey": "${ANTHROPIC_API_KEY}",
//!       "models": [{ "id": "claude-sonnet-4-6", "cost": { "input": 2.5, "output": 12.5 } }]
//!     }
//!   }
//! }
//! ```
//!
//! Two files are read: `~/.modular_agent/models.json` and `models.json` in
//! the working directory (the workspace, for both the desktop app and
//! `ma --workspace`). A provider defined in the second replaces the one with
//! the same ID in the first. They are read once, on first use; edits take
//! effect after a restart. If either file is invalid, every model lookup
//! fails, rather than silently falling back to the default endpoints.
//! Comments and trailing commas are accepted (JSONC), so the template
//! `models.json.sample` can keep providers commented out.
//!
//! The environment is consulted only through `${VAR}` in `apiKey`; there is
//! no implicit fallback to `OPENAI_API_KEY` and the like.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use modular_agent_core::{Error, Result};
use serde::Deserialize;

use crate::capabilities::{ModelCostRates, ThinkingLevel};
use crate::provider::{
    DEFAULT_CLAUDE_API_BASE, DEFAULT_OLLAMA_URL, DEFAULT_OPENAI_API_BASE, ProviderKind,
};

const MODELS_FILE: &str = "models.json";
const GLOBAL_DIR: &str = ".modular_agent";

const BUILTIN_PROVIDERS: [(&str, ProviderKind); 3] = [
    ("openai", ProviderKind::OpenAI),
    ("claude", ProviderKind::Claude),
    ("ollama", ProviderKind::Ollama),
];

// ============================================================================
// File format
// ============================================================================

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelsFile {
    #[serde(default, rename = "$schema")]
    _schema: Option<serde::de::IgnoredAny>,
    #[serde(default)]
    providers: HashMap<String, FileProvider>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FileProvider {
    #[serde(default, rename = "name")]
    _name: Option<String>,
    base_url: Option<String>,
    api: Option<Api>,
    api_key: Option<String>,
    #[serde(default)]
    models: Vec<FileModel>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum Api {
    OpenaiCompletions,
    AnthropicMessages,
    Ollama,
}

impl Api {
    fn kind(self) -> ProviderKind {
        match self {
            Api::OpenaiCompletions => ProviderKind::OpenAI,
            Api::AnthropicMessages => ProviderKind::Claude,
            Api::Ollama => ProviderKind::Ollama,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FileModel {
    id: String,
    #[serde(default, rename = "name")]
    _name: Option<String>,
    reasoning: Option<bool>,
    thinking_level_map: Option<BTreeMap<ThinkingLevel, Option<String>>>,
    input: Option<Vec<InputModality>>,
    context_window: Option<u32>,
    max_tokens: Option<u32>,
    cost: Option<FileCost>,
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum InputModality {
    Text,
    Image,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FileCost {
    input: f64,
    output: f64,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
}

// ============================================================================
// Resolved configuration
// ============================================================================

/// One model entry of a provider. Every field is optional: for a model the
/// built-in table knows, only the fields set here override it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct UserModel {
    pub(crate) context_window: Option<u32>,
    pub(crate) max_tokens: Option<u32>,
    pub(crate) reasoning: Option<bool>,
    /// A string sets the provider-side parameter for that level; `None`
    /// removes the level.
    pub(crate) thinking_level_map: Option<BTreeMap<ThinkingLevel, Option<String>>>,
    pub(crate) image_input: Option<bool>,
    pub(crate) cost: Option<ModelCostRates>,
}

/// A provider as configured: built-in defaults overlaid with `models.json`.
#[derive(Clone)]
pub(crate) struct Provider {
    pub(crate) id: String,
    pub(crate) kind: ProviderKind,
    pub(crate) builtin: bool,
    base_url: Option<String>,
    /// As written, `${VAR}` unexpanded, so the environment is read when a
    /// client is built.
    api_key: Option<String>,
    pub(crate) models: HashMap<String, UserModel>,
}

// Hand-written so a literal API key never reaches a log line.
impl std::fmt::Debug for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Provider")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("base_url", &self.base_url())
            .field("api_key", &self.api_key.as_ref().map(|_| "<set>"))
            .finish_non_exhaustive()
    }
}

impl Provider {
    fn builtin(id: &str, kind: ProviderKind) -> Self {
        Self {
            id: id.to_string(),
            kind,
            builtin: true,
            base_url: None,
            api_key: None,
            models: HashMap::new(),
        }
    }

    fn from_file(id: String, file: FileProvider) -> std::result::Result<Self, String> {
        if id.is_empty() || id.contains('/') {
            return Err(format!(
                "provider ID '{id}' must be non-empty and must not contain '/'"
            ));
        }
        let builtin_kind = builtin_kind(&id);
        let kind = match (builtin_kind, file.api) {
            (Some(_), Some(_)) => {
                return Err(format!(
                    "provider '{id}': 'api' cannot be set on a built-in provider"
                ));
            }
            (Some(kind), None) => kind,
            (None, Some(api)) => api.kind(),
            (None, None) => {
                return Err(format!(
                    "provider '{id}': a custom provider needs 'api' \
                     (openai-completions, anthropic-messages or ollama)"
                ));
            }
        };
        if builtin_kind.is_none() && file.base_url.is_none() {
            return Err(format!(
                "provider '{id}': a custom provider needs 'baseUrl'"
            ));
        }
        let mut models = HashMap::new();
        for model in file.models {
            if model.id.is_empty() {
                return Err(format!("provider '{id}': model 'id' must be non-empty"));
            }
            let model_id = model.id.clone();
            if models.insert(model_id.clone(), model.into()).is_some() {
                return Err(format!(
                    "provider '{id}': model '{model_id}' is listed twice"
                ));
            }
        }
        Ok(Self {
            id,
            kind,
            builtin: builtin_kind.is_some(),
            base_url: file.base_url,
            api_key: file.api_key,
            models,
        })
    }

    /// The configured base URL, or the default endpoint of a built-in
    /// provider (custom providers always configure one).
    pub(crate) fn base_url(&self) -> &str {
        self.base_url.as_deref().unwrap_or(match self.kind {
            ProviderKind::OpenAI => DEFAULT_OPENAI_API_BASE,
            ProviderKind::Claude => DEFAULT_CLAUDE_API_BASE,
            ProviderKind::Ollama => DEFAULT_OLLAMA_URL,
        })
    }

    /// The API key with `${VAR}` expanded from the environment. `None` (no
    /// key configured, or one that expands to "") means requests go out
    /// unauthenticated.
    pub(crate) fn api_key(&self) -> Result<Option<String>> {
        self.api_key_with(&|var| std::env::var(var).ok())
    }

    fn api_key_with(&self, lookup: &impl Fn(&str) -> Option<String>) -> Result<Option<String>> {
        let Some(raw) = &self.api_key else {
            return Ok(None);
        };
        let key = modular_agent_core::expand_vars(raw, lookup).map_err(|var| {
            Error::InvalidConfig(format!(
                "models.json provider '{}': apiKey references the undefined environment \
                 variable {var}",
                self.id
            ))
        })?;
        Ok(Some(key).filter(|k| !k.is_empty()))
    }
}

impl From<FileModel> for UserModel {
    fn from(m: FileModel) -> Self {
        Self {
            context_window: m.context_window,
            max_tokens: m.max_tokens,
            reasoning: m.reasoning,
            thinking_level_map: m.thinking_level_map,
            image_input: m.input.map(|input| input.contains(&InputModality::Image)),
            cost: m.cost.map(|c| ModelCostRates {
                input: c.input,
                output: c.output,
                cache_read: c.cache_read,
                cache_write: c.cache_write,
            }),
        }
    }
}

fn builtin_kind(id: &str) -> Option<ProviderKind> {
    BUILTIN_PROVIDERS
        .iter()
        .find(|(builtin, _)| *builtin == id)
        .map(|(_, kind)| *kind)
}

/// Every provider a model string can name: the built-in ones, replaced or
/// joined by those `models.json` defines.
#[derive(Debug)]
pub(crate) struct ModelsConfig {
    providers: HashMap<String, Arc<Provider>>,
}

impl ModelsConfig {
    /// Later providers replace earlier ones with the same ID.
    fn new(overrides: impl IntoIterator<Item = Provider>) -> Self {
        let mut providers: HashMap<String, Arc<Provider>> = BUILTIN_PROVIDERS
            .iter()
            .map(|(id, kind)| (id.to_string(), Arc::new(Provider::builtin(id, *kind))))
            .collect();
        for provider in overrides {
            providers.insert(provider.id.clone(), Arc::new(provider));
        }
        Self { providers }
    }

    pub(crate) fn provider(&self, id: &str) -> Option<&Arc<Provider>> {
        self.providers.get(id)
    }

    /// The built-in provider of `kind` (`openai`, `claude` or `ollama`).
    #[cfg_attr(not(feature = "ollama"), allow(dead_code))]
    pub(crate) fn builtin(&self, kind: ProviderKind) -> &Arc<Provider> {
        let (id, _) = BUILTIN_PROVIDERS
            .iter()
            .find(|(_, k)| *k == kind)
            .expect("every ProviderKind has a built-in provider");
        // `expect`: `new` inserts every built-in ID, and a models.json entry
        // for one replaces it rather than removing it.
        self.providers
            .get(*id)
            .expect("built-in providers are always configured")
    }
}

fn parse_providers(text: &str) -> std::result::Result<Vec<Provider>, String> {
    let file: ModelsFile = serde_json::from_str(&strip_jsonc(text)).map_err(|e| e.to_string())?;
    file.providers
        .into_iter()
        .map(|(id, provider)| Provider::from_file(id, provider))
        .collect()
}

/// Turns JSONC into JSON by blanking out `//` and `/* */` comments and
/// trailing commas, so a template can keep entries commented out. Every
/// byte is replaced in place (newlines kept), so serde_json's line and
/// column still point into the file as written.
fn strip_jsonc(text: &str) -> String {
    let src = text.as_bytes();
    let mut out = src.to_vec();
    let mut in_string = false;
    // A comma that turns out to be trailing if `}` or `]` comes next.
    let mut pending_comma = None;
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        if in_string {
            match c {
                b'\\' => i += 1,
                b'"' => in_string = false,
                _ => {}
            }
            i += 1;
            continue;
        }
        match (c, src.get(i + 1)) {
            (b'/', Some(b'/')) => {
                while i < src.len() && src[i] != b'\n' {
                    out[i] = b' ';
                    i += 1;
                }
                continue;
            }
            (b'/', Some(b'*')) => {
                let start = i;
                i += 2;
                while i < src.len() && !(src[i] == b'*' && src.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i = (i + 2).min(src.len());
                for byte in &mut out[start..i] {
                    if !matches!(*byte, b'\n' | b'\r') {
                        *byte = b' ';
                    }
                }
                continue;
            }
            (b',', _) => pending_comma = Some(i),
            (b'}' | b']', _) => {
                if let Some(comma) = pending_comma.take() {
                    out[comma] = b' ';
                }
            }
            (b'"', _) => {
                in_string = true;
                pending_comma = None;
            }
            _ if c.is_ascii_whitespace() => {}
            _ => pending_comma = None,
        }
        i += 1;
    }
    // Only whole comments (all their bytes) and ASCII commas were replaced,
    // so the result is still valid UTF-8.
    String::from_utf8_lossy(&out).into_owned()
}

// ============================================================================
// Loading
// ============================================================================

/// Reads `paths` (lowest priority first) into one configuration; a missing
/// file contributes nothing, any other failure makes the whole result an
/// error.
fn load(paths: &[PathBuf]) -> std::result::Result<Arc<ModelsConfig>, String> {
    let mut providers = Vec::new();
    let mut errors = Vec::new();
    for path in paths {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                errors.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        match parse_providers(&text) {
            Ok(parsed) => {
                log::info!("Loaded model configuration from {}", path.display());
                providers.extend(parsed);
            }
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    if !errors.is_empty() {
        return Err(format!(
            "Invalid models.json; no model can be used until it is fixed and the \
             app restarted: {}",
            errors.join("; ")
        ));
    }
    Ok(Arc::new(ModelsConfig::new(providers)))
}

/// `~/.modular_agent/models.json`, then `models.json` in the working
/// directory unless that is the same file.
#[cfg_attr(test, allow(dead_code))]
fn default_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = std::env::home_dir() {
        paths.push(home.join(GLOBAL_DIR).join(MODELS_FILE));
    }
    if let Ok(local) = std::path::absolute(MODELS_FILE)
        && !paths.contains(&local)
    {
        paths.push(local);
    }
    paths
}

/// Read on first use, which comes after the host has moved into its
/// workspace, and kept for the life of the process.
#[cfg_attr(test, allow(dead_code))]
static CONFIG: std::sync::LazyLock<std::result::Result<Arc<ModelsConfig>, String>> =
    std::sync::LazyLock::new(|| {
        let config = load(&default_paths());
        if let Err(e) = &config {
            log::error!("{e}");
        }
        config
    });

/// The configuration, or the reason `models.json` cannot be used.
///
/// Tests never read the user's files: they see the built-in providers plus
/// whatever `test_support::register_test_provider` added.
pub(crate) fn current() -> Result<Arc<ModelsConfig>> {
    #[cfg(test)]
    return Ok(test_support::config());

    #[cfg(not(test))]
    CONFIG.clone().map_err(Error::InvalidConfig)
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::sync::{LazyLock, Mutex};

    static PROVIDERS: LazyLock<Mutex<Vec<Provider>>> = LazyLock::new(|| Mutex::new(Vec::new()));

    /// Parses a `models.json` text into a configuration, panicking on
    /// invalid input.
    pub(crate) fn config_from_json(json: &str) -> ModelsConfig {
        ModelsConfig::new(parse_providers(json).expect("test models.json must parse"))
    }

    /// Adds a provider to what [`current`](super::current) returns in tests.
    /// Tests run concurrently and never remove providers, so each test uses
    /// an ID of its own.
    #[cfg(feature = "ollama")]
    pub(crate) fn register_test_provider(id: &str, provider_json: &str) {
        let json = format!(r#"{{ "providers": {{ "{id}": {provider_json} }} }}"#);
        let parsed = parse_providers(&json).expect("test provider must parse");
        PROVIDERS.lock().unwrap().extend(parsed);
    }

    pub(super) fn config() -> Arc<ModelsConfig> {
        Arc::new(ModelsConfig::new(PROVIDERS.lock().unwrap().iter().cloned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_err(json: &str) -> String {
        parse_providers(json).expect_err("must be rejected")
    }

    #[test]
    fn parses_pi_style_file() {
        let config = test_support::config_from_json(
            r#"{
                "$schema": "https://example.com/models.schema.json",
                "providers": {
                    "lmstudio": {
                        "name": "LM Studio",
                        "baseUrl": "http://localhost:1234/v1",
                        "api": "openai-completions",
                        "apiKey": "lm-key",
                        "models": [{
                            "id": "qwen3-30b-a3b",
                            "name": "Qwen3 30B",
                            "reasoning": true,
                            "thinkingLevelMap": { "minimal": null, "max": "max" },
                            "input": ["text", "image"],
                            "contextWindow": 32768,
                            "maxTokens": 8192,
                            "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0 }
                        }]
                    },
                    "claude": { "models": [{ "id": "claude-sonnet-4-6", "cost": { "input": 2.5, "output": 12.5 } }] }
                }
            }"#,
        );
        let lmstudio = config.provider("lmstudio").expect("custom provider");
        assert_eq!(lmstudio.kind, ProviderKind::OpenAI);
        assert!(!lmstudio.builtin);
        assert_eq!(lmstudio.base_url(), "http://localhost:1234/v1");
        assert_eq!(lmstudio.api_key().unwrap(), Some("lm-key".to_string()));
        let model = &lmstudio.models["qwen3-30b-a3b"];
        assert_eq!(model.context_window, Some(32768));
        assert_eq!(model.max_tokens, Some(8192));
        assert_eq!(model.reasoning, Some(true));
        assert_eq!(model.image_input, Some(true));
        assert_eq!(
            model.thinking_level_map,
            Some(BTreeMap::from([
                (ThinkingLevel::Minimal, None),
                (ThinkingLevel::Max, Some("max".to_string())),
            ]))
        );
        assert_eq!(
            model.cost,
            Some(ModelCostRates {
                input: 0.0,
                output: 0.0,
                cache_read: Some(0.0),
                cache_write: Some(0.0),
            })
        );

        let claude = config.provider("claude").expect("built-in provider");
        assert!(claude.builtin);
        assert_eq!(claude.base_url(), DEFAULT_CLAUDE_API_BASE);
        assert_eq!(claude.models["claude-sonnet-4-6"].context_window, None);
        // Built-ins not mentioned in the file still exist.
        assert_eq!(
            config.provider("openai").expect("openai").base_url(),
            DEFAULT_OPENAI_API_BASE
        );
    }

    #[test]
    fn builtin_provider_takes_base_url_and_key_but_not_api() {
        let config = test_support::config_from_json(
            r#"{ "providers": { "ollama": { "baseUrl": "http://gpu-box:11434", "apiKey": "k" } } }"#,
        );
        let ollama = config.provider("ollama").unwrap();
        assert_eq!(ollama.kind, ProviderKind::Ollama);
        assert_eq!(ollama.base_url(), "http://gpu-box:11434");

        let err = parse_err(r#"{ "providers": { "openai": { "api": "openai-completions" } } }"#);
        assert!(err.contains("built-in"), "{err}");
    }

    #[test]
    fn custom_provider_needs_base_url_and_api() {
        let err = parse_err(r#"{ "providers": { "x": { "api": "ollama" } } }"#);
        assert!(err.contains("baseUrl"), "{err}");
        let err = parse_err(r#"{ "providers": { "x": { "baseUrl": "http://h" } } }"#);
        assert!(err.contains("'api'"), "{err}");
    }

    #[test]
    fn accepts_comments_and_trailing_commas() {
        let config = test_support::config_from_json(
            r#"// header
            {
              /* block
                 comment */
              "providers": {
                // "openai": { "apiKey": "x" },
                "lm": {
                  "baseUrl": "http://localhost:1234/v1", // a URL is not a comment
                  "api": "openai-completions",
                  "apiKey": "a,b/*not a comment*/\"}",
                  "models": [{ "id": "m", "input": ["text", "image",], },],
                },
              },
            }"#,
        );
        let lm = config.provider("lm").unwrap();
        assert_eq!(lm.base_url(), "http://localhost:1234/v1");
        assert_eq!(
            lm.api_key().unwrap(),
            Some(r#"a,b/*not a comment*/"}"#.to_string())
        );
        assert_eq!(lm.models["m"].image_input, Some(true));
        assert!(config.provider("openai").unwrap().models.is_empty());
    }

    #[test]
    fn errors_point_at_the_file_as_written() {
        let err = parse_err(
            "// 日本語のコメント\n{\n  /* two\n     lines */\n  \"providers\": { \"openai\": { \"baseURL\": \"x\" } }\n}",
        );
        assert!(err.contains("unknown field `baseURL`"), "{err}");
        assert!(err.contains("line 5"), "{err}");
    }

    /// The shipped template must load as is, and with every provider block
    /// uncommented one level (as an editor's toggle-comment would do).
    #[test]
    fn sample_file_parses_commented_and_uncommented() {
        let sample = include_str!("../models.json.sample");
        let config = test_support::config_from_json(sample);
        assert!(config.provider("lmstudio").is_none());

        let uncommented: String = sample
            .lines()
            .map(|line| match line.strip_prefix("    // ") {
                Some(rest) if !rest.starts_with(|c: char| c.is_alphanumeric()) => {
                    format!("    {rest}\n")
                }
                _ => format!("{line}\n"),
            })
            .collect();
        let config = test_support::config_from_json(&uncommented);
        for id in ["openai", "claude", "ollama", "lmstudio"] {
            assert!(config.provider(id).is_some(), "{id}");
        }
        assert_eq!(
            config.provider("lmstudio").unwrap().kind,
            ProviderKind::OpenAI
        );
    }

    #[test]
    fn rejects_invalid_entries() {
        for (json, needle) in [
            (
                r#"{ "providers": { "x": { "baseUrl": "http://h", "api": "google-generative-ai" } } }"#,
                "unknown variant",
            ),
            (
                r#"{ "providers": { "a/b": { "baseUrl": "http://h", "api": "ollama" } } }"#,
                "'/'",
            ),
            (
                r#"{ "providers": { "openai": { "baseURL": "http://h" } } }"#,
                "unknown field",
            ),
            (
                r#"{ "providers": { "openai": { "models": [{ "id": "m", "contextWindw": 1 }] } } }"#,
                "unknown field",
            ),
            (
                r#"{ "providers": { "openai": { "models": [{ "id": "m", "thinkingLevelMap": { "off": "none" } }] } } }"#,
                "unknown variant",
            ),
            (
                r#"{ "providers": { "openai": { "models": [{ "id": "m" }, { "id": "m" }] } } }"#,
                "twice",
            ),
            (
                r#"{ "providers": { "openai": { "models": [{ "id": "m", "input": ["audio"] }] } } }"#,
                "unknown variant",
            ),
            (r#"{ "providers": { "#, "EOF"),
        ] {
            let err = parse_err(json);
            assert!(err.contains(needle), "{json}: {err}");
        }
    }

    #[test]
    fn api_key_expands_only_explicit_variables() {
        let lookup = |var: &str| (var == "LM_KEY").then(|| "from-env".to_string());
        let with_key = |key: &str| {
            let json = format!(
                r#"{{ "providers": {{ "lm": {{ "baseUrl": "http://h", "api": "openai-completions", "apiKey": "{key}" }} }} }}"#
            );
            test_support::config_from_json(&json)
        };

        let config = with_key("${LM_KEY}");
        let provider = config.provider("lm").unwrap();
        assert_eq!(
            provider.api_key_with(&lookup).unwrap(),
            Some("from-env".to_string())
        );

        let config = with_key("${LM_MISSING}");
        let err = config
            .provider("lm")
            .unwrap()
            .api_key_with(&lookup)
            .expect_err("undefined variable");
        assert!(
            err.to_string().contains("'lm'") && err.to_string().contains("LM_MISSING"),
            "{err}"
        );

        // No apiKey: unauthenticated, whatever the environment holds.
        let config = test_support::config_from_json(
            r#"{ "providers": { "lm": { "baseUrl": "http://h", "api": "openai-completions" } } }"#,
        );
        let everything = |_: &str| Some("leaked".to_string());
        assert_eq!(
            config
                .provider("lm")
                .unwrap()
                .api_key_with(&everything)
                .unwrap(),
            None
        );
        // Built-ins too: OPENAI_API_KEY is never read implicitly.
        let config = test_support::config_from_json("{}");
        assert_eq!(
            config
                .provider("openai")
                .unwrap()
                .api_key_with(&everything)
                .unwrap(),
            None
        );
        assert_eq!(
            config.provider("openai").unwrap().base_url(),
            DEFAULT_OPENAI_API_BASE
        );
    }

    #[test]
    fn debug_hides_api_key() {
        let config = test_support::config_from_json(
            r#"{ "providers": { "openai": { "apiKey": "sk-secret" } } }"#,
        );
        let debug = format!("{:?}", config.provider("openai").unwrap());
        assert!(!debug.contains("sk-secret"), "{debug}");
    }

    // -- loading --

    fn write(path: &std::path::Path, text: &str) {
        std::fs::write(path, text).expect("write test file");
    }

    #[test]
    fn local_file_replaces_global_provider() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("global.json");
        let local = dir.path().join("local.json");
        write(
            &global,
            r#"{ "providers": {
                "lm": { "baseUrl": "http://global", "api": "openai-completions", "models": [{ "id": "a" }] },
                "other": { "baseUrl": "http://other", "api": "ollama" }
            } }"#,
        );
        write(
            &local,
            r#"{ "providers": { "lm": { "baseUrl": "http://local", "api": "ollama" } } }"#,
        );

        let config = load(&[global, local]).expect("valid files");
        let lm = config.provider("lm").unwrap();
        assert_eq!(lm.base_url(), "http://local");
        assert_eq!(lm.kind, ProviderKind::Ollama);
        // Replaced as a whole: the global model list does not carry over.
        assert!(lm.models.is_empty());
        assert_eq!(config.provider("other").unwrap().base_url(), "http://other");
    }

    #[test]
    fn missing_files_leave_only_the_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let config = load(&[dir.path().join("models.json")]).expect("no file is fine");
        assert!(config.provider("openai").is_some());
        assert!(config.provider("lm").is_none());
    }

    #[test]
    fn an_invalid_file_fails_the_whole_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("global.json");
        let local = dir.path().join("local.json");
        write(
            &global,
            r#"{ "providers": { "openai": { "baseUrl": "http://local-llm/v1" } } }"#,
        );
        write(&local, r#"{ "providers": { "openai": { "baseUrl": "#);
        let err = load(&[global.clone(), local.clone()]).expect_err("syntax error");
        assert!(err.contains("local.json"), "{err}");

        // A validation error in the other file fails just the same.
        write(&local, "{}");
        write(&global, r#"{ "providers": { "x": { "api": "ollama" } } }"#);
        let err = load(&[global, local]).expect_err("validation error");
        assert!(err.contains("global.json"), "{err}");
    }
}
