//! Logging to stderr: env_logger's text format, or one JSON object per line
//! for container log pipelines.

use std::fmt::Display;
use std::io::Write as _;

use clap::ValueEnum;
use log::kv::{Error as KvError, Key, Value as KvValue, VisitSource};
use log::{Level, Record};
use serde_json::Value as JsonValue;

/// Without `RUST_LOG`, show the converted runtime events and Log node records
/// (`modular_agent::events`, `modular_agent::patch`) and only warnings from
/// everything else. Filter directives match target prefixes, so this leaves
/// out `modular_agent_core::*`.
const DEFAULT_FILTER: &str = "warn,modular_agent::=info";

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub(crate) enum LogFormat {
    #[default]
    Text,
    Json,
}

pub(crate) fn init(format: LogFormat) {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(DEFAULT_FILTER));
    builder.target(env_logger::Target::Stderr);
    if let LogFormat::Json = format {
        builder.format(|buf, record| {
            let line = json_line(buf.timestamp_millis(), record);
            writeln!(buf, "{line}")
        });
    }
    builder.init();
}

/// One log line in the flat shape cloud log pipelines read without extra
/// configuration: `timestamp`, `severity`, `message`, `target`, then the
/// record's key-values at the top level.
///
/// Written field by field rather than through a `serde_json::Map`, which
/// sorts its keys (serde_json's `preserve_order` would change map ordering
/// for the whole workspace); each value is still escaped by serde_json.
fn json_line(timestamp: impl Display, record: &Record) -> String {
    let mut line = String::from("{");
    let mut fields = Fields(&mut line);
    fields.push("timestamp", &timestamp.to_string());
    fields.push("severity", severity(record.level()));
    fields.push("message", &record.args().to_string());
    fields.push("target", record.target());
    // Visiting only fails when the visitor does, and ours never does.
    let _ = record.key_values().visit(&mut fields);
    line.push('}');
    line
}

/// Cloud Logging's severity names; it reads `WARN` as unknown.
fn severity(level: Level) -> &'static str {
    match level {
        Level::Error => "ERROR",
        Level::Warn => "WARNING",
        Level::Info => "INFO",
        Level::Debug | Level::Trace => "DEBUG",
    }
}

struct Fields<'a>(&'a mut String);

impl Fields<'_> {
    fn push(&mut self, key: &str, value: &str) {
        if self.0.len() > 1 {
            self.0.push(',');
        }
        self.0.push_str(&JsonValue::from(key).to_string());
        self.0.push(':');
        self.0.push_str(&JsonValue::from(value).to_string());
    }
}

impl<'kvs> VisitSource<'kvs> for Fields<'_> {
    fn visit_pair(&mut self, key: Key<'kvs>, value: KvValue<'kvs>) -> Result<(), KvError> {
        self.push(key.as_str(), &value.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_line_flattens_fields() {
        let kvs = [("patch", "bugs/triage"), ("module", "m1")];
        let line = json_line(
            "2026-10-03T08:00:00.123Z",
            &Record::builder()
                .level(Level::Warn)
                .target("modular_agent::patch")
                .args(format_args!("said \"hi\""))
                .key_values(&kvs)
                .build(),
        );
        assert!(
            line.starts_with(r#"{"timestamp":"2026-10-03T08:00:00.123Z","severity":"WARNING","#),
            "{line}"
        );
        let json: JsonValue = serde_json::from_str(&line).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "timestamp": "2026-10-03T08:00:00.123Z",
                "severity": "WARNING",
                "message": "said \"hi\"",
                "target": "modular_agent::patch",
                "patch": "bugs/triage",
                "module": "m1",
            })
        );
    }

    #[test]
    fn trace_maps_to_debug() {
        assert_eq!(severity(Level::Trace), "DEBUG");
        assert_eq!(severity(Level::Error), "ERROR");
    }
}
