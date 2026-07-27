use modular_agent_core::AgentError;

const VALID_JSON_ESCAPES: [char; 9] = ['"', '\\', '/', 'b', 'f', 'n', 'r', 't', 'u'];

/// Repair malformed JSON string literals by escaping raw control characters
/// inside strings and doubling backslashes that start invalid escapes.
///
/// Port of pi's `repairJson` (packages/ai/src/utils/json-parse.ts).
fn repair_json(json: &str) -> String {
    let chars: Vec<char> = json.chars().collect();
    let mut repaired = String::with_capacity(json.len());
    let mut in_string = false;

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        if !in_string {
            repaired.push(c);
            if c == '"' {
                in_string = true;
            }
            i += 1;
            continue;
        }

        if c == '"' {
            repaired.push(c);
            in_string = false;
            i += 1;
            continue;
        }

        if c == '\\' {
            let Some(&next) = chars.get(i + 1) else {
                repaired.push_str("\\\\");
                i += 1;
                continue;
            };
            if next == 'u' {
                let hex: String = chars.iter().skip(i + 2).take(4).collect();
                if hex.chars().count() == 4 && hex.chars().all(|h| h.is_ascii_hexdigit()) {
                    repaired.push_str("\\u");
                    repaired.push_str(&hex);
                    i += 6;
                    continue;
                }
            }
            if VALID_JSON_ESCAPES.contains(&next) {
                repaired.push('\\');
                repaired.push(next);
                i += 2;
            } else {
                repaired.push_str("\\\\");
                i += 1;
            }
            continue;
        }

        match c {
            '\u{08}' => repaired.push_str("\\b"),
            '\u{0c}' => repaired.push_str("\\f"),
            '\n' => repaired.push_str("\\n"),
            '\r' => repaired.push_str("\\r"),
            '\t' => repaired.push_str("\\t"),
            _ if (c as u32) <= 0x1f => {
                repaired.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => repaired.push(c),
        }
        i += 1;
    }

    repaired
}

/// Strict-parse a JSON string, falling back to a single repair pass.
///
/// Unlike pi's parser this never degrades to partial-JSON or an empty object:
/// it only runs at finalization, where a silent fallback would hide that the
/// model produced unusable arguments.
pub(crate) fn parse_json_with_repair(s: &str) -> Result<serde_json::Value, AgentError> {
    match serde_json::from_str(s) {
        Ok(value) => Ok(value),
        Err(e) => {
            let repaired = repair_json(s);
            if repaired != s
                && let Ok(value) = serde_json::from_str(&repaired)
            {
                return Ok(value);
            }
            Err(AgentError::InvalidValue(format!("Invalid JSON: {}", e)))
        }
    }
}

const RAW_ARGS_TRUNCATE_CHARS: usize = 200;

/// Parse a provider-sent tool-argument string at finalization time.
///
/// An empty string means a no-arg tool call, not a failure. Valid JSON that is
/// not an object is rejected like a parse failure: both the tool contract and
/// Claude's tool_use.input require an object, so letting e.g. `[1,2]` through
/// would break history replay. On failure, parameters fall back to an empty
/// object so replaying the history stays API-valid, and the returned error
/// string carries a truncated copy of the raw text so the model can see what
/// it produced and re-issue the call.
pub(crate) fn parse_tool_arguments(arguments: &str) -> (serde_json::Value, Option<String>) {
    if arguments.trim().is_empty() {
        return (serde_json::json!({}), None);
    }
    let error = match parse_json_with_repair(arguments) {
        Ok(value) if value.is_object() => return (value, None),
        Ok(value) => format!(
            "Tool arguments must be a JSON object, got {}.",
            json_type_name(&value)
        ),
        Err(e) => e.to_string(),
    };
    let mut raw: String = arguments.chars().take(RAW_ARGS_TRUNCATE_CHARS).collect();
    if arguments.chars().count() > RAW_ARGS_TRUNCATE_CHARS {
        raw.push_str("...");
    }
    (
        serde_json::json!({}),
        Some(format!("{} Raw arguments (truncated): {}", error, raw)),
    )
}

fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_json_passes_strict() {
        let value = parse_json_with_repair(r#"{"city": "Tokyo", "n": 3}"#).unwrap();
        assert_eq!(value, serde_json::json!({"city": "Tokyo", "n": 3}));
    }

    #[test]
    fn test_raw_control_chars_in_string_repaired() {
        let value = parse_json_with_repair("{\"text\": \"line1\nline2\tend\"}").unwrap();
        assert_eq!(value, serde_json::json!({"text": "line1\nline2\tend"}));

        // Non-printable control char becomes a \u escape
        let value = parse_json_with_repair("{\"text\": \"a\u{01}b\"}").unwrap();
        assert_eq!(value, serde_json::json!({"text": "a\u{01}b"}));
    }

    #[test]
    fn test_invalid_escape_repaired() {
        let value = parse_json_with_repair(r#"{"path": "C:\x\y"}"#).unwrap();
        assert_eq!(value, serde_json::json!({"path": "C:\\x\\y"}));
    }

    #[test]
    fn test_invalid_unicode_escape_backslash_doubled() {
        // \uZZZZ is not a valid escape; the backslash-u survives but the
        // repaired text is still invalid JSON, so this must be an error.
        assert!(parse_json_with_repair(r#"{"a": "\uZZZZ"}"#).is_err());
        // A valid \u escape is preserved as-is.
        let value = parse_json_with_repair(r#"{"a": "あ"}"#).unwrap();
        assert_eq!(value, serde_json::json!({"a": "あ"}));
    }

    #[test]
    fn test_truncated_object_is_err() {
        assert!(parse_json_with_repair(r#"{"city": "Tok"#).is_err());
    }

    #[test]
    fn test_plain_prose_is_err() {
        assert!(parse_json_with_repair("I will now call the tool.").is_err());
    }

    #[test]
    fn test_escaped_backslash_edge_cases() {
        // String a ends with an escaped backslash; if the state machine
        // wrongly treated a's closing quote as escaped, it would corrupt the
        // rest of the document instead of repairing the raw newline in b.
        let value = parse_json_with_repair("{\"a\": \"x\\\\\", \"b\": \"l1\nl2\"}").unwrap();
        assert_eq!(value, serde_json::json!({"a": "x\\", "b": "l1\nl2"}));

        // Escaped backslash followed by a raw control char INSIDE the string;
        // only the newline needs escaping.
        let value = parse_json_with_repair("{\"a\": \"x\\\\\ny\"}").unwrap();
        assert_eq!(value, serde_json::json!({"a": "x\\\ny"}));

        // Trailing lone backslash at end of input is doubled (still invalid
        // JSON overall because the string is unterminated).
        assert!(parse_json_with_repair("{\"a\": \"x\\").is_err());
    }

    #[test]
    fn test_parse_tool_arguments_empty_is_no_arg_call() {
        let (params, err) = parse_tool_arguments("");
        assert_eq!(params, serde_json::json!({}));
        assert!(err.is_none());

        let (params, err) = parse_tool_arguments("   ");
        assert_eq!(params, serde_json::json!({}));
        assert!(err.is_none());
    }

    #[test]
    fn test_parse_tool_arguments_rejects_non_object_json() {
        for (raw, type_name) in [
            ("null", "null"),
            ("[1,2]", "an array"),
            ("\"text\"", "a string"),
            ("42", "a number"),
            ("true", "a boolean"),
        ] {
            let (params, err) = parse_tool_arguments(raw);
            assert_eq!(params, serde_json::json!({}), "raw: {raw}");
            let err = err.unwrap();
            assert!(
                err.contains(&format!("must be a JSON object, got {}", type_name)),
                "raw: {raw}, err: {err}"
            );
            assert!(err.contains(raw), "raw: {raw}, err: {err}");
        }
    }

    #[test]
    fn test_parse_tool_arguments_failure_truncates_raw() {
        let long_garbage = format!("not json {}", "x".repeat(300));
        let (params, err) = parse_tool_arguments(&long_garbage);
        assert_eq!(params, serde_json::json!({}));
        let err = err.unwrap();
        assert!(err.contains("Raw arguments (truncated)"), "err was: {err}");
        assert!(err.contains("..."), "err was: {err}");
        assert!(!err.contains(&"x".repeat(250)), "err was: {err}");
    }
}
