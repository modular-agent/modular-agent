//! Helpers for building [`MessageContent`] from provider responses.

use modular_agent_core::{ContentBlock, MessageContent};

/// Collapse accumulated blocks into [`MessageContent`], keeping the legacy
/// plain-text form when no structured blocks are present so text-only
/// histories stay readable by older core versions.
#[cfg(feature = "claude")]
pub(crate) fn content_from_blocks(blocks: &[ContentBlock]) -> MessageContent {
    if blocks
        .iter()
        .all(|b| matches!(b, ContentBlock::Text { .. }))
    {
        MessageContent::Text(
            blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect(),
        )
    } else {
        MessageContent::Blocks(blocks.to_vec())
    }
}

/// Flattened text form of message content for string-only sinks: providers
/// whose tool role only accepts a string (OpenAI, Ollama), orphan tool-result
/// demotion, and the compaction transcript. Text blocks pass through and each
/// image block becomes an `[image: <mime_type>]` placeholder line, so the
/// model at least learns an image was returned instead of it being silently
/// dropped. Plain-text content is returned verbatim.
pub(crate) fn tool_result_fallback_text(content: &MessageContent) -> String {
    let MessageContent::Blocks(blocks) = content else {
        return content.text();
    };
    let parts: Vec<String> = blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } if !text.is_empty() => Some(text.clone()),
            ContentBlock::Image { mime_type, .. } => Some(format!("[image: {mime_type}]")),
            _ => None,
        })
        .collect();
    parts.join("\n")
}

/// Text content preceded by an unsigned thinking block; used by providers
/// that report thinking as a separate plain string (Ollama, OpenAI refusal).
/// Plain text when there is no thinking, so the common case keeps the legacy
/// string form.
#[cfg(any(feature = "openai", feature = "ollama"))]
pub(crate) fn content_with_thinking(thinking: &str, text: &str) -> MessageContent {
    if thinking.is_empty() {
        return MessageContent::Text(text.to_string());
    }
    let mut blocks = vec![ContentBlock::Thinking {
        thinking: thinking.to_string(),
        signature: None,
        redacted: false,
    }];
    if !text.is_empty() {
        blocks.push(ContentBlock::Text {
            text: text.to_string(),
        });
    }
    MessageContent::Blocks(blocks)
}
