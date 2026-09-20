use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::Value as JsonValue;
use tokio::sync::{Mutex as AsyncMutex, broadcast, broadcast::error::RecvError, mpsc};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::FnvIndexMap;
use crate::config::{ModuleConfigs, ModuleConfigsMap};
use crate::context::ModuleContext;
use crate::definition::{ModuleConfigSpecs, ModuleDefinition, ModuleDefinitions};
use crate::error::{Error, Result};
use crate::id::{new_id, update_ids};
use crate::message::{self, ModuleEventMessage};
use crate::module::{Module, ModuleMessage, ModuleStatus, module_new};
use crate::patch::{Patch, PatchInfo};
use crate::registry;
use crate::spec::{ConnectionSpec, ModuleSpec, PatchSpec};
use crate::value::Value;

/// Message queues are unbounded, so instead of backpressure a queue that
/// crosses this depth logs escalating warnings (at 1024, 2048, 4096, ...).
const QUEUE_HIGH_WATER: usize = 1024;
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Registry size at which dead context-token entries are pruned. Entries are
/// `Weak` and die with their flow (contexts hold the only strong references).
/// The registry may exceed this threshold when more flows are genuinely live:
/// live entries must remain tracked so every flow stays abortable.
const CONTEXT_TOKEN_PRUNE_THRESHOLD: usize = 1024;

/// Distinguishes which module-loop incarnation owns the `module_tokens` slot,
/// so a draining old loop cannot clobber the token installed for a restarted
/// module's new loop (tokens themselves have no identity to compare).
static MODULE_TOKEN_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Shared, lockable handle to a running module instance.
pub type SharedModule = Arc<AsyncMutex<Box<dyn Module>>>;

// target module id / source handle / target handle
pub(crate) type ConnectionTarget = (String, String, String);

/// Sender half of a module's inbox. The channel is unbounded, so the queue
/// depth is tracked alongside the sender and runaway growth is surfaced with
/// escalating warnings instead of backpressure.
#[derive(Clone)]
pub(crate) struct ModuleInbox {
    tx: mpsc::UnboundedSender<ModuleMessage>,
    depth: Arc<AtomicUsize>,
    warn_at: Arc<AtomicUsize>,
}

impl ModuleInbox {
    fn new(tx: mpsc::UnboundedSender<ModuleMessage>) -> Self {
        Self {
            tx,
            depth: Arc::new(AtomicUsize::new(0)),
            warn_at: Arc::new(AtomicUsize::new(QUEUE_HIGH_WATER)),
        }
    }

    /// Send a message, counting it toward the queue depth. The module loop
    /// decrements the depth as it dequeues, so the check runs on the sender
    /// side — a receiver-side check would stay silent exactly when it
    /// matters, while the loop is stuck inside a slow `process()`.
    fn send(&self, module_id: &str, message: ModuleMessage) -> Result<()> {
        self.tx
            .send(message)
            .map_err(|_| Error::SendMessageFailed("Failed to send input message".to_string()))?;
        let depth = self.depth.fetch_add(1, Ordering::Relaxed) + 1;
        // Depth races with concurrent sends and dequeues; the worst case is
        // a duplicated or skipped log line, so Relaxed everywhere is fine.
        let warn_at = self.warn_at.load(Ordering::Relaxed);
        if depth >= warn_at {
            log::warn!(
                "Module {} inbox depth reached {} (unbounded queue, consumer falling behind)",
                module_id,
                depth
            );
            let mut next = warn_at;
            while next <= depth {
                next *= 2;
            }
            self.warn_at.store(next, Ordering::Relaxed);
        } else if depth < QUEUE_HIGH_WATER / 2 && warn_at > QUEUE_HIGH_WATER {
            self.warn_at.store(QUEUE_HIGH_WATER, Ordering::Relaxed);
        }
        Ok(())
    }
}

/// The central orchestrator for the modular agent system.
///
/// `ModularAgent` manages module lifecycle, connections, and message routing.
/// It maintains module instances, connection maps, and handles [`ModularAgentEvent`]s.
///
/// # Lifecycle
///
/// 1. [`init()`](Self::init) - Create instance and register module definitions
/// 2. [`ready()`](Self::ready) - Start the internal message loop
/// 3. Load patches with [`open_patch_from_file()`](Self::open_patch_from_file) or [`add_patch()`](Self::add_patch)
/// 4. [`start_patch()`](Self::start_patch) - Start modules in a patch
/// 5. Interact via [`write_external_input()`](Self::write_external_input) and [`subscribe()`](Self::subscribe)
/// 6. [`stop_patch()`](Self::stop_patch) - Stop modules
/// 7. [`shutdown()`](Self::shutdown) - Stop remaining patches, wait for the
///    spawned tasks to finish, and release external resources ([`quit()`](Self::quit)
///    is the lightweight variant for tests and simple programs)
///
/// Restarting with [`ready()`](Self::ready) after `shutdown()` is not supported;
/// create a new instance instead.
///
/// # Example
///
#[cfg_attr(feature = "file", doc = "```rust,no_run")]
#[cfg_attr(not(feature = "file"), doc = "```rust,no_run,ignore")]
/// use modular_agent_core::{ModularAgent, Value, ModularAgentEvent};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Initialize and start
///     let ma = ModularAgent::init()?;
///     ma.ready().await?;
///
///     // Load a patch
///     let patch_id = ma.open_patch_from_file("my_patch.json", None).await?;
///     ma.start_patch(&patch_id).await?;
///
///     // Send external input
///     ma.write_external_input("input".to_string(), Value::string("hello")).await?;
///
///     // Cleanup
///     ma.shutdown(std::time::Duration::from_secs(5)).await?;
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct ModularAgent {
    // module id -> module
    pub(crate) modules: Arc<Mutex<FnvIndexMap<String, SharedModule>>>,

    // module id -> inbox sender
    pub(crate) module_txs: Arc<Mutex<FnvIndexMap<String, ModuleInbox>>>,

    // channel name -> [external input module id]
    pub(crate) external_input_modules: Arc<Mutex<FnvIndexMap<String, Vec<String>>>>,

    // channel name -> value
    pub(crate) external_values: Arc<Mutex<FnvIndexMap<String, Value>>>,

    // source module id -> [connection targets]
    pub(crate) connections: Arc<Mutex<FnvIndexMap<String, Vec<ConnectionTarget>>>>,

    // module def name -> module definition
    pub(crate) defs: Arc<Mutex<ModuleDefinitions>>,

    // patches (patch id -> patch)
    pub(crate) patches: Arc<Mutex<FnvIndexMap<String, Arc<AsyncMutex<Patch>>>>>,

    /// name -> patch id: the single source of truth for patch name lookup
    /// and uniqueness. Mutated only by `add_patch_raw`, `rename_patch`,
    /// and `remove_patch`.
    ///
    /// Lock order: never acquire `patches` or a patch's async mutex while
    /// holding this lock.
    pub(crate) patch_names: Arc<Mutex<FnvIndexMap<String, String>>>,

    // module def name -> config
    pub(crate) global_configs_map: Arc<Mutex<FnvIndexMap<String, ModuleConfigs>>>,

    // patch id -> parent cancellation token for the patch's modules
    pub(crate) patch_tokens: Arc<Mutex<FnvIndexMap<String, CancellationToken>>>,

    // module id -> (loop generation, current cancellation token of that loop)
    pub(crate) module_tokens: Arc<Mutex<FnvIndexMap<String, (u64, CancellationToken)>>>,

    // context id -> cancellation token (weak: dies with the flow's contexts)
    pub(crate) context_tokens: Arc<Mutex<FnvIndexMap<usize, Weak<CancellationToken>>>>,

    // message sender
    pub(crate) tx: Arc<Mutex<Option<mpsc::UnboundedSender<ModuleEventMessage>>>>,

    // observers
    pub(crate) observers: broadcast::Sender<EventEnvelope>,

    /// Tracks the message loop, module loops, and event forwarders so
    /// `shutdown` can wait for them.
    pub(crate) tasks: TaskTracker,

    /// Cancelled by `shutdown`; event forwarders watch a child of it.
    pub(crate) shutdown_token: CancellationToken,

    /// Origin tag stamped onto the [`EventEnvelope`] of every event emitted
    /// through this handle. Carried per clone (not shared) so tagged entry
    /// points can coexist with the untagged handles produced by `base()`.
    pub(crate) origin: Option<Arc<str>>,
}

impl Default for ModularAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl ModularAgent {
    /// Create a new `ModularAgent` instance without registering modules.
    ///
    /// For most use cases, prefer [`init()`](Self::init) which also registers
    /// all module definitions from the inventory.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            modules: Default::default(),
            module_txs: Default::default(),
            external_input_modules: Default::default(),
            external_values: Default::default(),
            connections: Default::default(),
            defs: Default::default(),
            patches: Default::default(),
            patch_names: Default::default(),
            global_configs_map: Default::default(),
            patch_tokens: Default::default(),
            module_tokens: Default::default(),
            context_tokens: Default::default(),
            tx: Arc::new(Mutex::new(None)),
            observers: tx,
            tasks: TaskTracker::new(),
            shutdown_token: CancellationToken::new(),
            origin: None,
        }
    }

    /// Returns a clone of this handle that stamps `origin` onto the
    /// [`EventEnvelope`] of every event emitted through it.
    ///
    /// Use this to attribute changes made through a specific entry point
    /// (e.g. a host UI or an external editing server) so subscribers can
    /// distinguish them from runtime-originated events, which carry `None`.
    pub fn with_origin(&self, origin: impl Into<Arc<str>>) -> Self {
        Self {
            origin: Some(origin.into()),
            ..self.clone()
        }
    }

    /// Returns a clone of this handle with no origin tag.
    ///
    /// Invariant: every handle stored beyond the current call (module data,
    /// spawned loops) must be created through this method. Otherwise runtime
    /// events emitted later would be attributed to whichever tagged entry
    /// point happened to create the module or loop.
    pub(crate) fn base(&self) -> Self {
        Self {
            origin: None,
            ..self.clone()
        }
    }

    pub(crate) fn tx(&self) -> Result<mpsc::UnboundedSender<ModuleEventMessage>> {
        self.tx.lock().clone().ok_or(Error::TxNotInitialized)
    }

    /// Initialize a new `ModularAgent` instance.
    ///
    /// This creates a new `ModularAgent` and registers all available module definitions
    /// from the inventory. Call [`ready`](Self::ready) after this to start the message loop.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use modular_agent_core::ModularAgent;
    ///
    /// let ma = ModularAgent::init().unwrap();
    /// ```
    pub fn init() -> Result<Self> {
        let ma = Self::new();
        ma.register_modules();
        Ok(ma)
    }

    fn register_modules(&self) {
        registry::register_inventory_modules(self);
    }

    /// Start the internal message loop.
    ///
    /// This must be called after [`init`](Self::init) before loading patches or sending messages.
    /// The message loop handles routing between modules and external output events.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use modular_agent_core::ModularAgent;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let ma = ModularAgent::init().unwrap();
    ///     ma.ready().await.unwrap(); // Start the message loop
    /// }
    /// ```
    pub async fn ready(&self) -> Result<()> {
        self.spawn_message_loop().await?;
        Ok(())
    }

    /// Stop the internal message loop without waiting for anything.
    ///
    /// Intended for tests and simple programs. Call [`stop_patch`](Self::stop_patch)
    /// for each running patch before calling this method.
    ///
    /// This neither waits for spawned tasks to finish nor releases external resources
    /// such as MCP server child processes. Production code should call
    /// [`shutdown`](Self::shutdown) instead.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use modular_agent_core::ModularAgent;
    /// # async fn example(ma: ModularAgent, patch_id: &str) {
    /// // Stop all patches first
    /// ma.stop_patch(patch_id).await.unwrap();
    /// // Then quit
    /// ma.quit();
    /// # }
    /// ```
    pub fn quit(&self) {
        let mut tx_lock = self.tx.lock();
        *tx_lock = None;
    }

    /// Gracefully shut down the `ModularAgent` and release external resources.
    ///
    /// Within `timeout`, this stops every running patch (calling each module's
    /// [`stop()`](crate::AsModule::stop)), stops the internal message loop, cancels
    /// event forwarders created by [`subscribe_to_event`](Self::subscribe_to_event),
    /// and waits for all of those tasks to finish. Afterwards, whether or not the
    /// timeout elapsed, pooled MCP server connections are closed so their child
    /// processes do not leak.
    ///
    /// Returns [`Error::ShutdownTimeout`] when the tasks did not finish in time.
    /// Events already forwarded to a `subscribe_to_event` receiver remain readable
    /// with `try_recv` after shutdown.
    ///
    /// Works even if [`ready`](Self::ready) was never called. Restarting with
    /// `ready()` afterwards is not supported; create a new instance instead.
    /// Do not start patches concurrently with shutdown: tasks spawned after the
    /// wait completes are not waited for.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use modular_agent_core::ModularAgent;
    /// # use std::time::Duration;
    /// # async fn example(ma: ModularAgent) {
    /// ma.shutdown(Duration::from_secs(5)).await.unwrap();
    /// # }
    /// ```
    pub async fn shutdown(&self, timeout: Duration) -> Result<()> {
        let result = tokio::time::timeout(timeout, async {
            let patch_ids: Vec<String> = self.patches.lock().keys().cloned().collect();
            for id in patch_ids {
                let running = match self.get_patch(&id) {
                    Some(patch) => patch.lock().await.running(),
                    None => false,
                };
                if running && let Err(e) = self.stop_patch(&id).await {
                    log::error!("Failed to stop patch {} during shutdown: {}", id, e);
                }
            }
            self.quit();
            self.shutdown_token.cancel();
            self.tasks.close();
            self.tasks.wait().await;
        })
        .await;

        // Runs even after a timeout: leaked MCP child processes are worse than
        // a late return, and the pool enforces its own bound on this.
        #[cfg(feature = "mcp")]
        crate::mcp::shutdown_all_mcp_connections().await?;

        result.map_err(|_| Error::ShutdownTimeout(timeout))
    }

    // Patch management

    /// Create a new empty patch.
    ///
    /// Returns the id of the new patch. The patch is created with default settings
    /// and contains no modules or connections initially.
    pub fn new_patch(&self) -> Result<String> {
        let spec = PatchSpec::default();
        let id = self.add_patch(spec)?;
        Ok(id)
    }

    /// Create a new empty patch with the given name.
    ///
    /// Returns the id of the new patch.
    pub fn new_patch_with_name(&self, name: String) -> Result<String> {
        let spec = PatchSpec::default();
        let id = self.add_patch_with_name(spec, name)?;
        Ok(id)
    }

    /// Get a patch by id.
    ///
    /// Returns `None` if no patch exists with the given id.
    pub fn get_patch(&self, id: &str) -> Option<Arc<AsyncMutex<Patch>>> {
        let patches = self.patches.lock();
        patches.get(id).cloned()
    }

    /// Find the id of a live patch by its name.
    ///
    /// Returns `None` when no patch with the given name is loaded.
    pub fn find_patch_id_by_name(&self, name: &str) -> Option<String> {
        let names = self.patch_names.lock();
        names.get(name).cloned()
    }

    /// Add a new patch with the given spec, and returns the id of the new patch.
    ///
    /// The ids of the given spec, including modules and connections, are changed to new unique ids.
    /// This allows the same spec to be added multiple times without id conflicts.
    pub fn add_patch(&self, spec: PatchSpec) -> Result<String> {
        self.add_patch_raw(spec, None)
    }

    /// Add a new patch with the given name and spec, and returns the id of the new patch.
    ///
    /// The ids of the given spec, including modules and connections, are changed to new unique ids.
    pub fn add_patch_with_name(&self, spec: PatchSpec, name: String) -> Result<String> {
        self.add_patch_raw(spec, Some(name))
    }

    fn add_patch_raw(&self, spec: PatchSpec, name: Option<String>) -> Result<String> {
        let mut patch = Patch::new(spec);
        if let Some(name) = &name {
            patch.set_name(name.clone());
        }
        let id = patch.id().to_string();

        // Reserve the name first so a duplicate fails before any modules are
        // created; the reservation is rolled back if a later step fails.
        if let Some(name) = &name {
            let mut names = self.patch_names.lock();
            if names.contains_key(name) {
                return Err(Error::PatchNameExists(name.clone()));
            }
            names.insert(name.clone(), id.clone());
        }

        // add modules
        for module in &patch.spec().modules {
            if let Err(e) = self.add_module_internal(id.clone(), module.clone()) {
                log::error!("Failed to add_module {}: {}", module.id, e);
            }
        }

        // add connections
        for connection in &patch.spec().connections {
            self.add_connection_internal(connection.clone())
                .unwrap_or_else(|e| {
                    log::error!("Failed to add_connection {}: {}", connection.source, e);
                });
        }

        // add the given patch into patches
        let inserted = {
            let mut patches = self.patches.lock();
            if patches.contains_key(&id) {
                false
            } else {
                patches.insert(id.clone(), Arc::new(AsyncMutex::new(patch)));
                true
            }
        };
        if !inserted {
            if let Some(name) = &name {
                self.patch_names.lock().swap_remove(name);
            }
            return Err(Error::DuplicateId(id));
        }

        self.emit_patch_added(id.clone(), name);

        Ok(id)
    }

    /// Rename a patch by id.
    ///
    /// Fails with [`Error::PatchNameExists`] when another patch
    /// already uses `new_name`. Renaming a patch to its current name is a
    /// no-op and succeeds. Emits [`ModularAgentEvent::PatchRenamed`].
    pub async fn rename_patch(&self, id: &str, new_name: String) -> Result<()> {
        let patch = self
            .get_patch(id)
            .ok_or_else(|| Error::PatchNotFound(id.to_string()))?;

        {
            let mut names = self.patch_names.lock();
            if let Some(owner) = names.get(&new_name)
                && owner != id
            {
                return Err(Error::PatchNameExists(new_name));
            }
            // Remove by id so a previously unnamed patch gaining its first
            // name is handled too.
            names.retain(|_, v| v != id);
            names.insert(new_name.clone(), id.to_string());
        }

        // Re-check liveness after reserving the name: a concurrent
        // remove_patch may have completed (including its name-index
        // cleanup) between get_patch above and the insert, which would
        // leave the new entry pointing at a dead id forever. The lock-order
        // rule (never take `patches` while holding `patch_names`) forces
        // this check to come after the insert; either remove_patch's
        // cleanup runs after our insert and clears it, or we observe the id
        // gone here and roll the reservation back.
        if !self.patches.lock().contains_key(id) {
            let mut names = self.patch_names.lock();
            if names.get(&new_name).is_some_and(|owner| owner == id) {
                names.swap_remove(&new_name);
            }
            return Err(Error::PatchNotFound(id.to_string()));
        }

        let old_name = {
            let mut patch = patch.lock().await;
            let old_name = patch.name().map(str::to_string);
            patch.set_name(new_name.clone());
            old_name
        };
        self.emit_patch_renamed(id.to_string(), old_name, new_name);
        Ok(())
    }

    /// Remove a patch by id.
    ///
    /// Stops the patch if running, then removes all associated modules and connections.
    /// Emits [`ModularAgentEvent::PatchRemoved`] after teardown.
    pub async fn remove_patch(&self, id: &str) -> Result<()> {
        let patch = self
            .get_patch(id)
            .ok_or_else(|| Error::PatchNotFound(id.to_string()))?;

        let mut patch = patch.lock().await;
        let name = patch.name().map(str::to_string);
        patch.stop(self).await.unwrap_or_else(|e| {
            log::error!("Failed to stop patch {}: {}", id, e);
        });

        // Remove all modules and connections associated with the patch
        for module in &patch.spec().modules {
            self.remove_module_internal(&module.id)
                .await
                .unwrap_or_else(|e| {
                    log::error!("Failed to remove_module {}: {}", module.id, e);
                });
        }
        for connection in &patch.spec().connections {
            self.remove_connection_internal(connection);
        }

        // Drop the patch lock before modifying the patches map
        drop(patch);

        // Remove the patch entry from the map
        {
            let mut patches = self.patches.lock();
            patches.swap_remove(id);
        }
        self.patch_names.lock().retain(|_, v| v != id);
        self.remove_patch_token(id);

        self.emit_patch_removed(id.to_string(), name);

        Ok(())
    }

    /// Start a patch by id.
    ///
    /// This starts all modules in the patch, enabling message flow between them.
    /// Each module's [`start()`](crate::AsModule::start) method is called.
    ///
    /// Emits [`ModularAgentEvent::PatchStarted`] when the patch was not
    /// already running.
    pub async fn start_patch(&self, id: &str) -> Result<()> {
        let patch = self
            .get_patch(id)
            .ok_or_else(|| Error::PatchNotFound(id.to_string()))?;
        // Emit outside the patch lock so observers cannot deadlock against it.
        let started = {
            let mut patch = patch.lock().await;
            let was_running = patch.running();
            patch.start(self).await?;
            !was_running && patch.running()
        };
        if started {
            self.emit_patch_started(id.to_string());
        }

        Ok(())
    }

    /// Stop a patch by id.
    ///
    /// This stops all modules in the patch, terminating message processing.
    /// Each module's [`stop()`](crate::AsModule::stop) method is called.
    ///
    /// Emits [`ModularAgentEvent::PatchStopped`] when the patch was running.
    pub async fn stop_patch(&self, id: &str) -> Result<()> {
        let patch = self
            .get_patch(id)
            .ok_or_else(|| Error::PatchNotFound(id.to_string()))?;
        let stopped = {
            let mut patch = patch.lock().await;
            let was_running = patch.running();
            patch.stop(self).await?;
            was_running && !patch.running()
        };
        if stopped {
            self.emit_patch_stopped(id.to_string());
        }

        Ok(())
    }

    /// Open a patch from a JSON file.
    ///
    /// Reads the file, parses the JSON as a [`PatchSpec`], and adds it to the system.
    /// Optionally provide a custom name for the patch.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON patch file
    /// * `name` - Optional custom name for the patch
    #[cfg(feature = "file")]
    pub async fn open_patch_from_file(&self, path: &str, name: Option<String>) -> Result<String> {
        let json_str = std::fs::read_to_string(path).map_err(|e| Error::IoError(e.to_string()))?;
        let spec = PatchSpec::from_json(&json_str)?;
        let id = self.add_patch_raw(spec, name)?;
        Ok(id)
    }

    /// Save a patch to a JSON file.
    ///
    /// Serializes the current patch state (including module configs) to JSON
    /// and writes it to the specified path. Emits
    /// [`ModularAgentEvent::PatchSaved`] when the patch has a name; unnamed
    /// patches have no list entry to refresh, so no event is emitted for them.
    #[cfg(feature = "file")]
    pub async fn save_patch(&self, id: &str, path: &str) -> Result<()> {
        let Some(patch_spec) = self.get_patch_spec(id).await else {
            return Err(Error::PatchNotFound(id.to_string()));
        };
        let json_str = patch_spec.to_json()?;
        std::fs::write(path, json_str).map_err(|e| Error::IoError(e.to_string()))?;
        if let Some(name) = self.get_patch_info(id).await.and_then(|info| info.name) {
            self.emit_patch_saved(id.to_string(), name);
        }
        Ok(())
    }

    // PatchSpec

    /// Get the current patch spec by id.
    pub async fn get_patch_spec(&self, id: &str) -> Option<PatchSpec> {
        let patch = self.get_patch(id)?;
        let mut patch_spec = {
            let patch = patch.lock().await;
            patch.spec().clone()
        };

        // Overlay live module specs onto the stored entries. A module whose
        // definition is not registered in this build has no live instance;
        // keep its stored spec so it survives the editor round-trip and the
        // save that follows (save_patch writes exactly what this returns).
        for module in &mut patch_spec.modules {
            if let Some(spec) = self.get_module_spec(&module.id).await {
                *module = spec;
            }
        }

        // No need to change connections

        Some(patch_spec)
    }

    /// Update the patch spec
    pub async fn update_patch_spec(&self, id: &str, value: &JsonValue) -> Result<()> {
        let patch = self
            .get_patch(id)
            .ok_or_else(|| Error::PatchNotFound(id.to_string()))?;
        let mut patch = patch.lock().await;
        patch.update_spec(value)?;
        drop(patch);
        self.emit_patch_structure_changed(id.to_string());
        Ok(())
    }

    // PatchInfo

    /// Get info of the patch by id.
    pub async fn get_patch_info(&self, id: &str) -> Option<PatchInfo> {
        let patch = self.get_patch(id)?;
        Some(PatchInfo::from(&*patch.lock().await))
    }

    /// Get infos of all patches.
    pub async fn get_patch_infos(&self) -> Vec<PatchInfo> {
        let patches = {
            let patches = self.patches.lock();
            patches.values().cloned().collect::<Vec<_>>()
        };
        let mut patch_infos = Vec::new();
        for patch in patches {
            let patch_guard = patch.lock().await;
            patch_infos.push(PatchInfo::from(&*patch_guard));
        }
        patch_infos
    }

    // Modules

    /// Register a module definition.
    ///
    /// This makes the module type available for use in patches. The definition
    /// includes metadata (title, category), input/output ports, and config specs.
    ///
    /// Note: Modules using `#[modular_agent]` macro are registered automatically via inventory.
    pub fn register_module_definition(&self, def: ModuleDefinition) {
        let def_name = def.name.clone();
        let def_global_configs = def.global_configs.clone();

        let mut defs = self.defs.lock();
        defs.insert(def.name.clone(), def);

        // if there is a global config, set it
        if let Some(def_global_configs) = def_global_configs {
            let mut new_configs = ModuleConfigs::default();
            for (key, config_entry) in def_global_configs.iter() {
                new_configs.set(key.clone(), config_entry.value.clone());
            }
            self.set_global_configs(def_name, new_configs);
        }
    }

    /// Get all registered module definitions.
    ///
    /// Returns a map of definition name to [`ModuleDefinition`].
    pub fn get_module_definitions(&self) -> ModuleDefinitions {
        let defs = self.defs.lock();
        defs.clone()
    }

    /// Get a module definition by name.
    ///
    /// The name is typically in the format `module::path::StructName`.
    pub fn get_module_definition(&self, def_name: &str) -> Option<ModuleDefinition> {
        let defs = self.defs.lock();
        defs.get(def_name).cloned()
    }

    /// Get the config specs of a module definition by name.
    pub fn get_module_config_specs(&self, def_name: &str) -> Option<ModuleConfigSpecs> {
        let defs = self.defs.lock();
        let def = defs.get(def_name)?;
        def.configs.clone()
    }

    /// Get the module spec by id.
    pub async fn get_module_spec(&self, module_id: &str) -> Option<ModuleSpec> {
        let module = {
            let modules = self.modules.lock();
            modules.get(module_id)?.clone()
        };
        let module = module.lock().await;
        Some(module.spec().clone())
    }

    /// Look up the stored patch spec entry of a module by id.
    ///
    /// Unlike [`Self::get_module_spec`] this also finds spec-only modules
    /// (whose definition is not registered in this build), which have no
    /// live instance. For a live module it returns the stored entry, not the
    /// instance spec.
    pub(crate) async fn find_stored_module_spec(&self, module_id: &str) -> Option<ModuleSpec> {
        let patches = {
            let patches = self.patches.lock();
            patches.values().cloned().collect::<Vec<_>>()
        };
        for patch in patches {
            let patch = patch.lock().await;
            if let Some(module) = patch.spec().modules.iter().find(|a| a.id == module_id) {
                return Some(module.clone());
            }
        }
        None
    }

    /// Update the module spec by id.
    ///
    /// A patch containing `configs` calls the module's
    /// [`AsModule::configs_changed`], so modules that derive ports or further
    /// configs from their config values rebuild them; an error it reports is
    /// propagated, as with [`ModularAgent::set_module_configs`].
    ///
    /// Emits [`ModularAgentEvent::ModuleSpecUpdated`], and additionally
    /// [`ModularAgentEvent::PatchStructureChanged`] when the patch contains
    /// keys other than `configs`. The events are emitted even when an error
    /// is returned: the module may have committed the patch before failing
    /// (`configs_changed` runs after the merge), and a spec change must never
    /// go unannounced to hosts.
    ///
    /// A module with no live instance (its definition is not registered in
    /// this build) is patched in the patch spec that holds it, with the same
    /// events; [`Error::ModuleNotFound`] is returned only when no patch
    /// holds the id either.
    pub async fn update_module_spec(&self, module_id: &str, value: &JsonValue) -> Result<()> {
        let module = {
            let modules = self.modules.lock();
            modules.get(module_id).cloned()
        };
        let Some(module) = module else {
            // No live instance: the module may still exist as a spec-only
            // entry, whose stored spec is the only place a patch can land.
            return self.update_spec_only_module(module_id, value).await;
        };
        let (patch_id, updated) = {
            let mut module = module.lock().await;
            let updated = module.update_spec(value);
            (module.patch_id().to_string(), updated)
        };

        // A failure may have left the patch committed (a module can reject a
        // value in configs_changed after storing it, as Switch does with an
        // unparsable condition), so announce first and propagate after: a
        // spurious refresh is harmless, an unannounced spec change is not.
        self.emit_module_spec_updated(module_id.to_string());

        if is_structural_spec_patch(value) {
            self.emit_patch_structure_changed(patch_id);
        }
        updated
    }

    /// Patch the stored spec entry of a module that has no live instance.
    ///
    /// A spec-only module (its definition is not registered in this build)
    /// never got instantiated, so the patch spec is the only place its
    /// layout, ports or configs can be recorded. The event contract matches
    /// the live path so hosts cannot tell the two apart.
    async fn update_spec_only_module(&self, module_id: &str, value: &JsonValue) -> Result<()> {
        let Some((patch_id, updated)) = self.patch_stored_module_spec(module_id, value).await
        else {
            return Err(Error::ModuleNotFound(module_id.to_string()));
        };

        // A rejected key can follow keys that were already merged, so a
        // failed patch still has to announce the change.
        self.emit_module_spec_updated(module_id.to_string());
        if is_structural_spec_patch(value) {
            self.emit_patch_structure_changed(patch_id);
        }
        updated
    }

    /// Applies a patch to a module's stored spec entry, emitting no events.
    ///
    /// Returns the id of the patch that holds the module together with the
    /// patch result, or `None` when no patch spec contains the id. The
    /// patch id is returned even when the patch failed, so callers can
    /// still announce a partially merged change.
    async fn patch_stored_module_spec(
        &self,
        module_id: &str,
        value: &JsonValue,
    ) -> Option<(String, Result<()>)> {
        // Take a snapshot and release the patches lock: a patch's async
        // mutex must never be awaited while the sync map lock is held.
        let patches = {
            let patches = self.patches.lock();
            patches.values().cloned().collect::<Vec<_>>()
        };

        for patch in patches {
            // One patch at a time, so no two patch locks are ever held.
            let mut patch = patch.lock().await;
            match patch.update_module_spec(module_id, value) {
                Ok(false) => continue,
                result => return Some((patch.id().to_string(), result.map(|_| ()))),
            }
        }
        None
    }

    /// Create a new module spec from the given module definition name.
    pub fn new_module_spec(&self, def_name: &str) -> Result<ModuleSpec> {
        let def = self
            .get_module_definition(def_name)
            .ok_or_else(|| Error::ModuleDefinitionNotFound(def_name.to_string()))?;
        Ok(def.to_spec())
    }

    /// Add a module to the specified patch.
    ///
    /// Creates a new module instance from the given spec and adds it to the patch.
    /// Returns the id of the newly created module. The module is not started automatically;
    /// call [`start_patch`](Self::start_patch) or [`start_module`](Self::start_module) to start it.
    pub async fn add_module(&self, patch_id: String, mut spec: ModuleSpec) -> Result<String> {
        let patch = self
            .get_patch(&patch_id)
            .ok_or_else(|| Error::PatchNotFound(patch_id.to_string()))?;

        let id = new_id();
        spec.id = id.clone();
        // Register the constructed spec: new() may have generated dynamic
        // configs/ports via update_spec, and the patch must reflect them.
        let constructed = self.add_module_internal(patch_id.clone(), spec)?;

        let mut patch = patch.lock().await;
        patch.add_module(constructed);
        drop(patch);

        self.emit_patch_structure_changed(patch_id);

        Ok(id)
    }

    fn add_module_internal(&self, patch_id: String, spec: ModuleSpec) -> Result<ModuleSpec> {
        let mut modules = self.modules.lock();
        if modules.contains_key(&spec.id) {
            return Err(Error::ModuleAlreadyExists(spec.id.to_string()));
        }
        let spec_id = spec.id.clone();
        // base(): the module keeps this handle for its lifetime, so runtime
        // events it emits later must not inherit the creator's origin tag.
        let mut module = module_new(self.base(), spec_id.clone(), spec)?;
        module.set_patch_id(patch_id);
        let constructed = module.spec().clone();
        modules.insert(spec_id, Arc::new(AsyncMutex::new(module)));
        Ok(constructed)
    }

    /// Get the module by id.
    pub fn get_module(&self, module_id: &str) -> Option<SharedModule> {
        let modules = self.modules.lock();
        modules.get(module_id).cloned()
    }

    /// Add a connection between two modules in the specified patch.
    ///
    /// When the source module outputs a value on the source handle (port),
    /// it will be delivered to the target module's target handle (port).
    pub async fn add_connection(&self, patch_id: &str, connection: ConnectionSpec) -> Result<()> {
        // check if the source and target modules exist
        {
            let modules = self.modules.lock();
            if !modules.contains_key(&connection.source) {
                return Err(Error::ModuleNotFound(connection.source.to_string()));
            }
            if !modules.contains_key(&connection.target) {
                return Err(Error::ModuleNotFound(connection.target.to_string()));
            }
        }

        // check if handles are valid
        if connection.source_handle.is_empty() {
            return Err(Error::EmptySourceHandle);
        }
        if connection.target_handle.is_empty() {
            return Err(Error::EmptyTargetHandle);
        }

        let patch = self
            .get_patch(patch_id)
            .ok_or_else(|| Error::PatchNotFound(patch_id.to_string()))?;
        let mut patch = patch.lock().await;
        // Register the routing entry first: it is the fallible step
        // (duplicate detection), and a failure must leave the patch spec
        // untouched so no spec change ever goes unannounced.
        self.add_connection_internal(connection.clone())?;
        patch.add_connection(connection);
        drop(patch);
        self.emit_patch_structure_changed(patch_id.to_string());
        Ok(())
    }

    fn add_connection_internal(&self, connection: ConnectionSpec) -> Result<()> {
        let mut connections = self.connections.lock();
        if let Some(targets) = connections.get_mut(&connection.source) {
            if targets
                .iter()
                .any(|(target, source_handle, target_handle)| {
                    *target == connection.target
                        && *source_handle == connection.source_handle
                        && *target_handle == connection.target_handle
                })
            {
                return Err(Error::ConnectionAlreadyExists);
            }
            targets.push((
                connection.target,
                connection.source_handle,
                connection.target_handle,
            ));
        } else {
            connections.insert(
                connection.source,
                vec![(
                    connection.target,
                    connection.source_handle,
                    connection.target_handle,
                )],
            );
        }
        Ok(())
    }

    /// Returns true if any connection originates from `source_module`'s `port`.
    ///
    /// Producers can use this to skip building expensive values for ports
    /// nobody listens to; `module_out` would only drop them after the
    /// conversion cost has already been paid.
    pub fn has_connections(&self, source_module: &str, port: &str) -> bool {
        let connections = self.connections.lock();
        connections.get(source_module).is_some_and(|targets| {
            targets
                .iter()
                .any(|(_, source_port, _)| source_port == port)
        })
    }

    /// Add modules and connections to the specified patch.
    ///
    /// The ids of the given modules and connections are changed to new unique ids.
    /// The modules are not started automatically, even if the patch is running.
    pub async fn add_modules_and_connections(
        &self,
        patch_id: &str,
        modules: &Vec<ModuleSpec>,
        connections: &Vec<ConnectionSpec>,
    ) -> Result<(Vec<ModuleSpec>, Vec<ConnectionSpec>)> {
        let (modules, connections) = update_ids(modules, connections);

        let patch = self
            .get_patch(patch_id)
            .ok_or_else(|| Error::PatchNotFound(patch_id.to_string()))?;
        let mut patch = patch.lock().await;

        // Track progress so a mid-batch failure can be rolled back: a
        // partial batch must not leave modules in the spec (or the runtime
        // maps) while returning an error without any event.
        let mut added_modules = 0;
        let mut added_connections = 0;
        let mut result = Ok(());

        // Collect the constructed specs (with dynamic configs/ports from
        // new()) so the patch and the caller both see the real state.
        let mut constructed_modules = Vec::with_capacity(modules.len());
        for module in &modules {
            match self.add_module_internal(patch_id.to_string(), module.clone()) {
                Ok(constructed) => {
                    patch.add_module(constructed.clone());
                    constructed_modules.push(constructed);
                    added_modules += 1;
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }

        if result.is_ok() {
            for connection in &connections {
                if let Err(e) = self.add_connection_internal(connection.clone()) {
                    result = Err(e);
                    break;
                }
                patch.add_connection(connection.clone());
                added_connections += 1;
            }
        }

        if let Err(e) = result {
            for connection in connections.iter().take(added_connections) {
                patch.remove_connection(connection);
                self.remove_connection_internal(connection);
            }
            // The rolled-back modules were never started, so no stop or
            // channel teardown is needed; dropping the map entries undoes
            // add_module_internal completely.
            let mut modules_map = self.modules.lock();
            for module in modules.iter().take(added_modules) {
                patch.remove_module(&module.id);
                modules_map.swap_remove(&module.id);
            }
            return Err(e);
        }
        drop(patch);

        self.emit_patch_structure_changed(patch_id.to_string());

        Ok((constructed_modules, connections))
    }

    /// Remove a module from the specified patch.
    ///
    /// If the module is running, it will be stopped first.
    pub async fn remove_module(&self, patch_id: &str, module_id: &str) -> Result<()> {
        let patch = self
            .get_patch(patch_id)
            .ok_or_else(|| Error::PatchNotFound(patch_id.to_string()))?;

        // Tear down the runtime instance before touching the spec so a
        // failure leaves the spec unchanged and no spec change ever goes
        // unannounced. A module can exist in the spec without a runtime
        // instance (its definition was unknown when the patch was added);
        // such a module is still removable from the spec.
        let runtime_removed = match self.remove_module_internal(module_id).await {
            Ok(()) => true,
            Err(Error::ModuleNotFound(_)) => false,
            Err(e) => return Err(e),
        };

        let spec_removed = {
            let mut patch = patch.lock().await;
            let count_before = patch.spec().modules.len();
            patch.remove_module(module_id);
            patch.spec().modules.len() != count_before
        };

        if !runtime_removed && !spec_removed {
            return Err(Error::ModuleNotFound(module_id.to_string()));
        }
        self.emit_patch_structure_changed(patch_id.to_string());
        Ok(())
    }

    async fn remove_module_internal(&self, module_id: &str) -> Result<()> {
        self.stop_module(module_id).await?;

        // remove from connections
        {
            let mut connections = self.connections.lock();
            let mut sources_to_remove = Vec::new();
            for (source, targets) in connections.iter_mut() {
                targets.retain(|(target, _, _)| target != module_id);
                if targets.is_empty() {
                    sources_to_remove.push(source.clone());
                }
            }
            for source in sources_to_remove {
                connections.swap_remove(&source);
            }
            connections.swap_remove(module_id);
        }

        // remove from modules
        {
            let mut modules = self.modules.lock();
            modules.swap_remove(module_id);
        }

        Ok(())
    }

    /// Remove a connection from the specified patch.
    pub async fn remove_connection(
        &self,
        patch_id: &str,
        connection: &ConnectionSpec,
    ) -> Result<()> {
        let patch = self
            .get_patch(patch_id)
            .ok_or_else(|| Error::PatchNotFound(patch_id.to_string()))?;
        let mut patch = patch.lock().await;
        let Some(connection) = patch.remove_connection(connection) else {
            return Err(Error::ConnectionNotFound(format!(
                "{}:{}->{}:{}",
                connection.source,
                connection.source_handle,
                connection.target,
                connection.target_handle
            )));
        };
        self.remove_connection_internal(&connection);
        drop(patch);
        self.emit_patch_structure_changed(patch_id.to_string());
        Ok(())
    }

    fn remove_connection_internal(&self, connection: &ConnectionSpec) {
        let mut connections = self.connections.lock();
        if let Some(targets) = connections.get_mut(&connection.source) {
            targets.retain(|(target, source_handle, target_handle)| {
                *target != connection.target
                    || *source_handle != connection.source_handle
                    || *target_handle != connection.target_handle
            });
            if targets.is_empty() {
                connections.swap_remove(&connection.source);
            }
        }
    }

    // Cancellation tokens

    /// Returns the parent cancellation token for a patch, creating it if needed.
    fn patch_token(&self, patch_id: &str) -> CancellationToken {
        let mut tokens = self.patch_tokens.lock();
        tokens.entry(patch_id.to_string()).or_default().clone()
    }

    /// Installs a fresh (uncancelled) parent token for a patch.
    ///
    /// A fired `CancellationToken` cannot be reset, so this is called when a
    /// patch starts to replace the token cancelled by a previous stop.
    pub(crate) fn reset_patch_token(&self, patch_id: &str) {
        let mut tokens = self.patch_tokens.lock();
        tokens.insert(patch_id.to_string(), CancellationToken::new());
    }

    /// Cancels the patch's parent token, aborting the in-flight `process()`
    /// of every module in the patch at once.
    ///
    /// The entry is kept (in its cancelled state) for the duration of the
    /// stop sequence so module tokens renewed while modules are still being
    /// stopped are born cancelled and queued inputs are skipped instead of
    /// processed. [`Patch::stop`](crate::patch::Patch::stop) removes the
    /// entry once every module has stopped, so a later `start_module` derives
    /// a live token instead of a child of the fired one.
    pub(crate) fn cancel_patch_token(&self, patch_id: &str) {
        let token = self.patch_tokens.lock().get(patch_id).cloned();
        if let Some(token) = token {
            token.cancel();
        }
    }

    pub(crate) fn remove_patch_token(&self, patch_id: &str) {
        self.patch_tokens.lock().swap_remove(patch_id);
    }

    /// Installs a module loop's inbox and a fresh cancellation token (a
    /// child of its patch's parent token). The returned generation
    /// identifies the module-loop incarnation that owns the slot.
    ///
    /// Both entries go in under one critical section so that
    /// [`release_module_slot`](Self::release_module_slot) never sees an inbox
    /// from one incarnation next to a token from another. Lock order is
    /// module_txs -> module_tokens; nothing else nests these two.
    fn create_module_slot(
        &self,
        patch_id: &str,
        module_id: &str,
        inbox: ModuleInbox,
    ) -> (u64, CancellationToken) {
        let generation = MODULE_TOKEN_GENERATION.fetch_add(1, Ordering::Relaxed);
        let token = self.patch_token(patch_id).child_token();
        let mut module_txs = self.module_txs.lock();
        let mut tokens = self.module_tokens.lock();
        module_txs.insert(module_id.to_string(), inbox);
        tokens.insert(module_id.to_string(), (generation, token.clone()));
        (generation, token)
    }

    /// Releases the inbox and token slot of a module loop whose start()
    /// failed.
    ///
    /// Both entries are removed under one critical section and only while
    /// the token slot still belongs to `generation`: stop_module may already
    /// have removed them, and a restarted module's new loop must keep its
    /// own. Lock order is module_txs -> module_tokens, same as
    /// [`create_module_slot`](Self::create_module_slot).
    fn release_module_slot(&self, module_id: &str, generation: u64) {
        let mut module_txs = self.module_txs.lock();
        let mut tokens = self.module_tokens.lock();
        if matches!(tokens.get(module_id), Some((g, _)) if *g == generation) {
            tokens.swap_remove(module_id);
            module_txs.swap_remove(module_id);
        }
    }

    /// Replaces a fired module token with a fresh child of the patch token.
    ///
    /// Called by the module loop after its token fired. Returns `None` when
    /// the slot no longer belongs to the calling loop — either
    /// [`stop_module`](Self::stop_module) removed the entry, a restarted
    /// module's new loop installed its own token (different generation), or
    /// the whole patch was removed. The caller then keeps its fired token
    /// so queued inputs are skipped until the `Stop` message arrives.
    fn renew_module_token(
        &self,
        patch_id: &str,
        module_id: &str,
        generation: u64,
    ) -> Option<CancellationToken> {
        // Look up (never create) the parent: a lagging loop must not
        // resurrect the token entry of a removed patch.
        let parent = self.patch_tokens.lock().get(patch_id).cloned()?;
        let fresh = parent.child_token();
        let mut tokens = self.module_tokens.lock();
        let slot = tokens.get_mut(module_id)?;
        if slot.0 != generation {
            return None;
        }
        slot.1 = fresh.clone();
        Some(fresh)
    }

    /// Returns the cancellation token for a context, creating it if needed.
    ///
    /// The registry holds `Weak` references: an entry dies when the flow's
    /// last context clone is dropped, so lookups for finished flows fail and
    /// dead entries can be pruned. Pruning starts once the registry reaches
    /// [`CONTEXT_TOKEN_PRUNE_THRESHOLD`], but live entries are never evicted.
    pub(crate) fn context_token(&self, ctx_id: usize) -> Arc<CancellationToken> {
        let mut tokens = self.context_tokens.lock();
        if let Some(token) = tokens.get(&ctx_id).and_then(Weak::upgrade) {
            return token;
        }
        if tokens.len() >= CONTEXT_TOKEN_PRUNE_THRESHOLD {
            tokens.retain(|_, weak| weak.strong_count() > 0);
        }
        let token = Arc::new(CancellationToken::new());
        tokens.insert(ctx_id, Arc::downgrade(&token));
        token
    }

    /// Aborts the flow identified by `ctx_id`.
    ///
    /// Cancels the context's cancellation token, which every module handling
    /// the flow received via [`ModuleContext::cancel_token`]. Cancellation is
    /// cooperative for work already in flight: modules that `select!` on the
    /// token (LLM streaming loops, [`CustomToolModule`](crate::tool::CustomToolModule)
    /// result waits) abort promptly with [`Error::Cancelled`], while
    /// modules that ignore it run to completion. Inputs dispatched after the
    /// token fires are skipped before `process()` is called. The cancelled
    /// token stays alive as long as any context of the flow does, so queued
    /// and cyclic inputs for the flow are skipped too.
    ///
    /// Returns `false` when no live flow is tracked under `ctx_id` (the flow
    /// already finished, or never reached a module): nothing is cancelled.
    pub fn abort_context(&self, ctx_id: usize) -> bool {
        let token = self
            .context_tokens
            .lock()
            .get(&ctx_id)
            .and_then(Weak::upgrade);
        match token {
            Some(token) => {
                token.cancel();
                true
            }
            None => {
                log::warn!("abort_context: no live flow for context {}", ctx_id);
                false
            }
        }
    }

    /// Start a module by id.
    ///
    /// Creates a message channel for the module and spawns its event loop.
    /// The module's [`start()`](crate::AsModule::start) method is called, then
    /// the module begins processing incoming messages. A module whose
    /// `start()` fails goes back to `Init` and its channel is removed, so
    /// later config edits are written to it directly.
    pub async fn start_module(&self, module_id: &str) -> Result<()> {
        let module = {
            let modules = self.modules.lock();
            let Some(a) = modules.get(module_id) else {
                return Err(Error::ModuleNotFound(module_id.to_string()));
            };
            a.clone()
        };
        let (def_name, patch_id) = {
            let module = module.lock().await;
            (module.def_name().to_string(), module.patch_id().to_string())
        };
        if !self.defs.lock().contains_key(&def_name) {
            return Err(Error::ModuleDefinitionNotFound(def_name));
        }
        let module_status = {
            // This will not block since the module is not started yet.
            let module = module.lock().await;
            module.status().clone()
        };
        if module_status == ModuleStatus::Init {
            log::info!("Starting module {}", module_id);

            let (tx, mut rx) = mpsc::unbounded_channel();
            let inbox = ModuleInbox::new(tx);
            let inbox_depth = inbox.depth.clone();

            let module_clone = module.clone();
            let module_id_clone = module_id.to_string();
            // base(): the module loop outlives this call, so it must not
            // stamp runtime events with the caller's origin.
            let ma = self.base();
            // Installed before spawning so stop_module can cancel it immediately.
            let (generation, mut token) = self.create_module_slot(&patch_id, module_id, inbox);

            let module_loop = async move {
                // Race start() against the token too: a start() stuck on
                // slow I/O holds the module lock, and without the race
                // stop_module would block on that lock until start() returns
                // on its own.
                let start = async {
                    let mut module_guard = module_clone.lock().await;
                    module_guard.start().await
                };
                let mut started = true;
                tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        log::info!("Start cancelled: {}", module_id_clone);
                        return;
                    }
                    r = start => {
                        if let Err(e) = r {
                            log::error!("Failed to start module {}: {}", module_id_clone, e);
                            // Status is already back to Init (Module::start
                            // resets it), so configs drained below cannot
                            // re-register runtime resources in
                            // configs_changed. Keep that ordering.
                            ma.release_module_slot(&module_id_clone, generation);
                            // Close first so later sends fail loudly; recv()
                            // still yields everything queued before the
                            // close, then returns None.
                            rx.close();
                            started = false;
                        }
                    }
                }

                while let Some(message) = rx.recv().await {
                    inbox_depth.fetch_sub(1, Ordering::Relaxed);
                    match message {
                        ModuleMessage::Input { .. } if !started => {
                            log::warn!(
                                "Dropping input queued before failed start: {}",
                                module_id_clone
                            );
                        }
                        ModuleMessage::Input { ctx, port, value } => {
                            // Attach the flow's cancellation token so
                            // downstream awaits (tool result waits, LLM
                            // streams) can observe per-context aborts.
                            let ctx = if ctx.cancel_token().is_none() {
                                ctx.with_cancel_token(ma.context_token(ctx.id()))
                            } else {
                                ctx
                            };
                            let fut =
                                async { module_clone.lock().await.process(ctx, port, value).await };
                            tokio::select! {
                                biased;
                                _ = token.cancelled() => {
                                    log::info!("Process cancelled: {}", module_id_clone);
                                    // Dropping the future aborts any in-flight
                                    // I/O and releases the module lock. A fired
                                    // token cannot be reset, so install a fresh
                                    // one unless this loop no longer owns the
                                    // token slot (module stopping or restarted).
                                    if let Some(fresh) = ma.renew_module_token(
                                        &patch_id,
                                        &module_id_clone,
                                        generation,
                                    ) {
                                        token = fresh;
                                    }
                                }
                                r = fut => r.unwrap_or_else(|e| {
                                    log::error!("Process Error {}: {}", module_id_clone, e);
                                }),
                            }
                        }
                        ModuleMessage::Config { key, value } => {
                            module_clone
                                .lock()
                                .await
                                .set_config(key, value)
                                .unwrap_or_else(|e| {
                                    log::error!("Config Error {}: {}", module_id_clone, e);
                                });
                        }
                        ModuleMessage::Configs { configs } => {
                            module_clone
                                .lock()
                                .await
                                .set_configs(configs)
                                .unwrap_or_else(|e| {
                                    log::error!("Configs Error {}: {}", module_id_clone, e);
                                });
                        }
                        ModuleMessage::Stop => {
                            rx.close();
                            break;
                        }
                    }
                }
            };

            self.tasks.spawn(module_loop);
        }
        Ok(())
    }

    /// Stop a module by id.
    ///
    /// Sends a stop message to the module, closes its message channel,
    /// and calls the module's [`stop()`](crate::AsModule::stop) method.
    pub async fn stop_module(&self, module_id: &str) -> Result<()> {
        {
            // remove the sender first to prevent new messages being sent
            let mut module_txs = self.module_txs.lock();
            if let Some(inbox) = module_txs.swap_remove(module_id)
                && let Err(e) = inbox.send(module_id, ModuleMessage::Stop)
            {
                log::warn!("Failed to send stop message to module {}: {}", module_id, e);
            }
        }

        // Cancel BEFORE awaiting the module lock: a long-running process()
        // holds the lock, and cancelling makes the module loop drop that
        // future (releasing the lock) instead of blocking stop until it
        // completes. Removing the entry first keeps the fired token in the
        // loop so inputs queued ahead of Stop are skipped rather than
        // processed with a renewed token.
        let token = self.module_tokens.lock().swap_remove(module_id);
        if let Some((_, token)) = token {
            token.cancel();
        }

        let module = {
            let modules = self.modules.lock();
            let Some(a) = modules.get(module_id) else {
                return Err(Error::ModuleNotFound(module_id.to_string()));
            };
            a.clone()
        };
        let mut module_guard = module.lock().await;
        if *module_guard.status() == ModuleStatus::Start {
            log::info!("Stopping module {}", module_id);
            module_guard.stop().await?;
        }

        Ok(())
    }

    /// Set configs for a module by id.
    ///
    /// Emits [`ModularAgentEvent::ModuleConfigUpdated`] for each key once the
    /// configs have been handed to the module. When the module is running, the
    /// configs travel through its message channel and are applied
    /// asynchronously: the events report successful delivery, not completed
    /// application. Events are emitted regardless of whether a key's value
    /// actually changed.
    ///
    /// A module with no live instance (its definition is not registered in
    /// this build) has the configs merged into its stored patch spec entry,
    /// so the edit survives a save.
    pub async fn set_module_configs(
        &self,
        module_id: String,
        configs: ModuleConfigs,
    ) -> Result<()> {
        let inbox = {
            let module_txs = self.module_txs.lock();
            module_txs.get(&module_id).cloned()
        };

        let Some(inbox) = inbox else {
            // The module is not running. We can set the configs directly.
            let module = {
                let modules = self.modules.lock();
                modules.get(&module_id).cloned()
            };
            let Some(module) = module else {
                // A spec-only module has no instance to configure, so write
                // through to its stored spec entry instead; otherwise the
                // edit would be lost on the next save. Same event contract
                // as the live branch below - per-key ModuleConfigUpdated, no
                // ModuleSpecUpdated - so hosts cannot tell the two apart.
                let configs_value = serde_json::to_value(&configs)
                    .map_err(|e| Error::SerializationError(e.to_string()))?;
                let patch = serde_json::json!({ "configs": configs_value });
                let Some((_, updated)) = self.patch_stored_module_spec(&module_id, &patch).await
                else {
                    return Err(Error::ModuleNotFound(module_id.to_string()));
                };
                updated?;
                for (key, value) in configs {
                    self.emit_module_config_updated(module_id.clone(), key, value);
                }
                return Ok(());
            };
            module.lock().await.set_configs(configs.clone())?;
            for (key, value) in configs {
                self.emit_module_config_updated(module_id.clone(), key, value);
            }
            return Ok(());
        };
        let message = ModuleMessage::Configs {
            configs: configs.clone(),
        };
        inbox.send(&module_id, message)?;
        for (key, value) in configs {
            self.emit_module_config_updated(module_id.clone(), key, value);
        }
        Ok(())
    }

    /// Get global configs for the module definition by name.
    pub fn get_global_configs(&self, def_name: &str) -> Option<ModuleConfigs> {
        let global_configs_map = self.global_configs_map.lock();
        global_configs_map.get(def_name).cloned()
    }

    /// Set global configs for the module definition by name.
    pub fn set_global_configs(&self, def_name: String, configs: ModuleConfigs) {
        let mut global_configs_map = self.global_configs_map.lock();

        let Some(existing_configs) = global_configs_map.get_mut(&def_name) else {
            global_configs_map.insert(def_name, configs);
            return;
        };

        for (key, value) in configs {
            existing_configs.set(key, value);
        }
    }

    /// Get the global configs map.
    pub fn get_global_configs_map(&self) -> ModuleConfigsMap {
        let global_configs_map = self.global_configs_map.lock();
        global_configs_map.clone()
    }

    /// Set the global configs map.
    pub fn set_global_configs_map(&self, new_configs_map: ModuleConfigsMap) {
        for (module_name, new_configs) in new_configs_map {
            self.set_global_configs(module_name, new_configs);
        }
    }

    /// Send input to a module.
    pub(crate) async fn module_input(
        &self,
        module_id: String,
        ctx: ModuleContext,
        port: String,
        value: Value,
    ) -> Result<()> {
        let message = if let Some(config_key) = port.strip_prefix("config:") {
            ModuleMessage::Config {
                key: config_key.to_string(),
                value,
            }
        } else {
            ModuleMessage::Input {
                ctx,
                port: port.clone(),
                value,
            }
        };

        let inbox = {
            let module_txs = self.module_txs.lock();
            module_txs.get(&module_id).cloned()
        };

        let Some(inbox) = inbox else {
            // The module is not running. If it's a config message, we can set it directly.
            let module: SharedModule = {
                let modules = self.modules.lock();
                let Some(a) = modules.get(&module_id) else {
                    return Err(Error::ModuleNotFound(module_id.to_string()));
                };
                a.clone()
            };
            if let ModuleMessage::Config { key, value } = message {
                module.lock().await.set_config(key.clone(), value.clone())?;
                self.emit_module_config_updated(module_id, key, value);
            }
            return Ok(());
        };
        // Same delivery semantics as set_module_configs: the event reports
        // successful delivery to the module's channel, not completed
        // application. This is what lets hosts show wire-driven config
        // values live.
        let config_update = match &message {
            ModuleMessage::Config { key, value } => Some((key.clone(), value.clone())),
            _ => None,
        };
        inbox.send(&module_id, message)?;
        if let Some((key, value)) = config_update {
            self.emit_module_config_updated(module_id.clone(), key, value);
        }

        self.emit_module_input(module_id.to_string(), port);

        Ok(())
    }

    /// Send output from a module. (Async version)
    pub async fn send_module_out(
        &self,
        module_id: String,
        ctx: ModuleContext,
        port: String,
        value: Value,
    ) -> Result<()> {
        message::send_module_out(self, module_id, ctx, port, value).await
    }

    /// Send output from a module.
    pub fn try_send_module_out(
        &self,
        module_id: String,
        ctx: ModuleContext,
        port: String,
        value: Value,
    ) -> Result<()> {
        message::try_send_module_out(self, module_id, ctx, port, value)
    }

    /// Write a value to a named channel.
    ///
    /// This is the primary method for sending external input into the module network.
    /// The value will be delivered to all [`ExternalInputModule`](crate::external_module::ExternalInputModule)
    /// instances listening to the specified channel name, which will then forward it to
    /// their connected modules.
    ///
    /// # Arguments
    ///
    /// * `name` - The channel name to write to. Must match the `name` config of an `ExternalInputModule`.
    /// * `value` - The value to send.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use modular_agent_core::{ModularAgent, Value};
    /// # async fn example(ma: ModularAgent) {
    /// // Send a string to the "input" channel
    /// ma.write_external_input("input".to_string(), Value::string("hello")).await.unwrap();
    ///
    /// // Send an integer
    /// ma.write_external_input("numbers".to_string(), Value::integer(42)).await.unwrap();
    /// # }
    /// ```
    pub async fn write_external_input(&self, name: String, value: Value) -> Result<()> {
        self.send_external_output(name, ModuleContext::new(), value)
            .await
    }

    /// Write a value to the local variable channel.
    pub async fn write_local_input(&self, patch_id: &str, name: &str, value: Value) -> Result<()> {
        let channel_name = format!("%{}/{}", patch_id, name);
        self.send_external_output(channel_name, ModuleContext::new(), value)
            .await
    }

    pub(crate) async fn send_external_output(
        &self,
        name: String,
        ctx: ModuleContext,
        value: Value,
    ) -> Result<()> {
        message::send_external_output(self, name, ctx, value).await
    }

    async fn spawn_message_loop(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        {
            let mut tx_lock = self.tx.lock();
            *tx_lock = Some(tx);
        }

        // spawn the main loop; base() so events emitted while routing
        // messages are never attributed to the caller of ready().
        let ma = self.base();
        self.tasks.spawn(async move {
            // Receiver-side depth check is enough here: unlike a module loop,
            // this loop never blocks on delivery, so it keeps returning to
            // recv() and the check keeps running.
            let mut warn_at = QUEUE_HIGH_WATER;
            while let Some(message) = rx.recv().await {
                let depth = rx.len();
                if depth >= warn_at {
                    log::warn!(
                        "Router queue depth reached {} (unbounded queue, routing falling behind)",
                        depth
                    );
                    while warn_at <= depth {
                        warn_at *= 2;
                    }
                } else if depth < QUEUE_HIGH_WATER / 2 {
                    warn_at = QUEUE_HIGH_WATER;
                }

                use ModuleEventMessage::*;

                match message {
                    ModuleOut {
                        module,
                        ctx,
                        port,
                        value,
                    } => {
                        message::module_out(&ma, module, ctx, port, value).await;
                    }
                    ExternalOutput { name, ctx, value } => {
                        message::external_input(&ma, name, ctx, value).await;
                    }
                }
            }
        });

        tokio::task::yield_now().await;

        Ok(())
    }

    /// Subscribe to all `ModularAgent` events.
    ///
    /// Returns a broadcast receiver of [`EventEnvelope`]s, each carrying a
    /// [`ModularAgentEvent`] together with the origin of the change.
    /// For filtered subscriptions, use [`subscribe_to_event`](Self::subscribe_to_event).
    ///
    /// **Note**: Subscribe before starting patches to avoid missing events.
    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.observers.subscribe()
    }

    /// Subscribe to filtered [`ModularAgentEvent`]s.
    ///
    /// This method creates a filtered subscription to events. The provided closure
    /// filters and maps events, and only successfully mapped events are forwarded
    /// to the returned receiver.
    ///
    /// **Important**: Subscribe to events BEFORE starting patches to avoid missing
    /// events due to race conditions.
    ///
    /// # Arguments
    ///
    /// * `filter_map` - A closure that receives each [`EventEnvelope`] and returns
    ///   `Some(T)` for events you want to receive, or `None` to skip them.
    ///
    /// # Returns
    ///
    /// An unbounded receiver that will receive the filtered and mapped events.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use modular_agent_core::{ModularAgent, ModularAgentEvent, Value};
    ///
    /// # async fn example(ma: &ModularAgent) {
    /// // Subscribe to a specific channel's output
    /// let output_channel = "output".to_string();
    /// let mut output_rx = ma.subscribe_to_event(move |envelope| {
    ///     if let ModularAgentEvent::ExternalOutput(name, value) = envelope.event {
    ///         if name == output_channel {
    ///             return Some(value);
    ///         }
    ///     }
    ///     None
    /// });
    ///
    /// // Now start the patch and receive events
    /// while let Some(value) = output_rx.recv().await {
    ///     println!("Received: {:?}", value);
    /// }
    /// # }
    /// ```
    pub fn subscribe_to_event<F, T>(&self, mut filter_map: F) -> mpsc::UnboundedReceiver<T>
    where
        F: FnMut(EventEnvelope) -> Option<T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut event_rx = self.subscribe();
        let shutdown = self.shutdown_token.child_token();

        self.tasks.spawn(async move {
            loop {
                tokio::select! {
                    // biased: events already in the broadcast channel (e.g.
                    // PatchStopped emitted by shutdown itself) are forwarded
                    // before cancellation is observed, so they are not lost.
                    biased;
                    // Without this arm the forwarder would outlive a dropped
                    // receiver until the next event arrives, which may be never.
                    _ = tx.closed() => break,
                    received = event_rx.recv() => match received {
                        Ok(envelope) => {
                            if let Some(mapped_event) = filter_map(envelope)
                                && tx.send(mapped_event).is_err()
                            {
                                break;
                            }
                        }
                        Err(RecvError::Lagged(n)) => {
                            log::warn!("Event subscriber lagged by {} events", n);
                        }
                        Err(RecvError::Closed) => break,
                    },
                    _ = shutdown.cancelled() => break,
                }
            }
        });
        rx
    }

    pub(crate) fn emit_module_config_updated(&self, module_id: String, key: String, value: Value) {
        self.notify_observers(ModularAgentEvent::ModuleConfigUpdated(
            module_id, key, value,
        ));
    }

    pub(crate) fn emit_module_error(&self, module_id: String, message: String) {
        self.notify_observers(ModularAgentEvent::ModuleError(module_id, message));
    }

    pub(crate) fn emit_module_input(&self, module_id: String, port: String) {
        self.notify_observers(ModularAgentEvent::ModuleIn(module_id, port));
    }

    pub(crate) fn emit_module_spec_updated(&self, module_id: String) {
        self.notify_observers(ModularAgentEvent::ModuleSpecUpdated(module_id));
    }

    pub(crate) fn emit_patch_structure_changed(&self, patch_id: String) {
        self.notify_observers(ModularAgentEvent::PatchStructureChanged { patch_id });
    }

    pub(crate) fn emit_patch_added(&self, patch_id: String, name: Option<String>) {
        self.notify_observers(ModularAgentEvent::PatchAdded { patch_id, name });
    }

    pub(crate) fn emit_patch_removed(&self, patch_id: String, name: Option<String>) {
        self.notify_observers(ModularAgentEvent::PatchRemoved { patch_id, name });
    }

    pub(crate) fn emit_patch_started(&self, patch_id: String) {
        self.notify_observers(ModularAgentEvent::PatchStarted { patch_id });
    }

    pub(crate) fn emit_patch_stopped(&self, patch_id: String) {
        self.notify_observers(ModularAgentEvent::PatchStopped { patch_id });
    }

    pub(crate) fn emit_patch_renamed(
        &self,
        patch_id: String,
        old_name: Option<String>,
        new_name: String,
    ) {
        self.notify_observers(ModularAgentEvent::PatchRenamed {
            patch_id,
            old_name,
            new_name,
        });
    }

    #[cfg(feature = "file")]
    pub(crate) fn emit_patch_saved(&self, patch_id: String, name: String) {
        self.notify_observers(ModularAgentEvent::PatchSaved { patch_id, name });
    }

    pub(crate) fn emit_external_output(&self, name: String, value: Value) {
        // // ignore local variables
        // if name.starts_with('%') {
        //     return;
        // }
        self.notify_observers(ModularAgentEvent::ExternalOutput(name, value));
    }

    /// The single point where events are wrapped into envelopes, so every
    /// emitted event carries exactly the origin of the handle it went through.
    fn notify_observers(&self, event: ModularAgentEvent) {
        let _ = self.observers.send(EventEnvelope {
            origin: self.origin.clone(),
            event,
        });
    }
}

/// Whether a module spec patch warrants a `PatchStructureChanged`.
///
/// Any non-config key (ports, title, layout, ...) may change how hosts render
/// the patch, so treat those patches as structural. Config-only patches stay
/// quiet here; they are covered by `ModuleSpecUpdated`.
fn is_structural_spec_patch(value: &JsonValue) -> bool {
    value
        .as_object()
        .is_some_and(|map| map.keys().any(|key| key != "configs"))
}

/// Carrier for a [`ModularAgentEvent`] together with the origin of the change.
///
/// `origin` identifies the entry point that performed the mutation which
/// produced the event (see [`ModularAgent::with_origin`]). `None` means the
/// event originated inside the module runtime itself.
#[derive(Clone, Debug)]
pub struct EventEnvelope {
    pub origin: Option<Arc<str>>,
    pub event: ModularAgentEvent,
}

/// Events emitted by [`ModularAgent`] during operation.
///
/// Subscribe to these events using [`ModularAgent::subscribe`] or
/// [`ModularAgent::subscribe_to_event`].
///
/// # Example
///
/// ```rust,no_run
/// use modular_agent_core::{ModularAgent, ModularAgentEvent};
///
/// # fn example(ma: &ModularAgent) {
/// // Subscribe to all external output events
/// let mut rx = ma.subscribe_to_event(|envelope| {
///     if let ModularAgentEvent::ExternalOutput(name, value) = envelope.event {
///         Some((name, value))
///     } else {
///         None
///     }
/// });
/// # }
/// ```
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ModularAgentEvent {
    /// A module's configuration was updated.
    ///
    /// Fields: `(module_id, config_key, new_value)`
    ModuleConfigUpdated(String, String, Value),

    /// A module encountered an error.
    ///
    /// Fields: `(module_id, error_message)`
    ModuleError(String, String),

    /// A module received input on a port.
    ///
    /// Fields: `(module_id, port_name)`
    ModuleIn(String, String),

    /// A module's spec was updated.
    ///
    /// Fields: `(module_id)`
    ModuleSpecUpdated(String),

    /// A patch's structure (modules, connections, or non-config spec keys)
    /// was changed.
    ///
    /// Emitted by [`ModularAgent::add_module`], [`ModularAgent::remove_module`],
    /// [`ModularAgent::add_connection`], [`ModularAgent::remove_connection`],
    /// [`ModularAgent::add_modules_and_connections`],
    /// [`ModularAgent::update_patch_spec`], and by
    /// [`ModularAgent::update_module_spec`] when the patch contains keys other
    /// than `configs`, so hosts can refresh their view of the patch.
    PatchStructureChanged { patch_id: String },

    /// A patch was added.
    ///
    /// Emitted whenever a patch is created or loaded
    /// ([`ModularAgent::new_patch`], [`ModularAgent::add_patch`], their
    /// named variants, and `open_patch_from_file`).
    PatchAdded {
        patch_id: String,
        name: Option<String>,
    },

    /// A patch was removed.
    ///
    /// Emitted by [`ModularAgent::remove_patch`] after the patch and its
    /// modules have been torn down, so hosts can close any view of it.
    PatchRemoved {
        patch_id: String,
        name: Option<String>,
    },

    /// A patch started running.
    ///
    /// Emitted by [`ModularAgent::start_patch`] only on an actual transition,
    /// so starting an already-running patch produces no event.
    PatchStarted { patch_id: String },

    /// A patch stopped running.
    ///
    /// Emitted by [`ModularAgent::stop_patch`] only on an actual transition.
    /// Removing a patch emits [`ModularAgentEvent::PatchRemoved`] instead.
    PatchStopped { patch_id: String },

    /// A patch was renamed.
    ///
    /// Emitted by [`ModularAgent::rename_patch`]. `old_name` is `None` when
    /// the patch had no name before.
    PatchRenamed {
        patch_id: String,
        old_name: Option<String>,
        new_name: String,
    },

    /// A named patch was saved to disk.
    ///
    /// Emitted by [`ModularAgent::save_patch`]; unnamed patches produce no
    /// event.
    #[cfg(feature = "file")]
    PatchSaved { patch_id: String, name: String },

    /// A value was written to an external output channel.
    ///
    /// This event is emitted when:
    /// - [`ModularAgent::write_external_input`] is called and flows through the network
    /// - An [`ExternalOutputModule`](crate::external_module::ExternalOutputModule) receives a value
    ///
    /// Fields: `(channel_name, value)`
    ExternalOutput(String, Value),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_context_tokens_are_not_evicted_at_prune_threshold() {
        let ma = ModularAgent::new();
        let tokens: Vec<_> = (0..=CONTEXT_TOKEN_PRUNE_THRESHOLD)
            .map(|ctx_id| ma.context_token(ctx_id))
            .collect();

        assert_eq!(tokens.len(), CONTEXT_TOKEN_PRUNE_THRESHOLD + 1);
        assert!(ma.abort_context(0));
        assert!(tokens[0].is_cancelled());
    }

    #[tokio::test]
    async fn event_forwarder_exits_when_receiver_is_dropped() {
        let ma = ModularAgent::new();
        let before = ma.tasks.len();

        let rx = ma.subscribe_to_event(|_| Some(()));
        assert_eq!(ma.tasks.len(), before + 1);

        // No event is ever emitted, so only `tx.closed()` can wake the forwarder.
        drop(rx);
        for _ in 0..100 {
            if ma.tasks.len() == before {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(ma.tasks.len(), before);
    }
}
