//! Model capability registry.
//!
//! Resolves per-model metadata (context window, max output tokens, reasoning
//! support, cost rates, image input) from four layers, highest priority first:
//!
//! 1. User-defined `models.json` (loaded via [`load_model_capabilities_json`])
//! 2. Built-in static table (major official Claude / OpenAI models)
//! 3. Probed Ollama metadata (`/api/show`: context length and vision
//!    capability, cached per process)
//! 4. Conservative defaults (8192 tokens)
//!
//! All layers use longest-prefix matching on the model name, so snapshot
//! suffixes (`claude-sonnet-4-6-20260115`) and `-latest` aliases resolve to
//! their base entry. Layer priority is stronger than prefix length: a short
//! `models.json` key (even the empty string, which matches every model)
//! overrides a longer built-in prefix, but only for the fields it sets.
//!
//! # Example
//!
//! `models.json` maps model-name prefixes (no provider prefix) to partial
//! entries; every field is optional so a single field of a built-in entry can
//! be overridden:
//!
//! ```json
//! {
//!   "qwen2.5-coder-32b": { "context_window": 32768, "max_tokens": 8192, "cost": { "input": 0, "output": 0 } },
//!   "claude-sonnet-4-5": { "cost": { "input": 2.5, "output": 12.5 } },
//!   "my-reasoning-model": { "reasoning": true, "thinking_levels": [["low", "low"], ["high", "high"]] }
//! }
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex, OnceLock};

use modular_agent_core::AgentError;
use serde::{Deserialize, Serialize};

use crate::provider::{ModelIdentifier, ProviderKind};

/// Reasoning intensity vocabulary shared across providers.
///
/// "off" is represented as the *absence* of a level (`Option<ThinkingLevel>`
/// on the consumer side), not as a variant, so that
/// [`ModelCapabilities::thinking_levels`] only lists levels a model supports.
/// Consumed by the `thinking_level` agent configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Minimal,
    Low,
    Medium,
    High,
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
            _ => None,
        }
    }

    /// Ordinal used for nearest-level clamping.
    fn rank(self) -> i32 {
        match self {
            Self::Minimal => 0,
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
        }
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCostRates {
    pub input: f64,
    pub output: f64,
    /// Rate for input tokens read from the prompt cache. `None` means the
    /// cached rate is unknown; cost consumers fall back to `input`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    /// Rate for input tokens written to the prompt cache. `None` means the
    /// provider has no separate write billing (or it is unknown); cost
    /// consumers fall back to `input`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

/// One entry of `models.json`, or an internal partially-resolved record.
/// All fields optional so a user entry can overwrite a single field of a
/// built-in entry (e.g. revise only `cost`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCapabilitiesEntry {
    pub context_window: Option<u32>,
    pub max_tokens: Option<u32>,
    pub reasoning: Option<bool>,
    pub thinking_levels: Option<Vec<(ThinkingLevel, Option<String>)>>,
    pub cost: Option<ModelCostRates>,
    pub image_input: Option<bool>,
}

impl ModelCapabilitiesEntry {
    /// Overwrite self with the `Some` fields of `other`. `thinking_levels`
    /// and `cost` are replaced wholesale, not element-merged, so a user entry
    /// fully controls any field it chooses to set.
    fn overwrite_with(&mut self, other: &Self) {
        if other.context_window.is_some() {
            self.context_window = other.context_window;
        }
        if other.max_tokens.is_some() {
            self.max_tokens = other.max_tokens;
        }
        if other.reasoning.is_some() {
            self.reasoning = other.reasoning;
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

    fn into_capabilities(self) -> ModelCapabilities {
        ModelCapabilities {
            context_window: self.context_window.unwrap_or(DEFAULT_CONTEXT_WINDOW),
            max_tokens: self.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            reasoning: self.reasoning.unwrap_or(false),
            thinking_levels: self.thinking_levels.unwrap_or_default(),
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
            reasoning: Some(self.reasoning),
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

// ============================================================================
// Global state
// ============================================================================

/// User-defined entries from models.json. Reload replaces the whole map.
static USER_ENTRIES: OnceLock<Mutex<HashMap<String, ModelCapabilitiesEntry>>> = OnceLock::new();

fn get_user_entries() -> &'static Mutex<HashMap<String, ModelCapabilitiesEntry>> {
    USER_ENTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Metadata probed from one Ollama `/api/show` call. A `None` field means
/// the response didn't expose it (older servers).
#[cfg(feature = "ollama")]
#[derive(Debug, Clone, Copy)]
struct OllamaProbe {
    context_length: Option<u32>,
    image_input: Option<bool>,
    thinking: Option<bool>,
}

/// Probed Ollama metadata, keyed by model name. An entry means /api/show
/// succeeded (cached to avoid re-querying every turn, even when all fields
/// came back empty). Network errors are NOT cached so a transient failure
/// retries next turn. The key omits the server URL: `ollama_url` is a global
/// config, so a process effectively talks to one server.
#[cfg(feature = "ollama")]
static OLLAMA_PROBES: OnceLock<Mutex<HashMap<String, OllamaProbe>>> = OnceLock::new();

#[cfg(feature = "ollama")]
fn get_ollama_probe_map() -> &'static Mutex<HashMap<String, OllamaProbe>> {
    OLLAMA_PROBES.get_or_init(|| Mutex::new(HashMap::new()))
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
/// The write rate assumes the 5-minute TTL. 1-hour cache writes (ChatAgent
/// `cache_retention = "long"`) are billed at 2x, but Anthropic reports one
/// combined cache-write token count, so the rate cannot be picked per write;
/// users on 1h retention can override `cache_write` via models.json.
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
/// Claude's effort vocabulary for these models is low/medium/high (plus
/// xhigh/max, which `ThinkingLevel` intentionally cannot represent — its
/// variants mirror the `thinking_level` config vocabulary). `budget_tokens` does not
/// apply here: it is rejected or deprecated on adaptive-thinking models.
/// The effort string reaches the API as `output_config.effort` alongside
/// `thinking: {"type": "adaptive"}`.
fn adaptive_levels() -> Vec<(ThinkingLevel, Option<String>)> {
    use ThinkingLevel::*;
    vec![
        (Low, Some("low".into())),
        (Medium, Some("medium".into())),
        (High, Some("high".into())),
    ]
}

/// Claude `budget_tokens` thinking: the budget amount per level is chosen by
/// the caller, hence no provider-side parameter value.
fn budget_levels() -> Vec<(ThinkingLevel, Option<String>)> {
    use ThinkingLevel::*;
    vec![(Minimal, None), (Low, None), (Medium, None), (High, None)]
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
// - Current Claude models: claude-api skill catalog (cached 2026-06-04).
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
            adaptive_levels(),
            claude_cost(10.0, 50.0),
        ),
        builtin(
            Claude,
            "claude-opus-4-8",
            1_000_000,
            128_000,
            adaptive_levels(),
            claude_cost(5.0, 25.0),
        ),
        builtin(
            Claude,
            "claude-opus-4-7",
            1_000_000,
            128_000,
            adaptive_levels(),
            claude_cost(5.0, 25.0),
        ),
        builtin(
            Claude,
            "claude-opus-4-6",
            1_000_000,
            128_000,
            adaptive_levels(),
            claude_cost(5.0, 25.0),
        ),
        builtin(
            Claude,
            "claude-sonnet-4-6",
            1_000_000,
            64_000,
            adaptive_levels(),
            claude_cost(3.0, 15.0),
        ),
        builtin(
            Claude,
            "claude-haiku-4-5",
            200_000,
            64_000,
            budget_levels(),
            claude_cost(1.0, 5.0),
        ),
        builtin(
            Claude,
            "claude-sonnet-4-5",
            200_000,
            64_000,
            budget_levels(),
            claude_cost(3.0, 15.0),
        ),
        builtin(
            Claude,
            "claude-opus-4-5",
            200_000,
            64_000,
            budget_levels(),
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

/// Merged (json > static > ollama) entry with `None` for unknown fields.
/// Request-building code uses this to distinguish "known limit" from
/// "unknown model" (only known models get clamped).
///
/// Locks are never held across await points, so `std::sync::Mutex` suffices.
/// `.unwrap()` on the locks: poisoning requires a prior panic while holding
/// the lock, in which case propagating the panic is the right outcome.
pub(crate) fn resolve_entry(id: &ModelIdentifier) -> ModelCapabilitiesEntry {
    let mut merged = ModelCapabilitiesEntry::default();

    // Layer 3: probed Ollama metadata (lowest of the known layers).
    // Exact model-name key, no prefix match: the cache key is the very name
    // that was queried.
    #[cfg(feature = "ollama")]
    if id.provider == ProviderKind::Ollama
        && let Some(probe) = get_ollama_probe_map().lock().unwrap().get(&id.model_name)
    {
        merged.context_window = probe.context_length;
        merged.image_input = probe.image_input;
        // Ollama's request-side `think` flag is boolean, so a probed
        // "thinking" capability maps to every level with no per-level
        // parameter; "capabilities present but no thinking" is a positive
        // "unsupported" signal (empty list).
        merged.thinking_levels = probe
            .thinking
            .map(|t| if t { ollama_thinking_levels() } else { vec![] });
        merged.reasoning = probe.thinking;
    }

    // Layer 2: built-in static table — provider-scoped, longest prefix wins.
    if let Some(caps) = longest_prefix_match(
        &id.model_name,
        BUILTIN_MODELS
            .iter()
            .filter(|m| m.provider == id.provider)
            .map(|m| (m.prefix, &m.caps)),
    ) {
        merged.overwrite_with(&caps.to_entry());
    }

    // Layer 1: models.json — provider-agnostic keys (bare model names, no
    // `claude/` prefix), longest prefix wins, only `Some` fields overwrite.
    {
        let user = get_user_entries().lock().unwrap();
        if let Some(entry) =
            longest_prefix_match(&id.model_name, user.iter().map(|(k, v)| (k.as_str(), v)))
        {
            merged.overwrite_with(entry);
        }
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
// models.json loading
// ============================================================================

/// Load (or reload) user-defined model capabilities from a JSON file.
///
/// Counterpart of `modular_agent_core::register_tools_from_mcp_json`: CLI and
/// desktop call this once at startup with their `models.json` path. Reloading
/// *replaces* the previous user table entirely (no merge) so reloads are
/// idempotent — e.g. removing an entry from the file actually removes it.
/// Unlike the MCP loader this is synchronous: it only reads one file.
pub fn load_model_capabilities_json(path: impl AsRef<Path>) -> Result<(), AgentError> {
    let path = path.as_ref();
    let s = std::fs::read_to_string(path).map_err(|e| {
        AgentError::IoError(format!(
            "Failed to read model capabilities file '{}': {}",
            path.display(),
            e
        ))
    })?;
    let entries = parse_model_capabilities_json(&s)?;
    let n = entries.len();
    *get_user_entries().lock().unwrap() = entries;
    log::info!(
        "Loaded {} model capability entries from {}",
        n,
        path.display()
    );
    Ok(())
}

/// Pure parser, unit-testable without touching global state.
/// `deny_unknown_fields` on the entry type surfaces key typos as load errors.
fn parse_model_capabilities_json(
    s: &str,
) -> Result<HashMap<String, ModelCapabilitiesEntry>, AgentError> {
    serde_json::from_str(s)
        .map_err(|e| AgentError::InvalidConfig(format!("Invalid model capabilities JSON: {}", e)))
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
/// probe existed); once P-19 makes the request path read `context_window`,
/// callers must warm before resolving.
#[cfg(feature = "ollama")]
pub(crate) async fn warm_ollama_context(
    client: &crate::ollama_client::OllamaClient,
    model_name: &str,
) {
    // Fast path: already queried (hit or confirmed-absent).
    if get_ollama_probe_map()
        .lock()
        .unwrap()
        .contains_key(model_name)
    {
        return;
    }
    match client.show_model_info(model_name).await {
        Ok(info) => {
            let probe = OllamaProbe {
                context_length: extract_context_length(&info),
                image_input: extract_capability(&info, "vision"),
                thinking: extract_capability(&info, "thinking"),
            };
            get_ollama_probe_map()
                .lock()
                .unwrap()
                .insert(model_name.to_string(), probe);
        }
        // Transient failures are not cached; next turn retries.
        Err(e) => log::warn!("Failed to probe Ollama model info for '{model_name}': {e}"),
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

/// Ollama's `think` request flag is boolean, so a model that reports the
/// "thinking" capability supports every level, with the boolean conversion
/// left to the caller (no per-level parameter).
#[cfg(feature = "ollama")]
fn ollama_thinking_levels() -> Vec<(ThinkingLevel, Option<String>)> {
    use ThinkingLevel::*;
    vec![(Minimal, None), (Low, None), (Medium, None), (High, None)]
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that mutate the global user table (and, for the
    /// Ollama tests, the measured-context cache). Read-only tests rely only
    /// on the built-in table / defaults for the fields they assert, so they
    /// may run concurrently with the mutating tests.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        // A panic in another test must not cascade into poisoning failures.
        TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn id(provider: ProviderKind, name: &str) -> ModelIdentifier {
        ModelIdentifier {
            provider,
            model_name: name.to_string(),
        }
    }

    fn set_user(json: &str) {
        *get_user_entries().lock().unwrap() =
            parse_model_capabilities_json(json).expect("test JSON must parse");
    }

    fn clear_user() {
        get_user_entries().lock().unwrap().clear();
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
    fn prefix_match_provider_scoped() {
        let caps = lookup_capabilities(&id(ProviderKind::Ollama, "gpt-5"));
        assert_eq!(caps.context_window, DEFAULT_CONTEXT_WINDOW);
        assert_eq!(caps.max_tokens, DEFAULT_MAX_TOKENS);
        assert!(!caps.reasoning);
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

    // -- merge semantics (mutate the user table; hold TEST_LOCK) --

    #[test]
    fn merge_user_partial_overrides_builtin() {
        let _guard = test_lock();
        set_user(r#"{ "claude-sonnet-4-6": { "cost": { "input": 9.9, "output": 99.9 } } }"#);
        let caps = lookup_capabilities(&id(ProviderKind::Claude, "claude-sonnet-4-6-20260115"));
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
        clear_user();
    }

    #[test]
    fn merge_user_short_prefix_beats_longer_builtin() {
        let _guard = test_lock();
        // Layer priority is stronger than prefix length: the short user key
        // "claude" overrides the longer built-in "claude-opus-4-8" entry.
        set_user(r#"{ "claude": { "cost": { "input": 1.0, "output": 2.0 } } }"#);
        let caps = lookup_capabilities(&id(ProviderKind::Claude, "claude-opus-4-8"));
        assert_eq!(
            caps.cost,
            Some(ModelCostRates {
                input: 1.0,
                output: 2.0,
                cache_read: None,
                cache_write: None,
            })
        );
        assert_eq!(caps.context_window, 1_000_000);
        assert_eq!(caps.max_tokens, 128_000);
        clear_user();
    }

    #[test]
    fn merge_user_entry_for_unknown_model() {
        let _guard = test_lock();
        set_user(
            r#"{ "testonly-local-model": {
                "context_window": 32768, "max_tokens": 4096, "reasoning": true,
                "thinking_levels": [["low", "low"], ["high", null]],
                "cost": { "input": 0, "output": 0 }, "image_input": true
            } }"#,
        );
        let caps = lookup_capabilities(&id(ProviderKind::OpenAI, "testonly-local-model-v2"));
        assert_eq!(caps.context_window, 32768);
        assert_eq!(caps.max_tokens, 4096);
        assert!(caps.reasoning);
        assert_eq!(
            caps.thinking_levels,
            vec![
                (ThinkingLevel::Low, Some("low".to_string())),
                (ThinkingLevel::High, None),
            ]
        );
        assert_eq!(
            caps.cost,
            Some(ModelCostRates {
                input: 0.0,
                output: 0.0,
                cache_read: None,
                cache_write: None,
            })
        );
        assert!(caps.image_input);
        clear_user();
    }

    // -- Ollama probed metadata --

    #[cfg(feature = "ollama")]
    fn set_probe(name: &str, probe: OllamaProbe) {
        get_ollama_probe_map()
            .lock()
            .unwrap()
            .insert(name.to_string(), probe);
    }

    #[cfg(feature = "ollama")]
    fn remove_probe(name: &str) {
        get_ollama_probe_map().lock().unwrap().remove(name);
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_measured_context_used() {
        let _guard = test_lock();
        set_probe(
            "testonly-ollama-model",
            OllamaProbe {
                context_length: Some(131_072),
                image_input: None,
                thinking: None,
            },
        );
        let caps = lookup_capabilities(&id(ProviderKind::Ollama, "testonly-ollama-model"));
        assert_eq!(caps.context_window, 131_072);
        // Only context_window is probed here; the rest stays at defaults.
        assert_eq!(caps.max_tokens, DEFAULT_MAX_TOKENS);
        remove_probe("testonly-ollama-model");
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_user_json_beats_measured() {
        let _guard = test_lock();
        set_probe(
            "testonly-ollama-model2",
            OllamaProbe {
                context_length: Some(131_072),
                image_input: None,
                thinking: None,
            },
        );
        set_user(r#"{ "testonly-ollama-model2": { "context_window": 8000 } }"#);
        let caps = lookup_capabilities(&id(ProviderKind::Ollama, "testonly-ollama-model2"));
        assert_eq!(caps.context_window, 8000);
        clear_user();
        remove_probe("testonly-ollama-model2");
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_probed_no_vision_enables_image_demotion() {
        let _guard = test_lock();
        set_probe(
            "testonly-textonly-model",
            OllamaProbe {
                context_length: None,
                image_input: Some(false),
                thinking: None,
            },
        );
        // resolve_entry must yield the positive "no vision" signal that
        // gates image demotion in prepare_messages.
        let entry = resolve_entry(&id(ProviderKind::Ollama, "testonly-textonly-model"));
        assert_eq!(entry.image_input, Some(false));
        remove_probe("testonly-textonly-model");
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_user_json_beats_probed_vision() {
        let _guard = test_lock();
        set_probe(
            "testonly-vision-override",
            OllamaProbe {
                context_length: None,
                image_input: Some(false),
                thinking: None,
            },
        );
        set_user(r#"{ "testonly-vision-override": { "image_input": true } }"#);
        let entry = resolve_entry(&id(ProviderKind::Ollama, "testonly-vision-override"));
        assert_eq!(entry.image_input, Some(true));
        clear_user();
        remove_probe("testonly-vision-override");
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_probed_thinking_enables_levels() {
        let _guard = test_lock();
        set_probe(
            "testonly-thinking-model",
            OllamaProbe {
                context_length: None,
                image_input: None,
                thinking: Some(true),
            },
        );
        // The probed capability must reach the per-turn clamp so that
        // thinking_level actually produces `think: true` on Ollama.
        let entry = resolve_entry(&id(ProviderKind::Ollama, "testonly-thinking-model"));
        assert_eq!(
            clamp_thinking_level(
                Some(ThinkingLevel::High),
                entry.thinking_levels.as_deref().unwrap_or(&[]),
            ),
            Some((ThinkingLevel::High, None))
        );
        let caps = lookup_capabilities(&id(ProviderKind::Ollama, "testonly-thinking-model"));
        assert!(caps.reasoning);
        remove_probe("testonly-thinking-model");
    }

    #[cfg(feature = "ollama")]
    #[test]
    fn ollama_probed_no_thinking_stays_off() {
        let _guard = test_lock();
        set_probe(
            "testonly-nothinking-model",
            OllamaProbe {
                context_length: None,
                image_input: None,
                thinking: Some(false),
            },
        );
        let entry = resolve_entry(&id(ProviderKind::Ollama, "testonly-nothinking-model"));
        assert_eq!(entry.thinking_levels, Some(vec![]));
        assert_eq!(
            clamp_thinking_level(
                Some(ThinkingLevel::High),
                entry.thinking_levels.as_deref().unwrap_or(&[]),
            ),
            None
        );
        remove_probe("testonly-nothinking-model");
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
        assert_eq!(ThinkingLevel::parse("off"), None);
        assert_eq!(ThinkingLevel::parse(""), None);
        assert_eq!(ThinkingLevel::parse("HIGH"), None);
    }

    #[test]
    fn clamp_thinking_exact_match() {
        let supported = budget_levels();
        assert_eq!(
            clamp_thinking_level(Some(ThinkingLevel::Medium), &supported),
            Some((ThinkingLevel::Medium, None))
        );
    }

    #[test]
    fn clamp_thinking_rounds_up_to_nearest() {
        // Adaptive models have no Minimal; the nearest supported level is Low.
        let supported = adaptive_levels();
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
    }

    #[test]
    fn clamp_thinking_unsupported_model_is_off() {
        assert_eq!(clamp_thinking_level(Some(ThinkingLevel::High), &[]), None);
    }

    #[test]
    fn clamp_thinking_off_stays_off() {
        assert_eq!(clamp_thinking_level(None, &budget_levels()), None);
    }

    // -- JSON parsing --

    #[test]
    fn parse_valid_json() {
        let entries = parse_model_capabilities_json(
            r#"{
                "qwen2.5-coder-32b": { "context_window": 32768, "max_tokens": 8192, "cost": { "input": 0, "output": 0 } },
                "claude-sonnet-4-5": { "cost": { "input": 2.5, "output": 12.5 } },
                "my-reasoning-model": { "reasoning": true, "thinking_levels": [["low", "low"], ["high", "high"]] }
            }"#,
        )
        .expect("must parse");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries["qwen2.5-coder-32b"].context_window, Some(32768));
        assert_eq!(entries["claude-sonnet-4-5"].context_window, None);
        assert_eq!(
            entries["claude-sonnet-4-5"].cost,
            Some(ModelCostRates {
                input: 2.5,
                output: 12.5,
                cache_read: None,
                cache_write: None,
            })
        );
        assert_eq!(
            entries["my-reasoning-model"].thinking_levels,
            Some(vec![
                (ThinkingLevel::Low, Some("low".to_string())),
                (ThinkingLevel::High, Some("high".to_string())),
            ])
        );
    }

    #[test]
    fn parse_cost_with_cache_rates() {
        let entries = parse_model_capabilities_json(
            r#"{ "m": { "cost": { "input": 1.0, "output": 2.0, "cache_read": 0.1, "cache_write": 1.25 } } }"#,
        )
        .expect("must parse");
        assert_eq!(
            entries["m"].cost,
            Some(ModelCostRates {
                input: 1.0,
                output: 2.0,
                cache_read: Some(0.1),
                cache_write: Some(1.25),
            })
        );
    }

    #[test]
    fn parse_invalid_json_is_error() {
        let err = parse_model_capabilities_json("{ not json").expect_err("must fail");
        assert!(matches!(err, AgentError::InvalidConfig(_)));
    }

    #[test]
    fn parse_unknown_field_is_error() {
        let err = parse_model_capabilities_json(r#"{ "m": { "context_windw": 1 } }"#)
            .expect_err("typo must be rejected");
        assert!(matches!(err, AgentError::InvalidConfig(_)));
    }

    #[test]
    fn load_replaces_previous_entries() {
        let _guard = test_lock();
        let dir = std::env::temp_dir();
        let p1 = dir.join("modular-agent-llm-testonly-models-1.json");
        let p2 = dir.join("modular-agent-llm-testonly-models-2.json");
        std::fs::write(&p1, r#"{ "testonly-old": { "max_tokens": 1 } }"#).expect("write");
        std::fs::write(&p2, r#"{ "testonly-new": { "max_tokens": 2 } }"#).expect("write");

        load_model_capabilities_json(&p1).expect("load 1");
        assert!(
            get_user_entries()
                .lock()
                .unwrap()
                .contains_key("testonly-old")
        );

        load_model_capabilities_json(&p2).expect("load 2");
        {
            let user = get_user_entries().lock().unwrap();
            assert!(!user.contains_key("testonly-old"));
            assert!(user.contains_key("testonly-new"));
        }

        clear_user();
        let _ = std::fs::remove_file(&p1);
        let _ = std::fs::remove_file(&p2);
    }

    #[test]
    fn load_missing_file_is_io_error() {
        let err =
            load_model_capabilities_json("Z:/does-not-exist/models.json").expect_err("must fail");
        assert!(matches!(err, AgentError::IoError(_)));
    }

    // -- serde --

    #[test]
    fn thinking_level_serde_roundtrip() {
        for (level, s) in [
            (ThinkingLevel::Minimal, "\"minimal\""),
            (ThinkingLevel::Low, "\"low\""),
            (ThinkingLevel::Medium, "\"medium\""),
            (ThinkingLevel::High, "\"high\""),
        ] {
            assert_eq!(serde_json::to_string(&level).expect("serialize"), s);
            let back: ThinkingLevel = serde_json::from_str(s).expect("deserialize");
            assert_eq!(back, level);
        }
    }
}
