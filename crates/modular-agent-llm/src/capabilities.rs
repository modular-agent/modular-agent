//! Model capability registry.
//!
//! Resolves per-model metadata (context window, max output tokens, thinking
//! levels, cost rates, image input) from four layers, highest priority first:
//!
//! 1. The model's entry in `models.json` (see `models_config.rs`),
//!    matched by exact ID within the provider the model string names
//! 2. Built-in static table (major official Claude / OpenAI models), for the
//!    built-in `claude` / `openai` providers only
//! 3. Probed Ollama metadata (`/api/show`: context length, vision and
//!    thinking capability, cached per process), for providers using the
//!    Ollama API
//! 4. Conservative defaults (8192 tokens)
//!
//! The built-in table matches by longest prefix, so snapshot suffixes
//! (`claude-sonnet-4-6-20260115`) and `-latest` aliases resolve to their base
//! entry. A `models.json` entry overrides only the fields it sets.

use std::sync::LazyLock;
#[cfg(feature = "ollama")]
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::models_config::UserModel;
use crate::provider::{ModelIdentifier, ProviderKind};

/// Reasoning intensity vocabulary shared across providers, in increasing
/// order (Pi's vocabulary minus "off").
///
/// "off" is represented as the *absence* of a level (`Option<ThinkingLevel>`
/// on the consumer side), not as a variant, so that
/// [`ModelCapabilities::thinking_levels`] only lists levels a model supports.
/// Consumed by the `thinking_level` module configuration and as the keys of
/// `thinkingLevelMap` in `models.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl ThinkingLevel {
    /// Parse the `thinking_level` config value. "off" and anything
    /// unrecognized both map to `None` so a typo degrades to no thinking
    /// instead of an error mid-flow.
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s {
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::XHigh),
            "max" => Some(Self::Max),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }

    /// Ordinal used for nearest-level clamping.
    fn rank(self) -> i32 {
        self as i32
    }
}

/// Clamp a requested thinking level to the nearest level the model supports,
/// returning that level's registry entry (level + provider-side parameter).
///
/// `None` requested means "off" and always yields `None`. An empty
/// `supported` list (model without thinking support) silently yields `None`
/// as well, so patches can set a thinking level once and still work on
/// non-reasoning models. Ties between an equally-distant lower and higher
/// level resolve to the lower (cheaper) one.
pub(crate) fn clamp_thinking_level(
    requested: Option<ThinkingLevel>,
    supported: &[(ThinkingLevel, Option<String>)],
) -> Option<(ThinkingLevel, Option<String>)> {
    let requested = requested?;
    supported
        .iter()
        .min_by_key(|(level, _)| {
            let distance = (level.rank() - requested.rank()).abs();
            // Secondary key makes the lower level win an equal distance.
            (distance, level.rank())
        })
        .cloned()
}

/// API cost rates in USD per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ModelCostRates {
    pub input: f64,
    pub output: f64,
    /// Rate for input tokens read from the prompt cache. `None` means the
    /// cached rate is unknown; cost consumers fall back to `input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    /// Rate for input tokens written to the prompt cache. `None` means the
    /// provider has no separate write billing (or it is unknown); cost
    /// consumers fall back to `input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
}

/// Fully resolved capabilities for one model. Every field has a value;
/// unknown models fall back to conservative defaults.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelCapabilities {
    /// Total context window in tokens (input + output).
    pub context_window: u32,
    /// Maximum output tokens per request.
    pub max_tokens: u32,
    /// Whether the model supports extended thinking / reasoning.
    pub reasoning: bool,
    /// Supported thinking levels. The `Option<String>` is the provider-side
    /// parameter value for that level (OpenAI `reasoning_effort`, Claude
    /// adaptive `effort`); `None` means there is no per-level parameter and
    /// the caller maps the level to the provider mechanism itself (Claude
    /// `budget_tokens` amounts, Ollama's boolean `think` flag).
    /// Empty means thinking is unsupported.
    pub thinking_levels: Vec<(ThinkingLevel, Option<String>)>,
    /// Cost rates in USD per Mtok; `None` when unknown.
    pub cost: Option<ModelCostRates>,
    /// Whether the model accepts image input.
    pub image_input: bool,
}

/// Capabilities merged across the registry layers, with `None` for fields no
/// layer knows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelCapabilitiesEntry {
    pub context_window: Option<u32>,
    pub max_tokens: Option<u32>,
    pub thinking_levels: Option<Vec<(ThinkingLevel, Option<String>)>>,
    pub cost: Option<ModelCostRates>,
    pub image_input: Option<bool>,
}

impl ModelCapabilitiesEntry {
    /// Overwrite self with the `Some` fields of `other`. `thinking_levels`
    /// and `cost` are replaced wholesale, not element-merged.
    fn overwrite_with(&mut self, other: &Self) {
        if other.context_window.is_some() {
            self.context_window = other.context_window;
        }
        if other.max_tokens.is_some() {
            self.max_tokens = other.max_tokens;
        }
        if other.thinking_levels.is_some() {
            self.thinking_levels = other.thinking_levels.clone();
        }
        if other.cost.is_some() {
            self.cost = other.cost;
        }
        if other.image_input.is_some() {
            self.image_input = other.image_input;
        }
    }

    /// Apply a `models.json` entry: scalar fields overwrite, `reasoning`
    /// toggles the provider's default levels, and `thinkingLevelMap` edits
    /// the resulting level list one level at a time.
    fn apply_user_model(&mut self, user: &UserModel, kind: ProviderKind) {
        self.overwrite_with(&ModelCapabilitiesEntry {
            context_window: user.context_window,
            max_tokens: user.max_tokens,
            thinking_levels: None,
            cost: user.cost,
            image_input: user.image_input,
        });

        match user.reasoning {
            Some(false) => self.thinking_levels = Some(vec![]),
            Some(true) if self.thinking_levels.as_ref().is_none_or(|l| l.is_empty()) => {
                self.thinking_levels = Some(default_thinking_levels(kind));
            }
            _ => {}
        }

        if let Some(map) = &user.thinking_level_map {
            let mut levels = self.thinking_levels.take().unwrap_or_default();
            for (level, param) in map {
                levels.retain(|(l, _)| l != level);
                if let Some(param) = param {
                    levels.push((*level, Some(param.clone())));
                }
            }
            levels.sort_by_key(|(level, _)| *level);
            self.thinking_levels = Some(levels);
        }
    }

    fn into_capabilities(self) -> ModelCapabilities {
        let thinking_levels = self.thinking_levels.unwrap_or_default();
        ModelCapabilities {
            context_window: self.context_window.unwrap_or(DEFAULT_CONTEXT_WINDOW),
            max_tokens: self.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            reasoning: !thinking_levels.is_empty(),
            thinking_levels,
            cost: self.cost,
            image_input: self.image_input.unwrap_or(false),
        }
    }
}

impl ModelCapabilities {
    fn to_entry(&self) -> ModelCapabilitiesEntry {
        ModelCapabilitiesEntry {
            context_window: Some(self.context_window),
            max_tokens: Some(self.max_tokens),
            thinking_levels: Some(self.thinking_levels.clone()),
            cost: self.cost,
            image_input: Some(self.image_input),
        }
    }
}

// Conservative defaults preserving pre-registry behavior (the historical
// 8192-token Claude hardcode).
pub(crate) const DEFAULT_MAX_TOKENS: u32 = 8192;
pub(crate) const DEFAULT_CONTEXT_WINDOW: u32 = 8192;

/// Levels a model gets from `reasoning: true` when no other layer lists any:
/// minimal through high, as in Pi (xhigh and max only when mapped
/// explicitly). OpenAI takes the level name as `reasoning_effort`; Claude
/// (budget_tokens amounts) and Ollama (boolean `think`) have no per-level
/// parameter.
fn default_thinking_levels(kind: ProviderKind) -> Vec<(ThinkingLevel, Option<String>)> {
    use ThinkingLevel::*;
    [Minimal, Low, Medium, High]
        .into_iter()
        .map(|level| {
            let param = (kind == ProviderKind::OpenAI).then(|| level.as_str().to_string());
            (level, param)
        })
        .collect()
}

// ============================================================================
// Global state
// ============================================================================

/// Metadata probed from one Ollama `/api/show` call. A `None` field means
/// the response didn't expose it (older servers).
#[cfg(feature = "ollama")]
#[derive(Debug, Clone, Copy)]
struct OllamaProbe {
    context_length: Option<u32>,
    image_input: Option<bool>,
    thinking: Option<bool>,
}

/// Probed Ollama metadata, keyed by the full `provider/model` string so two
/// providers pointing at different servers do not share entries. An entry
/// means /api/show succeeded (cached to avoid re-querying every turn, even
/// when all fields came back empty). Network errors are NOT cached so a
/// transient failure retries next turn.
#[cfg(feature = "ollama")]
static OLLAMA_PROBES: OnceLock<Mutex<std::collections::HashMap<String, OllamaProbe>>> =
    OnceLock::new();

#[cfg(feature = "ollama")]
fn get_ollama_probe_map() -> &'static Mutex<std::collections::HashMap<String, OllamaProbe>> {
    OLLAMA_PROBES.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

#[cfg(feature = "ollama")]
fn probe_key(id: &ModelIdentifier) -> String {
    format!("{}/{}", id.config.id, id.model_name)
}

// ============================================================================
// Built-in static table
// ============================================================================

struct BuiltinModel {
    provider: ProviderKind,
    prefix: &'static str,
    caps: ModelCapabilities,
}

/// All built-in models accept image input and derive `reasoning` from the
/// thinking-level list, which happens to hold for the current catalog.
fn builtin(
    provider: ProviderKind,
    prefix: &'static str,
    context_window: u32,
    max_tokens: u32,
    thinking_levels: Vec<(ThinkingLevel, Option<String>)>,
    cost: ModelCostRates,
) -> BuiltinModel {
    BuiltinModel {
        provider,
        prefix,
        caps: ModelCapabilities {
            context_window,
            max_tokens,
            reasoning: !thinking_levels.is_empty(),
            thinking_levels,
            cost: Some(cost),
            image_input: true,
        },
    }
}

/// Anthropic's published cache multipliers apply uniformly across current
/// Claude models: reads are 0.1x input, 5-minute cache writes are 1.25x.
///
/// The write rate assumes the 5-minute TTL. 1-hour cache writes (ChatModule
/// `cache_retention = "long"`) are billed at 2x, but Anthropic reports one
/// combined cache-write token count, so the rate cannot be picked per write;
/// users on 1h retention can override `cacheWrite` via models.json.
fn claude_cost(input: f64, output: f64) -> ModelCostRates {
    ModelCostRates {
        input,
        output,
        cache_read: Some(input * 0.1),
        cache_write: Some(input * 1.25),
    }
}

/// OpenAI gpt-5 family: published cached-input rate is 0.1x input; cache
/// writes are not billed separately, so `cache_write` stays `None`.
fn openai_gpt5_cost(input: f64, output: f64) -> ModelCostRates {
    ModelCostRates {
        input,
        output,
        cache_read: Some(input * 0.1),
        cache_write: None,
    }
}

/// Cost with no known cache rates (older OpenAI models: cached-input
/// multipliers vary per model, so they are left for models.json overrides).
fn plain_cost(input: f64, output: f64) -> ModelCostRates {
    ModelCostRates {
        input,
        output,
        cache_read: None,
        cache_write: None,
    }
}

/// Claude adaptive thinking with effort levels. `Minimal` is absent because
/// Claude's effort vocabulary starts at low. `xhigh` arrived with Opus 4.7,
/// so Opus 4.6 / Sonnet 4.6 go from high straight to max. `budget_tokens`
/// does not apply here: it is rejected or deprecated on adaptive-thinking
/// models. The effort string reaches the API as `output_config.effort`
/// alongside `thinking: {"type": "adaptive"}`.
fn adaptive_levels(xhigh: bool) -> Vec<(ThinkingLevel, Option<String>)> {
    use ThinkingLevel::*;
    let mut levels = vec![Low, Medium, High];
    if xhigh {
        levels.push(XHigh);
    }
    levels.push(Max);
    levels
        .into_iter()
        .map(|level| (level, Some(level.as_str().to_string())))
        .collect()
}

/// OpenAI `reasoning_effort` levels. gpt-5.1 dropped "minimal" in favor of
/// "none", so the parameter value for `Minimal` varies per model.
fn openai_effort_levels(minimal_param: &str) -> Vec<(ThinkingLevel, Option<String>)> {
    use ThinkingLevel::*;
    vec![
        (Minimal, Some(minimal_param.into())),
        (Low, Some("low".into())),
        (Medium, Some("medium".into())),
        (High, Some("high".into())),
    ]
}

// Data sources:
// - Current Claude models: claude-api skill catalog (cached 2026-06-04;
//   effort levels re-checked 2026-11-14).
// - Legacy Claude entries (claude-sonnet-4-5, claude-opus-4-5) and ALL
//   OpenAI entries: author knowledge as of 2026-01. These are reasonable
//   defaults meant to be corrected via models.json if they drift.
// Prefixes must be unique within a provider; longest prefix wins, so e.g.
// `gpt-5-mini` shadows `gpt-5` for `gpt-5-mini-*` names. `gpt-5` also
// matches `gpt-5-chat-latest` (non-reasoning); accepted as an overridable
// default. No Ollama entries: model names are arbitrary strings there, so
// the /api/show layer and models.json cover them instead.
static BUILTIN_MODELS: LazyLock<Vec<BuiltinModel>> = LazyLock::new(|| {
    use ProviderKind::{Claude, OpenAI};
    vec![
        // Claude
        builtin(
            Claude,
            "claude-fable-5",
            1_000_000,
            128_000,
            adaptive_levels(true),
            claude_cost(10.0, 50.0),
        ),
        builtin(
            Claude,
            "claude-opus-4-8",
            1_000_000,
            128_000,
            adaptive_levels(true),
            claude_cost(5.0, 25.0),
        ),
        builtin(
            Claude,
            "claude-opus-4-7",
            1_000_000,
            128_000,
            adaptive_levels(true),
            claude_cost(5.0, 25.0),
        ),
        builtin(
            Claude,
            "claude-opus-4-6",
            1_000_000,
            128_000,
            adaptive_levels(false),
            claude_cost(5.0, 25.0),
        ),
        builtin(
            Claude,
            "claude-sonnet-4-6",
            1_000_000,
            64_000,
            adaptive_levels(false),
            claude_cost(3.0, 15.0),
        ),
        builtin(
            Claude,
            "claude-haiku-4-5",
            200_000,
            64_000,
            default_thinking_levels(Claude),
            claude_cost(1.0, 5.0),
        ),
        builtin(
            Claude,
            "claude-sonnet-4-5",
            200_000,
            64_000,
            default_thinking_levels(Claude),
            claude_cost(3.0, 15.0),
        ),
        builtin(
            Claude,
            "claude-opus-4-5",
            200_000,
            64_000,
            default_thinking_levels(Claude),
            claude_cost(5.0, 25.0),
        ),
        // OpenAI
        builtin(
            OpenAI,
            "gpt-5.1",
            400_000,
            128_000,
            openai_effort_levels("none"),
            openai_gpt5_cost(1.25, 10.0),
        ),
        builtin(
            OpenAI,
            "gpt-5-mini",
            400_000,
            128_000,
            openai_effort_levels("minimal"),
            openai_gpt5_cost(0.25, 2.0),
        ),
        builtin(
            OpenAI,
            "gpt-5-nano",
            400_000,
            128_000,
            openai_effort_levels("minimal"),
            openai_gpt5_cost(0.05, 0.40),
        ),
        builtin(
            OpenAI,
            "gpt-5",
            400_000,
            128_000,
            openai_effort_levels("minimal"),
            openai_gpt5_cost(1.25, 10.0),
        ),
        builtin(
            OpenAI,
            "gpt-4.1-mini",
            1_047_576,
            32_768,
            vec![],
            plain_cost(0.40, 1.60),
        ),
        builtin(
            OpenAI,
            "gpt-4.1-nano",
            1_047_576,
            32_768,
            vec![],
            plain_cost(0.10, 0.40),
        ),
        builtin(
            OpenAI,
            "gpt-4.1",
            1_047_576,
            32_768,
            vec![],
            plain_cost(2.0, 8.0),
        ),
        builtin(
            OpenAI,
            "gpt-4o-mini",
            128_000,
            16_384,
            vec![],
            plain_cost(0.15, 0.60),
        ),
        builtin(
            OpenAI,
            "gpt-4o",
            128_000,
            16_384,
            vec![],
            plain_cost(2.50, 10.0),
        ),
    ]
});

// ============================================================================
// Resolution
// ============================================================================

/// Longest-prefix match over `(prefix, value)` candidates. Case-sensitive,
/// no normalization. The empty prefix matches every model name.
fn longest_prefix_match<'a, T>(
    model_name: &str,
    candidates: impl Iterator<Item = (&'a str, &'a T)>,
) -> Option<&'a T> {
    let mut best: Option<(&str, &T)> = None;
    for (prefix, value) in candidates {
        if model_name.starts_with(prefix) && best.is_none_or(|(bp, _)| prefix.len() > bp.len()) {
            best = Some((prefix, value));
        }
    }
    best.map(|(_, v)| v)
}

/// Merged (models.json > static > ollama) entry with `None` for unknown
/// fields. Request-building code uses this to distinguish "known limit" from
/// "unknown model" (only known models get clamped).
///
/// Locks are never held across await points, so `std::sync::Mutex` suffices.
/// `.unwrap()` on the locks: poisoning requires a prior panic while holding
/// the lock, in which case propagating the panic is the right outcome.
pub(crate) fn resolve_entry(id: &ModelIdentifier) -> ModelCapabilitiesEntry {
    let mut merged = ModelCapabilitiesEntry::default();

    // Layer 3: probed Ollama metadata (lowest of the known layers).
    // Exact key, no prefix match: the cache key is the very model that was
    // queried.
    #[cfg(feature = "ollama")]
    if id.provider == ProviderKind::Ollama
        && let Some(probe) = get_ollama_probe_map().lock().unwrap().get(&probe_key(id))
    {
        merged.context_window = probe.context_length;
        merged.image_input = probe.image_input;
        // Ollama's request-side `think` flag is boolean, so a probed
        // "thinking" capability maps to every default level with no
        // per-level parameter; "capabilities present but no thinking" is a
        // positive "unsupported" signal (empty list).
        merged.thinking_levels = probe.thinking.map(|t| {
            if t {
                default_thinking_levels(ProviderKind::Ollama)
            } else {
                vec![]
            }
        });
    }

    // Layer 2: built-in static table — official endpoints only, longest
    // prefix wins.
    if id.config.builtin
        && let Some(caps) = longest_prefix_match(
            &id.model_name,
            BUILTIN_MODELS
                .iter()
                .filter(|m| m.provider == id.provider)
                .map(|m| (m.prefix, &m.caps)),
        )
    {
        merged.overwrite_with(&caps.to_entry());
    }

    // Layer 1: models.json — exact model ID within this provider.
    if let Some(user) = id.config.models.get(&id.model_name) {
        merged.apply_user_model(user, id.provider);
    }

    merged
}

/// Resolve full capabilities for a model, falling back to defaults for any
/// field no layer knows about. Synchronous and cheap; safe to call per turn.
pub fn lookup_capabilities(id: &ModelIdentifier) -> ModelCapabilities {
    resolve_entry(id).into_capabilities()
}

/// Clamp a user-configured max_tokens to a model limit.
/// `configured <= 0` means "unset" and returns `None` (caller decides the
/// provider-specific default). Values above `u32::MAX` saturate.
/// `limit == None` (unknown model) leaves the configured value untouched so
/// users running local models they know better than the registry aren't
/// clamped down to a guessed default.
pub(crate) fn clamp_max_tokens(configured: i64, limit: Option<u32>) -> Option<u32> {
    if configured <= 0 {
        return None;
    }
    let v = u32::try_from(configured).unwrap_or(u32::MAX);
    Some(match limit {
        Some(l) => v.min(l),
        None => v,
    })
}

// ============================================================================
// Ollama /api/show probing
// ============================================================================

/// Best-effort: query Ollama `/api/show` once per model and cache the
/// context length plus vision/thinking capabilities for later
/// [`lookup_capabilities`] / [`resolve_entry`] calls. Failures are logged at
/// warn level and never fail the caller.
///
/// Called from the Ollama chat/completion paths right after client
/// acquisition, which is *after* the per-turn `resolve_entry` call: probed
/// fields only take effect from a model's second turn onward. For the image
/// demotion in `prepare_messages` this means the very first turn with an
/// unprobed non-vision model is sent as-is (best-effort, same as before the
/// probe existed); once the request path reads `context_window`, callers
/// must warm before resolving.
#[cfg(feature = "ollama")]
pub(crate) async fn warm_ollama_context(
    client: &crate::ollama_client::OllamaClient,
    id: &ModelIdentifier,
) {
    let key = probe_key(id);
    // Fast path: already queried (hit or confirmed-absent).
    if get_ollama_probe_map().lock().unwrap().contains_key(&key) {
        return;
    }
    match client.show_model_info(&id.model_name).await {
        Ok(info) => {
            let probe = OllamaProbe {
                context_length: extract_context_length(&info),
                image_input: extract_capability(&info, "vision"),
                thinking: extract_capability(&info, "thinking"),
            };
            get_ollama_probe_map().lock().unwrap().insert(key, probe);
        }
        // Transient failures are not cached; next turn retries.
        Err(e) => log::warn!(
            "Failed to probe Ollama model info for '{}': {e}",
            id.model_name
        ),
    }
}

/// `/api/show` returns `model_info` keyed by architecture, e.g.
/// `{"general.architecture": "llama", "llama.context_length": 131072}`.
#[cfg(feature = "ollama")]
fn extract_context_length(info: &serde_json::Value) -> Option<u32> {
    let mi = info.get("model_info")?.as_object()?;
    // Prefer the architecture-qualified key; fall back to any *.context_length.
    let arch = mi.get("general.architecture").and_then(|v| v.as_str());
    let by_arch = arch.and_then(|a| mi.get(&format!("{a}.context_length")));
    let v = by_arch.or_else(|| {
        mi.iter()
            .find(|(k, _)| k.ends_with(".context_length"))
            .map(|(_, v)| v)
    })?;
    u32::try_from(v.as_u64()?).ok()
}

/// `/api/show` lists model capabilities as strings, e.g.
/// `{"capabilities": ["completion", "vision", "thinking"]}`. Older Ollama
/// servers omit the field, in which case support stays unknown (`None`)
/// rather than assumed absent — image demotion must only fire on a positive
/// "no vision" signal.
#[cfg(feature = "ollama")]
fn extract_capability(info: &serde_json::Value, name: &str) -> Option<bool> {
    let caps = info.get("capabilities")?.as_array()?;
    Some(caps.iter().any(|v| v.as_str() == Some(name)))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models_config::test_support::config_from_json;

    /// A model string resolved against the built-in providers only.
    fn id(provider: ProviderKind, name: &str) -> ModelIdentifier {
        let prefix = match provider {
            ProviderKind::OpenAI => "openai",
            ProviderKind::Claude => "claude",
            ProviderKind::Ollama => "ollama",
        };
        with_json("{}", &format!("{prefix}/{name}"))
    }

    /// A model string resolved against the given `models.json` text. Built
    /// directly rather than through `parse`, whose provider-feature check
    /// would fail these tests in reduced-feature builds.
    fn with_json(json: &str, model: &str) -> ModelIdentifier {
        let config = config_from_json(json);
        let (provider_id, model_name) = model.split_once('/').expect("provider/model");
        let provider = config
            .provider(provider_id)
            .expect("known provider")
            .clone();
        ModelIdentifier {
            provider: provider.kind,
            model_name: model_name.to_string(),
            config: provider,
        }
    }

    // -- prefix matching over the built-in table --

    #[test]
    fn prefix_match_exact() {
        let caps = lookup_capabilities(&id(ProviderKind::Claude, "claude-sonnet-4-6"));
        assert_eq!(caps.context_window, 1_000_000);
        assert_eq!(caps.max_tokens, 64_000);
        assert!(caps.reasoning);
        assert!(caps.image_input);
    }

    #[test]
    fn prefix_match_snapshot_suffix() {
        let caps = lookup_capabilities(&id(ProviderKind::Claude, "claude-sonnet-4-6-20260115"));
        assert_eq!(caps.context_window, 1_000_000);
        assert_eq!(caps.max_tokens, 64_000);

        let caps = lookup_capabilities(&id(ProviderKind::OpenAI, "gpt-5-2025-08-07"));
        assert_eq!(caps.context_window, 400_000);
        assert_eq!(caps.max_tokens, 128_000);
        assert!(caps.reasoning);
    }

    #[test]
    fn prefix_match_longest_wins() {
        let caps = lookup_capabilities(&id(ProviderKind::OpenAI, "gpt-5-mini-xxx"));
        assert_eq!(
            caps.cost,
            Some(ModelCostRates {
                input: 0.25,
                output: 2.0,
                cache_read: Some(0.025),
                cache_write: None,
            })
        );

        let caps = lookup_capabilities(&id(ProviderKind::OpenAI, "gpt-5.1"));
        // gpt-5.1 uses "none" for minimal effort, distinguishing it from gpt-5.
        assert_eq!(
            caps.thinking_levels[0],
            (ThinkingLevel::Minimal, Some("none".to_string()))
        );
    }

    #[test]
    fn builtin_cache_rates() {
        // Claude: reads 0.1x input, writes 1.25x input. Compare via the same
        // float expressions the table uses (3.0 * 0.1 is not exactly 0.3).
        let caps = lookup_capabilities(&id(ProviderKind::Claude, "claude-sonnet-4-6"));
        assert_eq!(
            caps.cost,
            Some(ModelCostRates {
                input: 3.0,
                output: 15.0,
                cache_read: Some(3.0 * 0.1),
                cache_write: Some(3.0 * 1.25),
            })
        );

        // OpenAI gpt-5 family: cached reads 0.1x input, no write billing.
        let caps = lookup_capabilities(&id(ProviderKind::OpenAI, "gpt-5"));
        assert_eq!(
            caps.cost,
            Some(ModelCostRates {
                input: 1.25,
                output: 10.0,
                cache_read: Some(0.125),
                cache_write: None,
            })
        );

        // Older OpenAI models: cached rates unknown.
        let caps = lookup_capabilities(&id(ProviderKind::OpenAI, "gpt-4o"));
        let cost = caps.cost.expect("gpt-4o has cost");
        assert_eq!(cost.cache_read, None);
        assert_eq!(cost.cache_write, None);
    }

    #[test]
    fn builtin_claude_effort_levels() {
        let levels = |name: &str| {
            lookup_capabilities(&id(ProviderKind::Claude, name))
                .thinking_levels
                .into_iter()
                .map(|(level, _)| level)
                .collect::<Vec<_>>()
        };
        use ThinkingLevel::*;
        assert_eq!(
            levels("claude-opus-4-8"),
            vec![Low, Medium, High, XHigh, Max]
        );
        assert_eq!(levels("claude-sonnet-4-6"), vec![Low, Medium, High, Max]);
        assert_eq!(levels("claude-haiku-4-5"), vec![Minimal, Low, Medium, High]);
    }

    #[test]
    fn prefix_match_provider_scoped() {
        let caps = lookup_capabilities(&id(ProviderKind::Ollama, "gpt-5"));
        assert_eq!(caps.context_window, DEFAULT_CONTEXT_WINDOW);
        assert_eq!(caps.max_tokens, DEFAULT_MAX_TOKENS);
        assert!(!caps.reasoning);
    }

    #[test]
    fn builtin_table_skips_custom_providers() {
        // A proxy speaking the OpenAI API is not OpenAI: its gpt-5 may have
        // different limits and prices.
        let caps = lookup_capabilities(&with_json(
            r#"{ "providers": { "proxy": { "baseUrl": "http://h", "api": "openai-completions" } } }"#,
            "proxy/gpt-5",
        ));
        assert_eq!(caps.context_window, DEFAULT_CONTEXT_WINDOW);
        assert_eq!(caps.cost, None);
    }

    #[test]
    fn unknown_model_falls_back_to_default() {
        let caps = lookup_capabilities(&id(ProviderKind::OpenAI, "totally-unknown"));
        assert_eq!(caps.context_window, 8192);
        assert_eq!(caps.max_tokens, 8192);
        assert!(!caps.reasoning);
        assert!(caps.thinking_levels.is_empty());
        assert_eq!(caps.cost, None);
        assert!(!caps.image_input);
    }

    // -- models.json entries --

    #[test]
    fn user_entry_overrides_builtin_fields() {
        let json = r#"{ "providers": { "claude": { "models": [
            { "id": "claude-sonnet-4-6", "cost": { "input": 9.9, "output": 99.9 } }
        ] } } }"#;
        let caps = lookup_capabilities(&with_json(json, "claude/claude-sonnet-4-6"));
        assert_eq!(
            caps.cost,
            Some(ModelCostRates {
                input: 9.9,
                output: 99.9,
                cache_read: None,
                cache_write: None,
            })
        );
        // Non-overridden fields keep their built-in values.
        assert_eq!(caps.context_window, 1_000_000);
        assert_eq!(caps.max_tokens, 64_000);

        // Exact match only: a snapshot of the same model keeps the built-in
        // cost.
        let caps = lookup_capabilities(&with_json(json, "claude/claude-sonnet-4-6-20260115"));
        assert_eq!(caps.cost.map(|c| c.input), Some(3.0));
    }

    #[test]
    fn user_entry_is_scoped_to_its_provider() {
        let json = r#"{ "providers": {
            "claude": { "models": [{ "id": "shared-name", "contextWindow": 1000 }] },
            "lm": { "baseUrl": "http://h", "api": "anthropic-messages" }
        } }"#;
        let caps = lookup_capabilities(&with_json(json, "lm/shared-name"));
        assert_eq!(caps.context_window, DEFAULT_CONTEXT_WINDOW);
    }

    #[test]
    fn user_entry_for_custom_provider_model() {
        let caps = lookup_capabilities(&with_json(
            r#"{ "providers": { "lm": {
                "baseUrl": "http://h", "api": "openai-completions",
                "models": [{
                    "id": "local-model", "contextWindow": 32768, "maxTokens": 4096,
                    "input": ["text", "image"], "cost": { "input": 0, "output": 0 }
                }]
            } } }"#,
            "lm/local-model",
        ));
        assert_eq!(caps.context_window, 32768);
        assert_eq!(caps.max_tokens, 4096);
        assert!(caps.image_input);
        assert!(!caps.reasoning);
        assert_eq!(
            caps.cost,
            Some(ModelCostRates {
                input: 0.0,
                output: 0.0,
                cache_read: None,
                cache_write: None,
            })
        );
    }

    fn levels_for(api: &str, model: &str) -> Vec<(ThinkingLevel, Option<String>)> {
        let json = format!(
            r#"{{ "providers": {{ "p": {{ "baseUrl": "http://h", "api": "{api}",
                "models": [{model}] }} }} }}"#
        );
        lookup_capabilities(&with_json(&json, "p/m")).thinking_levels
    }

    #[test]
    fn reasoning_true_gives_per_api_default_levels() {
        use ThinkingLevel::*;
        assert_eq!(
            levels_for("openai-completions", r#"{ "id": "m", "reasoning": true }"#),
            vec![
                (Minimal, Some("minimal".to_string())),
                (Low, Some("low".to_string())),
                (Medium, Some("medium".to_string())),
                (High, Some("high".to_string())),
            ]
        );
        for api in ["anthropic-messages", "ollama"] {
            assert_eq!(
                levels_for(api, r#"{ "id": "m", "reasoning": true }"#),
                vec![(Minimal, None), (Low, None), (Medium, None), (High, None)],
                "{api}"
            );
        }
    }

    #[test]
    fn thinking_level_map_edits_levels() {
        use ThinkingLevel::*;
        // null removes a level, a string sets its parameter, and xhigh/max
        // become available only when mapped.
        assert_eq!(
            levels_for(
                "anthropic-messages",
                r#"{ "id": "m", "reasoning": true,
                     "thinkingLevelMap": { "minimal": null, "high": "high", "max": "max" } }"#
            ),
            vec![
                (Low, None),
                (Medium, None),
                (High, Some("high".to_string())),
                (Max, Some("max".to_string())),
            ]
        );
        // A map alone (no `reasoning`) enables just the levels it names.
        assert_eq!(
            levels_for(
                "openai-completions",
                r#"{ "id": "m", "thinkingLevelMap": { "low": "low", "xhigh": "xhigh" } }"#
            ),
            vec![
                (Low, Some("low".to_string())),
                (XHigh, Some("xhigh".to_string()))
            ]
        );
        assert!(
            levels_for("openai-completions", r#"{ "id": "m", "reasoning": false }"#).is_empty()
        );
    }

    #[test]
    fn user_entry_can_turn_off_builtin_thinking() {
        let caps = lookup_capabilities(&with_json(
            r#"{ "providers": { "openai": { "models": [{ "id": "gpt-5", "reasoning": false }] } } }"#,
            "openai/gpt-5",
        ));
        assert!(!caps.reasoning);
        assert!(caps.thinking_levels.is_empty());
    }

    #[test]
    fn max_request_clamps_to_highest_supported_level() {
        use ThinkingLevel::*;
        let unmapped = levels_for("openai-completions", r#"{ "id": "m", "reasoning": true }"#);
        assert_eq!(
            clamp_thinking_level(Some(Max), &unmapped),
            Some((High, Some("high".to_string())))
        );
        let mapped = levels_for(
            "openai-completions",
            r#"{ "id": "m", "reasoning": true, "thinkingLevelMap": { "xhigh": "xhigh", "max": "max" } }"#,
        );
        assert_eq!(
            clamp_thinking_level(Some(Max), &mapped),
            Some((Max, Some("max".to_string())))
        );
    }

    // -- Ollama probed metadata --

    #[cfg(feature = "ollama")]
    fn set_probe(id: &ModelIdentifier, probe: OllamaProbe) {
        get_ollama_probe_map()
            .lock()
            .unwrap()
            .insert(probe_key(id), probe);
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_measured_context_used() {
        let model = id(ProviderKind::Ollama, "testonly-ollama-model");
        set_probe(
            &model,
            OllamaProbe {
                context_length: Some(131_072),
                image_input: None,
                thinking: None,
            },
        );
        let caps = lookup_capabilities(&model);
        assert_eq!(caps.context_window, 131_072);
        // Only context_window is probed here; the rest stays at defaults.
        assert_eq!(caps.max_tokens, DEFAULT_MAX_TOKENS);
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_probes_are_per_provider() {
        let json =
            r#"{ "providers": { "gpu": { "baseUrl": "http://gpu:11434", "api": "ollama" } } }"#;
        let local = id(ProviderKind::Ollama, "testonly-per-provider");
        let remote = with_json(json, "gpu/testonly-per-provider");
        set_probe(
            &remote,
            OllamaProbe {
                context_length: Some(65_536),
                image_input: None,
                thinking: None,
            },
        );
        assert_eq!(lookup_capabilities(&remote).context_window, 65_536);
        assert_eq!(
            lookup_capabilities(&local).context_window,
            DEFAULT_CONTEXT_WINDOW
        );
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_user_json_beats_measured() {
        let model = with_json(
            r#"{ "providers": { "ollama": { "models": [
                { "id": "testonly-ollama-model2", "contextWindow": 8000 }
            ] } } }"#,
            "ollama/testonly-ollama-model2",
        );
        set_probe(
            &model,
            OllamaProbe {
                context_length: Some(131_072),
                image_input: None,
                thinking: None,
            },
        );
        assert_eq!(lookup_capabilities(&model).context_window, 8000);
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_probed_no_vision_enables_image_demotion() {
        let model = id(ProviderKind::Ollama, "testonly-textonly-model");
        set_probe(
            &model,
            OllamaProbe {
                context_length: None,
                image_input: Some(false),
                thinking: None,
            },
        );
        // resolve_entry must yield the positive "no vision" signal that
        // gates image demotion in prepare_messages.
        assert_eq!(resolve_entry(&model).image_input, Some(false));
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_user_json_beats_probed_vision() {
        let model = with_json(
            r#"{ "providers": { "ollama": { "models": [
                { "id": "testonly-vision-override", "input": ["text", "image"] }
            ] } } }"#,
            "ollama/testonly-vision-override",
        );
        set_probe(
            &model,
            OllamaProbe {
                context_length: None,
                image_input: Some(false),
                thinking: None,
            },
        );
        assert_eq!(resolve_entry(&model).image_input, Some(true));
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_probed_thinking_enables_levels() {
        let model = id(ProviderKind::Ollama, "testonly-thinking-model");
        set_probe(
            &model,
            OllamaProbe {
                context_length: None,
                image_input: None,
                thinking: Some(true),
            },
        );
        // The probed capability must reach the per-turn clamp so that
        // thinking_level actually produces `think: true` on Ollama.
        let entry = resolve_entry(&model);
        assert_eq!(
            clamp_thinking_level(
                Some(ThinkingLevel::High),
                entry.thinking_levels.as_deref().unwrap_or(&[]),
            ),
            Some((ThinkingLevel::High, None))
        );
        assert!(lookup_capabilities(&model).reasoning);
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_probed_no_thinking_stays_off() {
        let model = id(ProviderKind::Ollama, "testonly-nothinking-model");
        set_probe(
            &model,
            OllamaProbe {
                context_length: None,
                image_input: None,
                thinking: Some(false),
            },
        );
        let entry = resolve_entry(&model);
        assert_eq!(entry.thinking_levels, Some(vec![]));
        assert_eq!(
            clamp_thinking_level(
                Some(ThinkingLevel::High),
                entry.thinking_levels.as_deref().unwrap_or(&[]),
            ),
            None
        );
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn extract_capability_variants() {
        // Listed capabilities are positive signals.
        let info = serde_json::json!({ "capabilities": ["completion", "vision", "thinking"] });
        assert_eq!(extract_capability(&info, "vision"), Some(true));
        assert_eq!(extract_capability(&info, "thinking"), Some(true));

        // Capabilities present but missing: a positive "unsupported" signal.
        let info = serde_json::json!({ "capabilities": ["completion", "tools"] });
        assert_eq!(extract_capability(&info, "vision"), Some(false));
        assert_eq!(extract_capability(&info, "thinking"), Some(false));

        // Older servers omit the field entirely: unknown.
        let info = serde_json::json!({ "model_info": {} });
        assert_eq!(extract_capability(&info, "vision"), None);
        assert_eq!(extract_capability(&info, "thinking"), None);
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn extract_context_length_variants() {
        // Architecture-qualified key preferred.
        let info = serde_json::json!({
            "model_info": {
                "general.architecture": "llama",
                "llama.context_length": 131072,
                "other.context_length": 999
            }
        });
        assert_eq!(extract_context_length(&info), Some(131_072));

        // Fallback scan when the architecture key is absent.
        let info = serde_json::json!({
            "model_info": { "qwen2.context_length": 32768 }
        });
        assert_eq!(extract_context_length(&info), Some(32_768));

        // No model_info at all.
        let info = serde_json::json!({ "details": {} });
        assert_eq!(extract_context_length(&info), None);

        // model_info present but no context_length key.
        let info = serde_json::json!({
            "model_info": { "general.architecture": "llama" }
        });
        assert_eq!(extract_context_length(&info), None);
    }

    // -- clamping --

    #[test]
    fn clamp_unset_returns_none() {
        assert_eq!(clamp_max_tokens(0, Some(64_000)), None);
        assert_eq!(clamp_max_tokens(-1, Some(64_000)), None);
        assert_eq!(clamp_max_tokens(-1, None), None);
    }

    #[test]
    fn clamp_above_limit() {
        assert_eq!(clamp_max_tokens(999_999, Some(64_000)), Some(64_000));
    }

    #[test]
    fn clamp_below_limit_kept() {
        assert_eq!(clamp_max_tokens(4096, Some(64_000)), Some(4096));
    }

    #[test]
    fn clamp_unknown_model_not_clamped() {
        assert_eq!(clamp_max_tokens(999_999, None), Some(999_999));
    }

    #[test]
    fn clamp_saturates_above_u32() {
        assert_eq!(clamp_max_tokens(i64::MAX, None), Some(u32::MAX));
        assert_eq!(clamp_max_tokens(i64::MAX, Some(64_000)), Some(64_000));
    }

    // -- thinking level parsing and clamping --

    #[test]
    fn thinking_level_parse_values() {
        assert_eq!(
            ThinkingLevel::parse("minimal"),
            Some(ThinkingLevel::Minimal)
        );
        assert_eq!(ThinkingLevel::parse("low"), Some(ThinkingLevel::Low));
        assert_eq!(ThinkingLevel::parse("medium"), Some(ThinkingLevel::Medium));
        assert_eq!(ThinkingLevel::parse("high"), Some(ThinkingLevel::High));
        assert_eq!(ThinkingLevel::parse("xhigh"), Some(ThinkingLevel::XHigh));
        assert_eq!(ThinkingLevel::parse("max"), Some(ThinkingLevel::Max));
        assert_eq!(ThinkingLevel::parse("off"), None);
        assert_eq!(ThinkingLevel::parse(""), None);
        assert_eq!(ThinkingLevel::parse("HIGH"), None);
    }

    #[test]
    fn clamp_thinking_exact_match() {
        let supported = default_thinking_levels(ProviderKind::Claude);
        assert_eq!(
            clamp_thinking_level(Some(ThinkingLevel::Medium), &supported),
            Some((ThinkingLevel::Medium, None))
        );
    }

    #[test]
    fn clamp_thinking_rounds_up_to_nearest() {
        // Adaptive models have no Minimal; the nearest supported level is Low.
        let supported = adaptive_levels(true);
        assert_eq!(
            clamp_thinking_level(Some(ThinkingLevel::Minimal), &supported),
            Some((ThinkingLevel::Low, Some("low".to_string())))
        );
    }

    #[test]
    fn clamp_thinking_rounds_down_to_nearest() {
        let supported = vec![(ThinkingLevel::Low, Some("low".to_string()))];
        assert_eq!(
            clamp_thinking_level(Some(ThinkingLevel::High), &supported),
            Some((ThinkingLevel::Low, Some("low".to_string())))
        );
    }

    #[test]
    fn clamp_thinking_tie_prefers_lower_level() {
        // Medium is equidistant from Low and High; the cheaper Low wins.
        let supported = vec![(ThinkingLevel::Low, None), (ThinkingLevel::High, None)];
        assert_eq!(
            clamp_thinking_level(Some(ThinkingLevel::Medium), &supported),
            Some((ThinkingLevel::Low, None))
        );
        // XHigh sits between High and Max on a model without it.
        let supported = adaptive_levels(false);
        assert_eq!(
            clamp_thinking_level(Some(ThinkingLevel::XHigh), &supported),
            Some((ThinkingLevel::High, Some("high".to_string())))
        );
    }

    #[test]
    fn clamp_thinking_unsupported_model_is_off() {
        assert_eq!(clamp_thinking_level(Some(ThinkingLevel::High), &[]), None);
    }

    #[test]
    fn clamp_thinking_off_stays_off() {
        assert_eq!(
            clamp_thinking_level(None, &default_thinking_levels(ProviderKind::Claude)),
            None
        );
    }

    // -- serde --

    #[test]
    fn thinking_level_serde_roundtrip() {
        for (level, s) in [
            (ThinkingLevel::Minimal, "\"minimal\""),
            (ThinkingLevel::Low, "\"low\""),
            (ThinkingLevel::Medium, "\"medium\""),
            (ThinkingLevel::High, "\"high\""),
            (ThinkingLevel::XHigh, "\"xhigh\""),
            (ThinkingLevel::Max, "\"max\""),
        ] {
            assert_eq!(serde_json::to_string(&level).expect("serialize"), s);
            let back: ThinkingLevel = serde_json::from_str(s).expect("deserialize");
            assert_eq!(back, level);
            assert_eq!(level.as_str(), s.trim_matches('"'));
        }
    }
}
