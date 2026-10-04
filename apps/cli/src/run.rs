//! `ma run`: one patch wired to stdin/stdout, ending at EOF.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result};
use modular_agent_core::{ModularAgentEvent, Value};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::host::{Host, ServerArgs, parse_duration, shutdown_signal};

#[derive(clap::Args)]
pub(crate) struct RunArgs {
    /// Patch name in the workspace with --workspace, otherwise a patch file path
    patch: String,

    /// Workspace whose global configs apply (its auto-start list is not used)
    #[arg(long, value_name = "DIR")]
    workspace: Option<PathBuf>,

    /// Input channel fed from stdin
    #[arg(short, long, default_value = "input")]
    input: String,

    /// Output channel written to stdout
    #[arg(short, long, default_value = "output")]
    output: String,

    /// Read and write JSON lines, so values keep their types
    #[arg(long)]
    json: bool,

    /// Keep writing outputs this long after stdin ends
    #[arg(long, value_name = "DURATION", value_parser = parse_duration)]
    drain: Option<Duration>,

    #[command(flatten)]
    server: ServerArgs,
}

pub(crate) async fn run(args: RunArgs) -> Result<()> {
    // Without a workspace, MCP's save_patch writes next to the patch file.
    let fallback_patches_dir =
        args.workspace
            .is_none()
            .then(|| match Path::new(&args.patch).parent() {
                Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
                _ => PathBuf::from("."),
            });
    let host = Host::start(args.workspace.clone(), &args.server, fallback_patches_dir).await?;

    // Subscribe before starting the patch so no output is missed.
    let output_channel = args.output.clone();
    let mut outputs = host
        .ma
        .subscribe_to_event(move |envelope| match envelope.event {
            ModularAgentEvent::ExternalOutput(name, value) if name == output_channel => Some(value),
            _ => None,
        });

    let started = async {
        let patch_id = match &host.workspace {
            Some(workspace) => workspace.open_patch(&host.ma, &args.patch).await?,
            None => host
                .ma
                .open_patch_from_file(&args.patch, None)
                .await
                .with_context(|| format!("Failed to open patch {}", args.patch))?,
        };
        host.ma.start_patch(&patch_id).await?;
        anyhow::Ok(())
    }
    .await;
    if let Err(e) = started {
        host.shutdown().await?;
        return Err(e);
    }
    host.set_ready();

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let signal = shutdown_signal();
    tokio::pin!(signal);
    let mut interrupted = false;
    loop {
        tokio::select! {
            _ = &mut signal => {
                interrupted = true;
                break;
            }
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    if let Some(value) = parse_input(&line, args.json) {
                        host.ma.write_external_input(args.input.clone(), value).await?;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    log::error!("Failed to read stdin: {e}");
                    break;
                }
            },
            Some(value) = outputs.recv() => println!("{}", format_output(&value, args.json)),
        }
    }

    if !interrupted && let Some(drain) = args.drain {
        let deadline = tokio::time::sleep(drain);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                _ = &mut signal => break,
                Some(value) = outputs.recv() => println!("{}", format_output(&value, args.json)),
            }
        }
    }

    let result = host.shutdown().await;
    while let Ok(value) = outputs.try_recv() {
        println!("{}", format_output(&value, args.json));
    }
    result
}

/// One stdin line as an input value: the text itself, or with `json` the
/// value it encodes. Malformed JSON is logged and skipped.
fn parse_input(line: &str, json: bool) -> Option<Value> {
    if !json {
        return Some(Value::string(line));
    }
    let parsed = serde_json::from_str(line)
        .map_err(|e| e.to_string())
        .and_then(|v| Value::from_json(v).map_err(|e| e.to_string()));
    match parsed {
        Ok(value) => Some(value),
        Err(e) => {
            log::error!("Skipping input line that is not JSON: {e}");
            None
        }
    }
}

/// Strings print as-is in line mode; everything else, and everything in
/// JSON mode, prints as one line of JSON.
fn format_output(value: &Value, json: bool) -> String {
    match value {
        Value::String(s) if !json => s.to_string(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_and_json_modes() {
        assert_eq!(
            parse_input("{\"a\":1}", false),
            Some(Value::string("{\"a\":1}"))
        );
        let value = parse_input("{\"a\":1}", true).unwrap();
        assert_eq!(format_output(&value, true), "{\"a\":1}");
        assert_eq!(parse_input("not json", true), None);
        assert_eq!(format_output(&Value::string("hi"), false), "hi");
        assert_eq!(format_output(&Value::string("hi"), true), "\"hi\"");
    }
}
