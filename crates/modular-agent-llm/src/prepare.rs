//! Provider-cross message normalization boundary (P-02).
//!
//! [`prepare_messages`] is the single pass every chat history goes through
//! immediately before provider-specific conversion. It repairs invariants
//! that provider APIs enforce but the dataflow graph cannot guarantee —
//! most importantly tool_use/tool_result pairing, which breaks when a
//! persisted history produced by one provider is replayed against another
//! (e.g. Ollama's historically id-less tool calls sent to Claude), or when
//! history trimming cuts between an assistant call turn and its results.
//!
//! The function is pure over the history; the graph model is unchanged.

use std::collections::{HashMap, HashSet};

use im::Vector;
use modular_agent_core::{AgentValue, Message, ToolCall};

use crate::provider::ProviderKind;

/// Synthetic result content for a tool call that never received one
/// (mirrors pi-agent's `insertSyntheticToolResults`).
const NO_RESULT_CONTENT: &str = "No result provided";

#[cfg(feature = "image")]
const IMAGE_PLACEHOLDER: &str = "[Image omitted: model does not support image input]";

/// Roles the target's converter handles natively. "developer" survives only
/// for the OpenAI target, where the Responses API maps it to its native
/// developer role (and the chat converter degrades it to user itself);
/// Claude would degrade it anyway, and Ollama forwards role strings verbatim
/// to chat templates that only know system/user/assistant/tool, so both get
/// the uniform user fallback here.
fn is_known_role(target: ProviderKind, role: &str) -> bool {
    match role {
        "user" | "assistant" | "system" | "tool" => true,
        "developer" => target == ProviderKind::OpenAI,
        _ => false,
    }
}

fn id_valid_for_target(target: ProviderKind, id: &str) -> bool {
    match target {
        // Anthropic requires tool_use ids to match ^[a-zA-Z0-9_-]{1,64}$.
        ProviderKind::Claude => {
            (1..=64).contains(&id.len())
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        }
        // OpenAI rejects tool_call_id values longer than 40 characters
        // ("string too long"); the cap is enforced server-side though absent
        // from the API reference.
        ProviderKind::OpenAI => (1..=40).contains(&id.len()),
        // Ollama's converter drops ids entirely, so any non-empty id works.
        ProviderKind::Ollama => !id.is_empty(),
    }
}

fn fresh_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Deterministic id for a call that was persisted without one, derived from
/// the message/call position so re-running the repair over the same history
/// yields the same ids every turn — a per-request random id would invalidate
/// provider prompt caches at the first legacy call for the lifetime of the
/// conversation. (Front-trimming shifts indices, but a trim busts the prompt
/// cache regardless.)
fn positional_id(mi: usize, ci: usize, used: &HashSet<String>) -> String {
    let base = format!("call_{mi}_{ci}");
    if !used.contains(&base) {
        return base;
    }
    // Deterministic disambiguation if a real id in the history collides.
    let mut n = 2usize;
    loop {
        let candidate = format!("{base}_{n}");
        if !used.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Deterministic repair keeps ids stable across turns (good for prompt
/// caching); a fresh UUID is only the fallback when the repair produces
/// nothing or collides with another id in the history.
fn sanitize_id(target: ProviderKind, id: &str, used: &HashSet<String>) -> String {
    let repaired = match target {
        ProviderKind::Claude => {
            let mut s: String = id
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            // All chars are ASCII after the mapping, so this cannot split a char.
            s.truncate(64);
            s
        }
        ProviderKind::OpenAI => {
            let mut s = id.to_string();
            if s.len() > 40 {
                // Truncate on a char boundary; ids are usually ASCII but the
                // history is arbitrary preset data.
                let mut end = 40;
                while !s.is_char_boundary(end) {
                    end -= 1;
                }
                s.truncate(end);
            }
            s
        }
        // Only an empty id is invalid for Ollama; nothing to repair.
        ProviderKind::Ollama => String::new(),
    };
    if !repaired.is_empty() && !used.contains(&repaired) {
        return repaired;
    }
    fresh_id()
}

/// Final tool_call ids per assistant-message index, in call order. The
/// result side is rewritten during the pairing pass in [`prepare_messages`],
/// so no separate old-id → new-id map is needed.
fn plan_call_ids(
    messages: &Vector<AgentValue>,
    target: ProviderKind,
) -> HashMap<usize, Vec<String>> {
    // Seed `used` with every already-valid id up front so a sanitized id can
    // never collide with a valid one appearing later in the history.
    let mut used: HashSet<String> = HashSet::new();
    for v in messages.iter() {
        if let Some(msg) = v.as_message()
            && msg.role == "assistant"
            && let Some(calls) = &msg.tool_calls
        {
            for call in calls.iter() {
                if let Some(id) = &call.function.id
                    && id_valid_for_target(target, id)
                {
                    used.insert(id.clone());
                }
            }
        }
    }

    let mut finals: HashMap<usize, Vec<String>> = HashMap::new();
    for (mi, v) in messages.iter().enumerate() {
        let Some(msg) = v.as_message() else { continue };
        if msg.role != "assistant" {
            continue;
        }
        let Some(calls) = &msg.tool_calls else {
            continue;
        };
        if calls.is_empty() {
            continue;
        }
        let ids = calls
            .iter()
            .enumerate()
            .map(|(ci, call)| match &call.function.id {
                Some(id) if id_valid_for_target(target, id) => id.clone(),
                Some(id) => {
                    let new = sanitize_id(target, id, &used);
                    used.insert(new.clone());
                    new
                }
                None => {
                    let new = positional_id(mi, ci, &used);
                    used.insert(new.clone());
                    new
                }
            })
            .collect();
        finals.insert(mi, ids);
    }
    finals
}

/// Per-message normalizations that don't involve tool pairing: unknown-role
/// fallback and image demotion. Returns `None` when the message needs no
/// change so callers can keep the original `AgentValue` untouched.
fn normalize_single(msg: &Message, target: ProviderKind, demote_images: bool) -> Option<Message> {
    let role_change = !is_known_role(target, &msg.role);

    #[cfg(feature = "image")]
    let image_change = demote_images && msg.image.is_some();
    #[cfg(not(feature = "image"))]
    let image_change = {
        // Without the image feature this crate never attaches images.
        let _ = demote_images;
        false
    };

    if !role_change && !image_change {
        return None;
    }
    let mut m = msg.clone();
    if role_change {
        m.role = "user".to_string();
    }
    #[cfg(feature = "image")]
    if image_change {
        m.image = None;
        m.content = if m.content.is_empty() {
            IMAGE_PLACEHOLDER.to_string()
        } else {
            format!("{}\n\n{}", m.content, IMAGE_PLACEHOLDER)
        };
    }
    Some(m)
}

/// Normalize a message outside tool pairing. Returns the original
/// `AgentValue` (cheap Arc clone) when nothing changes, so clean histories
/// pass through structurally unchanged.
fn prepare_plain(
    value: &AgentValue,
    msg: &Message,
    target: ProviderKind,
    demote_images: bool,
) -> AgentValue {
    match normalize_single(msg, target, demote_images) {
        Some(m) => m.into(),
        None => value.clone(),
    }
}

/// A tool result with no owning call — its call turn was trimmed off the
/// front of the history, it arrived after a non-tool message closed the
/// segment, or its call already got a result — cannot be sent as a tool
/// message: strict providers reject unpaired or duplicate tool results.
/// Demote it to plain user text so the content survives.
fn demote_orphan_tool_result(msg: &Message) -> Message {
    let content = match &msg.tool_name {
        Some(name) => format!("[Tool result from '{}']\n{}", name, msg.content),
        None => format!("[Tool result]\n{}", msg.content),
    };
    Message::user(content)
}

/// Normalize a chat history for the target provider:
///
/// 1. Missing tool_call ids get deterministic positional ids; ids violating
///    the target's constraints (Anthropic: `^[a-zA-Z0-9_-]{1,64}$`, OpenAI:
///    at most 40 chars) are rewritten, with the matching tool-result
///    messages rewritten consistently. Results whose ids don't match any
///    call (old Claude histories assigned independent UUIDs to each side)
///    are recovered by tool name, then by position within the segment.
/// 2. Tool calls without a result get a synthetic "No result provided" tool
///    message with `is_error`; tool results without an owning call (e.g.
///    the call turn was trimmed off the front of the history) are demoted
///    to plain user text. Both directions keep tool_use/tool_result pairing
///    intact for strict providers.
/// 3. Roles the target cannot handle fall back to user uniformly.
/// 4. When `demote_images` is set (the capability registry positively knows
///    the model lacks vision), image attachments are replaced with a text
///    placeholder.
pub(crate) fn prepare_messages(
    messages: &Vector<AgentValue>,
    target: ProviderKind,
    demote_images: bool,
) -> Vector<AgentValue> {
    let finals_by_msg = plan_call_ids(messages, target);
    let vals: Vec<&AgentValue> = messages.iter().collect();
    let mut out: Vec<AgentValue> = Vec::with_capacity(vals.len());

    let mut i = 0;
    while i < vals.len() {
        let Some(msg) = vals[i].as_message() else {
            // Non-message values are skipped by every provider converter;
            // pass them through untouched.
            out.push(vals[i].clone());
            i += 1;
            continue;
        };

        let is_call_turn =
            msg.role == "assistant" && msg.tool_calls.as_ref().is_some_and(|c| !c.is_empty());
        if !is_call_turn {
            if msg.role == "tool" {
                // Reached outside any call segment: an orphan result.
                out.push(demote_orphan_tool_result(msg).into());
            } else {
                out.push(prepare_plain(vals[i], msg, target, demote_images));
            }
            i += 1;
            continue;
        }

        // --- assistant turn carrying tool calls ---
        let calls: Vec<&ToolCall> = msg.tool_calls.iter().flatten().collect();
        let finals: Vec<String> = finals_by_msg.get(&i).cloned().unwrap_or_else(|| {
            // Unreachable: plan_call_ids visits the same messages. Regenerate
            // instead of panicking if the invariant ever breaks.
            calls
                .iter()
                .map(|c| c.function.id.clone().unwrap_or_else(fresh_id))
                .collect()
        });

        let ids_changed = calls
            .iter()
            .zip(&finals)
            .any(|(c, f)| c.function.id.as_deref() != Some(f.as_str()));
        if ids_changed {
            let mut m = msg.clone();
            m.tool_calls = Some(
                calls
                    .iter()
                    .zip(&finals)
                    .map(|(c, f)| {
                        let mut c = (*c).clone();
                        c.function.id = Some(f.clone());
                        c
                    })
                    .collect(),
            );
            let m = normalize_single(&m, target, demote_images).unwrap_or(m);
            out.push(m.into());
        } else {
            out.push(prepare_plain(vals[i], msg, target, demote_images));
        }
        i += 1;

        // Consume the tool-result run belonging to this turn. Non-message
        // values are transparent to every provider converter, so they must
        // not terminate the run (that would orphan the calls behind them AND
        // leave the real results as strays — a duplicate pair on the wire).
        let mut satisfied = vec![false; calls.len()];
        while i < vals.len() {
            let Some(tmsg) = vals[i].as_message() else {
                out.push(vals[i].clone());
                i += 1;
                continue;
            };
            if tmsg.role != "tool" {
                break;
            }
            // Pair by id first; recover missing ids (legacy Ollama) and
            // mismatched ids (old Claude double-UUID histories) by tool
            // name, then by position among the unsatisfied calls.
            let ci = tmsg
                .id
                .as_ref()
                .and_then(|tid| {
                    (0..calls.len()).find(|&ci| {
                        !satisfied[ci] && calls[ci].function.id.as_deref() == Some(tid.as_str())
                    })
                })
                .or_else(|| {
                    (0..calls.len()).find(|&ci| {
                        !satisfied[ci]
                            && tmsg.tool_name.as_deref() == Some(calls[ci].function.name.as_str())
                    })
                })
                .or_else(|| (0..calls.len()).find(|&ci| !satisfied[ci]));
            match ci {
                Some(ci) => {
                    satisfied[ci] = true;
                    if tmsg.id.as_deref() == Some(finals[ci].as_str()) {
                        out.push(prepare_plain(vals[i], tmsg, target, demote_images));
                    } else {
                        let mut m = tmsg.clone();
                        m.id = Some(finals[ci].clone());
                        let m = normalize_single(&m, target, demote_images).unwrap_or(m);
                        out.push(m.into());
                    }
                }
                // More results than calls: a second result on the same call
                // would put two tool_results on one id.
                None => out.push(demote_orphan_tool_result(tmsg).into()),
            }
            i += 1;
        }

        // Synthesize error results for calls that never got one.
        for ci in 0..calls.len() {
            if !satisfied[ci] {
                let mut m = Message::tool(
                    calls[ci].function.name.clone(),
                    NO_RESULT_CONTENT.to_string(),
                );
                m.id = Some(finals[ci].clone());
                m.is_error = Some(true);
                out.push(m.into());
            }
        }
    }

    out.into_iter().collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use im::vector;
    use modular_agent_core::ToolCallFunction;

    fn call(id: Option<&str>, name: &str) -> ToolCall {
        ToolCall {
            function: ToolCallFunction {
                id: id.map(String::from),
                name: name.to_string(),
                parameters: serde_json::json!({}),
                parse_error: None,
            },
        }
    }

    fn assistant_with_calls(content: &str, calls: Vec<ToolCall>) -> AgentValue {
        let mut m = Message::assistant(content.to_string());
        m.tool_calls = Some(calls.into());
        m.into()
    }

    fn tool_result(id: Option<&str>, name: &str, content: &str) -> AgentValue {
        let mut m = Message::tool(name.to_string(), content.to_string());
        m.id = id.map(String::from);
        m.into()
    }

    fn get(out: &Vector<AgentValue>, i: usize) -> &Message {
        out[i].as_message().expect("expected a message")
    }

    fn call_ids(msg: &Message) -> Vec<Option<String>> {
        msg.tool_calls
            .iter()
            .flatten()
            .map(|c| c.function.id.clone())
            .collect()
    }

    // -- missing-id assignment --

    #[test]
    fn missing_id_assigned_and_result_rewritten() {
        let history = vector![
            AgentValue::from(Message::user("weather?".to_string())),
            assistant_with_calls("", vec![call(None, "get_weather")]),
            tool_result(None, "get_weather", "22C"),
        ];
        let out = prepare_messages(&history, ProviderKind::OpenAI, false);

        assert_eq!(out.len(), 3);
        let ids = call_ids(get(&out, 1));
        let id = ids[0].as_deref().expect("id must be assigned");
        assert!(!id.is_empty());
        // Result side carries the same id so the pair stays matched.
        assert_eq!(get(&out, 2).id.as_deref(), Some(id));
        // No synthetic message was inserted.
        assert_eq!(get(&out, 2).content, "22C");
    }

    #[test]
    fn missing_id_assigned_is_anthropic_safe() {
        let history = vector![
            assistant_with_calls("", vec![call(None, "t")]),
            tool_result(None, "t", "ok"),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);
        let ids = call_ids(get(&out, 0));
        let id = ids[0].as_deref().expect("id must be assigned");
        assert!(id_valid_for_target(ProviderKind::Claude, id));
        assert_eq!(get(&out, 1).id.as_deref(), Some(id));
    }

    #[test]
    fn empty_string_id_replaced() {
        let history = vector![
            assistant_with_calls("", vec![call(Some(""), "t")]),
            tool_result(Some(""), "t", "ok"),
        ];
        let out = prepare_messages(&history, ProviderKind::OpenAI, false);
        let ids = call_ids(get(&out, 0));
        let id = ids[0].as_deref().expect("id must be assigned");
        assert!(!id.is_empty());
        assert_eq!(get(&out, 1).id.as_deref(), Some(id));
    }

    #[test]
    fn missing_ids_are_deterministic_across_calls() {
        // Random per-request ids would invalidate provider prompt caches on
        // every turn of a conversation with legacy id-less calls.
        let history = vector![
            assistant_with_calls("", vec![call(None, "a"), call(None, "b")]),
            tool_result(None, "a", "ra"),
            tool_result(None, "b", "rb"),
        ];
        let out1 = prepare_messages(&history, ProviderKind::Claude, false);
        let out2 = prepare_messages(&history, ProviderKind::Claude, false);
        assert_eq!(call_ids(get(&out1, 0)), call_ids(get(&out2, 0)));
        assert_eq!(get(&out1, 1).id, get(&out2, 1).id);
        assert_eq!(get(&out1, 2).id, get(&out2, 2).id);
    }

    #[test]
    fn positional_id_collision_disambiguated_deterministically() {
        let mut used = HashSet::new();
        used.insert("call_1_0".to_string());
        used.insert("call_1_0_2".to_string());
        assert_eq!(positional_id(1, 0, &used), "call_1_0_3");
        assert_eq!(positional_id(1, 0, &used), "call_1_0_3");
    }

    // -- Anthropic charset normalization --

    #[test]
    fn anthropic_charset_normalized_on_both_sides() {
        let history = vector![
            AgentValue::from(Message::user("hi".to_string())),
            assistant_with_calls("", vec![call(Some("call:weather!*"), "get_weather")]),
            tool_result(Some("call:weather!*"), "get_weather", "22C"),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        let ids = call_ids(get(&out, 1));
        assert_eq!(ids[0].as_deref(), Some("call_weather__"));
        assert_eq!(get(&out, 2).id.as_deref(), Some("call_weather__"));
    }

    #[test]
    fn anthropic_long_id_truncated_consistently() {
        let long = "a".repeat(80);
        let history = vector![
            assistant_with_calls("", vec![call(Some(&long), "t")]),
            tool_result(Some(&long), "t", "ok"),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        let ids = call_ids(get(&out, 0));
        let id = ids[0].as_deref().expect("id present");
        assert_eq!(id, "a".repeat(64));
        assert_eq!(get(&out, 1).id.as_deref(), Some(id));
    }

    #[test]
    fn openai_target_keeps_nonstandard_ids() {
        let history = vector![
            assistant_with_calls("", vec![call(Some("call:weather!*"), "get_weather")]),
            tool_result(Some("call:weather!*"), "get_weather", "22C"),
        ];
        let out = prepare_messages(&history, ProviderKind::OpenAI, false);

        let ids = call_ids(get(&out, 0));
        assert_eq!(ids[0].as_deref(), Some("call:weather!*"));
        assert_eq!(get(&out, 1).id.as_deref(), Some("call:weather!*"));
    }

    #[test]
    fn openai_long_id_truncated_to_40_on_both_sides() {
        // OpenAI enforces a 40-char cap on tool_call_id; a Claude-side
        // history can legitimately carry up to 64 chars.
        let long = "x".repeat(64);
        let history = vector![
            assistant_with_calls("", vec![call(Some(&long), "t")]),
            tool_result(Some(&long), "t", "ok"),
        ];
        let out = prepare_messages(&history, ProviderKind::OpenAI, false);

        let ids = call_ids(get(&out, 0));
        let id = ids[0].as_deref().expect("id present");
        assert_eq!(id, "x".repeat(40));
        assert_eq!(get(&out, 1).id.as_deref(), Some(id));
    }

    #[test]
    fn openai_40_char_id_kept() {
        let id40 = "y".repeat(40);
        let history = vector![
            assistant_with_calls("", vec![call(Some(&id40), "t")]),
            tool_result(Some(&id40), "t", "ok"),
        ];
        let out = prepare_messages(&history, ProviderKind::OpenAI, false);
        assert_eq!(call_ids(get(&out, 0))[0].as_deref(), Some(id40.as_str()));
    }

    #[test]
    fn sanitize_collision_falls_back_to_uuid_and_stays_paired() {
        // Both ids sanitize to "a_b"; the second must not collide.
        let history = vector![
            assistant_with_calls("", vec![call(Some("a b"), "t1"), call(Some("a?b"), "t2")]),
            tool_result(Some("a b"), "t1", "r1"),
            tool_result(Some("a?b"), "t2", "r2"),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        let ids = call_ids(get(&out, 0));
        let id1 = ids[0].clone().expect("id1");
        let id2 = ids[1].clone().expect("id2");
        assert_eq!(id1, "a_b");
        assert_ne!(id1, id2);
        assert!(id_valid_for_target(ProviderKind::Claude, &id2));
        assert_eq!(get(&out, 1).id.as_deref(), Some(id1.as_str()));
        assert_eq!(get(&out, 2).id.as_deref(), Some(id2.as_str()));
    }

    // -- orphan call synthesis --

    #[test]
    fn orphan_call_gets_synthetic_error_result_before_next_turn() {
        let history = vector![
            AgentValue::from(Message::user("go".to_string())),
            assistant_with_calls(
                "",
                vec![
                    call(Some("call_a"), "tool_a"),
                    call(Some("call_b"), "tool_b")
                ]
            ),
            tool_result(Some("call_a"), "tool_a", "ok"),
            AgentValue::from(Message::user("next".to_string())),
        ];
        let out = prepare_messages(&history, ProviderKind::OpenAI, false);

        assert_eq!(out.len(), 5);
        let synth = get(&out, 3);
        assert_eq!(synth.role, "tool");
        assert_eq!(synth.content, NO_RESULT_CONTENT);
        assert_eq!(synth.is_error, Some(true));
        assert_eq!(synth.id.as_deref(), Some("call_b"));
        assert_eq!(synth.tool_name.as_deref(), Some("tool_b"));
        // The user turn follows the synthetic result.
        assert_eq!(get(&out, 4).role, "user");
        assert_eq!(get(&out, 4).content, "next");
    }

    #[test]
    fn orphan_call_at_end_of_history_synthesized() {
        let history = vector![
            AgentValue::from(Message::user("go".to_string())),
            assistant_with_calls("", vec![call(Some("call_x"), "tool_x")]),
        ];
        let out = prepare_messages(&history, ProviderKind::OpenAI, false);

        assert_eq!(out.len(), 3);
        let synth = get(&out, 2);
        assert_eq!(synth.role, "tool");
        assert_eq!(synth.content, NO_RESULT_CONTENT);
        assert_eq!(synth.is_error, Some(true));
        assert_eq!(synth.id.as_deref(), Some("call_x"));
    }

    #[test]
    fn orphan_synthetic_inherits_normalized_id() {
        let history = vector![assistant_with_calls("", vec![call(Some("x y"), "t")])];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        assert_eq!(out.len(), 2);
        let ids = call_ids(get(&out, 0));
        assert_eq!(ids[0].as_deref(), Some("x_y"));
        assert_eq!(get(&out, 1).id.as_deref(), Some("x_y"));
        assert_eq!(get(&out, 1).is_error, Some(true));
    }

    #[test]
    fn missing_id_orphan_gets_synthetic_with_assigned_id() {
        let history = vector![assistant_with_calls("", vec![call(None, "t")])];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        assert_eq!(out.len(), 2);
        let ids = call_ids(get(&out, 0));
        let id = ids[0].as_deref().expect("id assigned");
        assert_eq!(get(&out, 1).id.as_deref(), Some(id));
        assert_eq!(get(&out, 1).is_error, Some(true));
    }

    // -- orphan result demotion --

    #[test]
    fn leading_orphan_tool_results_demoted_to_user_text() {
        // Front-trimming (MessagesAgent max_size) removes the assistant call
        // turn but keeps its results at the head of the history.
        let history = vector![
            tool_result(Some("call_gone"), "tool_a", "trimmed result"),
            tool_result(Some("call_gone2"), "tool_b", "trimmed result 2"),
            AgentValue::from(Message::user("next".to_string())),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        assert_eq!(out.len(), 3);
        for i in 0..2 {
            assert_eq!(get(&out, i).role, "user");
            assert_eq!(get(&out, i).id, None);
        }
        assert!(get(&out, 0).content.contains("trimmed result"));
        assert!(get(&out, 0).content.contains("tool_a"));
        assert_eq!(get(&out, 2).role, "user");
        assert_eq!(get(&out, 2).content, "next");
    }

    #[test]
    fn late_result_after_intervening_message_not_duplicated() {
        // The real result arrives after a non-tool message closed the
        // segment: the call gets a synthetic result, and the late real
        // result must not become a second tool result for the same id.
        let history = vector![
            assistant_with_calls("", vec![call(Some("call_a"), "tool_a")]),
            AgentValue::from(Message::user("mid".to_string())),
            tool_result(Some("call_a"), "tool_a", "real result"),
        ];
        let out = prepare_messages(&history, ProviderKind::OpenAI, false);

        assert_eq!(out.len(), 4);
        let synth = get(&out, 1);
        assert_eq!(synth.role, "tool");
        assert_eq!(synth.id.as_deref(), Some("call_a"));
        let late = get(&out, 3);
        assert_eq!(late.role, "user");
        assert!(late.content.contains("real result"));
        // Exactly one tool-role message for the call.
        let tool_count = out
            .iter()
            .filter(|v| v.as_message().is_some_and(|m| m.role == "tool"))
            .count();
        assert_eq!(tool_count, 1);
    }

    #[test]
    fn surplus_duplicate_result_demoted() {
        let history = vector![
            assistant_with_calls("", vec![call(Some("call_a"), "t")]),
            tool_result(Some("call_a"), "t", "first"),
            tool_result(Some("call_a"), "t", "second"),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        assert_eq!(out.len(), 3);
        assert_eq!(get(&out, 1).role, "tool");
        assert_eq!(get(&out, 1).content, "first");
        assert_eq!(get(&out, 2).role, "user");
        assert!(get(&out, 2).content.contains("second"));
    }

    // -- mismatched-id recovery (old Claude double-UUID histories) --

    #[test]
    fn mismatched_result_id_recovered_by_tool_name() {
        // The old claude_client assigned independent random UUIDs to
        // tool_use and tool_result, so ids never match across the pair.
        let history = vector![
            assistant_with_calls("", vec![call(Some("uuid_call_side"), "get_weather")]),
            tool_result(Some("uuid_result_side"), "get_weather", "22C"),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        assert_eq!(out.len(), 2);
        assert_eq!(get(&out, 1).id.as_deref(), Some("uuid_call_side"));
        assert_eq!(get(&out, 1).content, "22C");
        assert_eq!(get(&out, 1).role, "tool");
    }

    #[test]
    fn mismatched_result_id_recovered_by_position() {
        // Neither id nor tool name matches; the sole unsatisfied call in the
        // segment still claims the result rather than 400ing on both sides.
        let mut m = Message::new("tool".to_string(), "22C".to_string());
        m.id = Some("uuid_result_side".to_string());
        let history = vector![
            assistant_with_calls("", vec![call(Some("uuid_call_side"), "get_weather")]),
            AgentValue::from(m),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        assert_eq!(out.len(), 2);
        assert_eq!(get(&out, 1).id.as_deref(), Some("uuid_call_side"));
        assert_eq!(get(&out, 1).role, "tool");
    }

    // -- legacy id-less pairing --

    #[test]
    fn legacy_idless_results_pair_by_tool_name() {
        // Old Ollama histories have no ids on either side; results arrive
        // out of order relative to the calls.
        let history = vector![
            assistant_with_calls("", vec![call(None, "tool_a"), call(None, "tool_b")]),
            tool_result(None, "tool_b", "rb"),
            tool_result(None, "tool_a", "ra"),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        assert_eq!(out.len(), 3);
        let ids = call_ids(get(&out, 0));
        let ida = ids[0].as_deref().expect("ida");
        let idb = ids[1].as_deref().expect("idb");
        assert_ne!(ida, idb);
        assert_eq!(get(&out, 1).id.as_deref(), Some(idb));
        assert_eq!(get(&out, 1).content, "rb");
        assert_eq!(get(&out, 2).id.as_deref(), Some(ida));
        assert_eq!(get(&out, 2).content, "ra");
    }

    // -- no-op on clean histories --

    #[test]
    fn clean_text_history_passes_through_unchanged() {
        let history = vector![
            AgentValue::from(Message::system("Be helpful.".to_string())),
            AgentValue::from(Message::user("Hello".to_string())),
            AgentValue::from(Message::assistant("Hi!".to_string())),
            AgentValue::from(Message::user("How are you?".to_string())),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        assert_eq!(out.len(), history.len());
        for (a, b) in history.iter().zip(out.iter()) {
            // Full structural comparison via serialization (Message's
            // PartialEq only covers id/role/content).
            let ja = serde_json::to_value(a.as_message().expect("msg")).expect("ser");
            let jb = serde_json::to_value(b.as_message().expect("msg")).expect("ser");
            assert_eq!(ja, jb);
        }
    }

    #[test]
    fn clean_tool_pair_passes_through_unchanged() {
        let history = vector![
            AgentValue::from(Message::user("go".to_string())),
            assistant_with_calls("calling", vec![call(Some("toolu_ok1"), "t")]),
            tool_result(Some("toolu_ok1"), "t", "done"),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        assert_eq!(out.len(), 3);
        let ids = call_ids(get(&out, 1));
        assert_eq!(ids[0].as_deref(), Some("toolu_ok1"));
        assert_eq!(get(&out, 2).id.as_deref(), Some("toolu_ok1"));
        assert_eq!(get(&out, 2).content, "done");
    }

    #[test]
    fn non_message_values_pass_through() {
        let history = vector![
            AgentValue::string("not a message"),
            AgentValue::from(Message::user("hi".to_string())),
        ];
        let out = prepare_messages(&history, ProviderKind::OpenAI, false);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].as_str(), Some("not a message"));
    }

    #[test]
    fn non_message_value_inside_tool_run_does_not_orphan_calls() {
        // Provider converters skip non-message values, so one interleaved
        // inside a tool segment must not break the pairing (no synthetic
        // result, no demotion of the real one).
        let history = vector![
            assistant_with_calls("", vec![call(Some("call_a"), "t")]),
            AgentValue::string("interleaved"),
            tool_result(Some("call_a"), "t", "real"),
        ];
        let out = prepare_messages(&history, ProviderKind::Claude, false);

        assert_eq!(out.len(), 3);
        assert_eq!(out[1].as_str(), Some("interleaved"));
        assert_eq!(get(&out, 2).role, "tool");
        assert_eq!(get(&out, 2).id.as_deref(), Some("call_a"));
        assert_eq!(get(&out, 2).content, "real");
    }

    // -- role fallback --

    #[test]
    fn unknown_role_falls_back_to_user() {
        let history = vector![AgentValue::from(Message::new(
            "function".to_string(),
            "legacy".to_string(),
        ))];
        let out = prepare_messages(&history, ProviderKind::Ollama, false);
        assert_eq!(get(&out, 0).role, "user");
        assert_eq!(get(&out, 0).content, "legacy");
    }

    #[test]
    fn known_roles_are_preserved() {
        for role in ["user", "assistant", "system", "developer"] {
            let history = vector![AgentValue::from(Message::new(
                role.to_string(),
                "c".to_string(),
            ))];
            let out = prepare_messages(&history, ProviderKind::OpenAI, false);
            assert_eq!(get(&out, 0).role, role);
        }
    }

    #[test]
    fn developer_role_demoted_for_non_openai_targets() {
        // Ollama forwards role strings verbatim to chat templates that don't
        // know "developer"; Claude's converter would degrade it anyway.
        for target in [ProviderKind::Ollama, ProviderKind::Claude] {
            let history = vector![AgentValue::from(Message::new(
                "developer".to_string(),
                "policy".to_string(),
            ))];
            let out = prepare_messages(&history, target, false);
            assert_eq!(get(&out, 0).role, "user");
            assert_eq!(get(&out, 0).content, "policy");
        }
    }

    // -- image demotion --

    #[cfg(feature = "image")]
    fn image_message(content: &str) -> AgentValue {
        use std::sync::Arc;
        let img = photon_rs::PhotonImage::new(vec![0, 0, 0, 255], 1, 1);
        Message::user(content.to_string())
            .with_image(Arc::new(img))
            .into()
    }

    #[cfg(feature = "image")]
    #[test]
    fn image_demoted_to_placeholder_for_non_vision_target() {
        let history = vector![image_message("what is this?")];
        let out = prepare_messages(&history, ProviderKind::OpenAI, true);

        let msg = get(&out, 0);
        assert!(msg.image.is_none());
        assert!(msg.content.contains("what is this?"), "{}", msg.content);
        assert!(msg.content.contains(IMAGE_PLACEHOLDER), "{}", msg.content);
    }

    #[cfg(feature = "image")]
    #[test]
    fn image_only_message_demoted_to_placeholder_text() {
        let history = vector![image_message("")];
        let out = prepare_messages(&history, ProviderKind::OpenAI, true);

        let msg = get(&out, 0);
        assert!(msg.image.is_none());
        assert_eq!(msg.content, IMAGE_PLACEHOLDER);
    }

    #[cfg(feature = "image")]
    #[test]
    fn image_preserved_for_vision_target() {
        let history = vector![image_message("what is this?")];
        let out = prepare_messages(&history, ProviderKind::OpenAI, false);

        let msg = get(&out, 0);
        assert!(msg.image.is_some());
        assert_eq!(msg.content, "what is this?");
    }
}
