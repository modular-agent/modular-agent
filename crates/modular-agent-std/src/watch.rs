use std::path::Path;
use std::time::Duration;

use im::hashmap;
use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentSpec, AgentStatus, AgentValue, AsAgent,
    ModularAgent, async_trait, modular_agent,
};
use notify_debouncer_full::notify::event::ModifyKind;
use notify_debouncer_full::notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

use crate::time::parse_duration_to_ms;

const CATEGORY: &str = "Std/File";

const PORT_EVENT: &str = "event";

const CONFIG_PATH: &str = "path";
const CONFIG_RECURSIVE: &str = "recursive";
const CONFIG_DEBOUNCE: &str = "debounce";

const DEBOUNCE_DEFAULT: &str = "500ms";

/// Watches a directory for file system changes and emits an event for each change.
///
/// Changes are debounced: rapid bursts of notifications for the same file (for
/// example, the duplicate modify events some platforms produce for a single save)
/// are collapsed, and one event is emitted after the `debounce` interval has
/// passed without further activity. File access notifications are dropped.
/// When `path` is empty, the agent stays idle and watches nothing. Changing any
/// configuration while the agent is running restarts the watcher with the new
/// settings.
///
/// # Ports
/// - Output `event`: Object describing one debounced change:
///   - `kind`: One of "create", "modify", "rename", "remove", or "other"
///   - `path`: First affected path
///   - `paths`: All affected paths (a rename may carry both the old and new path)
///
/// # Configuration
/// - `path`: Directory to watch. Empty means watch nothing (default: "")
/// - `recursive`: Also watch subdirectories (default: true)
/// - `debounce`: Debounce interval, e.g. "500ms", "2s" (default: "500ms")
#[modular_agent(
    title = "Watch Directory",
    category = CATEGORY,
    outputs = [PORT_EVENT],
    string_config(name = CONFIG_PATH, default = ""),
    boolean_config(name = CONFIG_RECURSIVE, default = true),
    string_config(name = CONFIG_DEBOUNCE, default = DEBOUNCE_DEFAULT, description = "(ex. 500ms, 2s)", detail),
)]
struct WatchDirectoryAgent {
    data: AgentData,
    watcher: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
    path: String,
    recursive: bool,
    debounce_ms: u64,
}

fn event_kind_str(kind: &EventKind) -> Option<&'static str> {
    match kind {
        EventKind::Create(_) => Some("create"),
        EventKind::Modify(ModifyKind::Name(_)) => Some("rename"),
        EventKind::Modify(_) => Some("modify"),
        EventKind::Remove(_) => Some("remove"),
        EventKind::Access(_) => None,
        _ => Some("other"),
    }
}

impl WatchDirectoryAgent {
    fn start_watcher(&mut self) -> Result<(), AgentError> {
        if self.path.is_empty() {
            return Ok(());
        }

        let ma = self.ma().clone();
        let agent_id = self.id().to_string();

        let handler = move |result: DebounceEventResult| match result {
            Ok(events) => {
                for event in events {
                    let Some(kind) = event_kind_str(&event.kind) else {
                        continue;
                    };
                    let paths: im::Vector<AgentValue> = event
                        .paths
                        .iter()
                        .map(|p| AgentValue::string(p.to_string_lossy().to_string()))
                        .collect();
                    let path = event
                        .paths
                        .first()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let value = AgentValue::object(hashmap! {
                        "kind".to_string() => AgentValue::string(kind),
                        "path".to_string() => AgentValue::string(path),
                        "paths".to_string() => AgentValue::array(paths),
                    });
                    if let Err(e) = ma.try_send_agent_out(
                        agent_id.clone(),
                        AgentContext::new(),
                        PORT_EVENT.to_string(),
                        value,
                    ) {
                        log::error!("Failed to send watch event: {}", e);
                    }
                }
            }
            Err(errors) => {
                for e in errors {
                    log::error!("Watch error: {}", e);
                }
            }
        };

        // The worker thread checks its stop flag only once per tick (default:
        // debounce / 4), so cap the tick to keep stop/restart latency bounded
        // even when a large debounce interval is configured.
        let tick = Duration::from_millis((self.debounce_ms / 4).clamp(1, 250));
        let mut debouncer =
            new_debouncer(Duration::from_millis(self.debounce_ms), Some(tick), handler)
                .map_err(|e| AgentError::IoError(format!("Failed to create watcher: {}", e)))?;

        let mode = if self.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        debouncer.watch(Path::new(&self.path), mode).map_err(|e| {
            AgentError::IoError(format!("Failed to watch path '{}': {}", self.path, e))
        })?;

        self.watcher = Some(debouncer);
        Ok(())
    }

    fn stop_watcher(&mut self) {
        // Dropping the debouncer only signals its worker thread to stop; the
        // thread exits at the next tick and may still deliver one final batch
        // of already-debounced events through the old handler before it does.
        self.watcher.take();
    }
}

#[async_trait]
impl AsAgent for WatchDirectoryAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let configs = spec.configs.as_ref().ok_or(AgentError::NoConfig)?;
        let path = configs.get_string_or_default(CONFIG_PATH);
        let recursive = configs.get_bool_or(CONFIG_RECURSIVE, true);
        let debounce = configs.get_string_or(CONFIG_DEBOUNCE, DEBOUNCE_DEFAULT);
        let debounce_ms = parse_duration_to_ms(&debounce)?;

        Ok(Self {
            data: AgentData::new(ma, id, spec),
            watcher: None,
            path,
            recursive,
            debounce_ms,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        self.start_watcher()
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.stop_watcher();
        Ok(())
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let configs = self.configs()?;
        let path = configs.get_string_or_default(CONFIG_PATH);
        let recursive = configs.get_bool_or(CONFIG_RECURSIVE, true);
        let debounce = configs.get_string_or(CONFIG_DEBOUNCE, DEBOUNCE_DEFAULT);
        let debounce_ms = parse_duration_to_ms(&debounce)?;

        if path != self.path || recursive != self.recursive || debounce_ms != self.debounce_ms {
            self.path = path;
            self.recursive = recursive;
            self.debounce_ms = debounce_ms;
            if *self.status() == AgentStatus::Start {
                // Restart the watcher with the new settings
                self.stop_watcher();
                self.start_watcher()?;
            }
        }
        Ok(())
    }
}
