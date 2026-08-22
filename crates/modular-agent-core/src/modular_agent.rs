use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use parking_lot::Mutex;
use serde_json::Value;
use tokio::sync::{Mutex as AsyncMutex, broadcast, broadcast::error::RecvError, mpsc};
use tokio_util::sync::CancellationToken;

use crate::FnvIndexMap;
use crate::agent::{Agent, AgentMessage, AgentStatus, agent_new};
use crate::config::{AgentConfigs, AgentConfigsMap};
use crate::context::AgentContext;
use crate::definition::{AgentConfigSpecs, AgentDefinition, AgentDefinitions};
use crate::error::AgentError;
use crate::id::{new_id, update_ids};
use crate::message::{self, AgentEventMessage};
use crate::patch::{Patch, PatchInfo};
use crate::registry;
use crate::spec::{AgentSpec, ConnectionSpec, PatchSpec};
use crate::value::AgentValue;

const MESSAGE_LIMIT: usize = 1024;
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Registry size at which dead context-token entries are pruned. Entries are
/// `Weak` and die with their flow (contexts hold the only strong references).
/// The registry may exceed this threshold when more flows are genuinely live:
/// live entries must remain tracked so every flow stays abortable.
const CONTEXT_TOKEN_PRUNE_THRESHOLD: usize = 1024;

/// Distinguishes which agent-loop incarnation owns the `agent_tokens` slot,
/// so a draining old loop cannot clobber the token installed for a restarted
/// agent's new loop (tokens themselves have no identity to compare).
static AGENT_TOKEN_GENERATION: AtomicU64 = AtomicU64::new(1);

/// The central orchestrator for the modular agent system.
///
/// `ModularAgent` manages agent lifecycle, connections, and message routing.
/// It maintains agent instances, connection maps, and handles [`ModularAgentEvent`]s.
///
/// # Lifecycle
///
/// 1. [`init()`](Self::init) - Create instance and register agent definitions
/// 2. [`ready()`](Self::ready) - Start the internal message loop
/// 3. Load patches with [`open_patch_from_file()`](Self::open_patch_from_file) or [`add_patch()`](Self::add_patch)
/// 4. [`start_patch()`](Self::start_patch) - Start agents in a patch
/// 5. Interact via [`write_external_input()`](Self::write_external_input) and [`subscribe()`](Self::subscribe)
/// 6. [`stop_patch()`](Self::stop_patch) - Stop agents
/// 7. [`quit()`](Self::quit) - Shut down
///
/// # Example
///
#[cfg_attr(feature = "file", doc = "```rust,no_run")]
#[cfg_attr(not(feature = "file"), doc = "```rust,no_run,ignore")]
/// use modular_agent_core::{ModularAgent, AgentValue, ModularAgentEvent};
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
///     ma.write_external_input("input".to_string(), AgentValue::string("hello")).await?;
///
///     // Cleanup
///     ma.stop_patch(&patch_id).await?;
///     ma.quit();
///     Ok(())
/// }
/// ```
/// Shared, lockable handle to a running agent instance.
pub type SharedAgent = Arc<AsyncMutex<Box<dyn Agent>>>;

// target agent id / source handle / target handle
pub(crate) type ConnectionTarget = (String, String, String);

#[derive(Clone)]
pub struct ModularAgent {
    // agent id -> agent
    pub(crate) agents: Arc<Mutex<FnvIndexMap<String, SharedAgent>>>,

    // agent id -> sender
    pub(crate) agent_txs: Arc<Mutex<FnvIndexMap<String, mpsc::Sender<AgentMessage>>>>,

    // channel name -> [external input agent id]
    pub(crate) external_input_agents: Arc<Mutex<FnvIndexMap<String, Vec<String>>>>,

    // channel name -> value
    pub(crate) external_values: Arc<Mutex<FnvIndexMap<String, AgentValue>>>,

    // source agent id -> [connection targets]
    pub(crate) connections: Arc<Mutex<FnvIndexMap<String, Vec<ConnectionTarget>>>>,

    // agent def name -> agent definition
    pub(crate) defs: Arc<Mutex<AgentDefinitions>>,

    // patches (patch id -> patch)
    pub(crate) patches: Arc<Mutex<FnvIndexMap<String, Arc<AsyncMutex<Patch>>>>>,

    /// name -> patch id: the single source of truth for patch name lookup
    /// and uniqueness. Mutated only by `add_patch_raw`, `rename_patch`,
    /// and `remove_patch`.
    ///
    /// Lock order: never acquire `patches` or a patch's async mutex while
    /// holding this lock.
    pub(crate) patch_names: Arc<Mutex<FnvIndexMap<String, String>>>,

    // agent def name -> config
    pub(crate) global_configs_map: Arc<Mutex<FnvIndexMap<String, AgentConfigs>>>,

    // patch id -> parent cancellation token for the patch's agents
    pub(crate) patch_tokens: Arc<Mutex<FnvIndexMap<String, CancellationToken>>>,

    // agent id -> (loop generation, current cancellation token of that loop)
    pub(crate) agent_tokens: Arc<Mutex<FnvIndexMap<String, (u64, CancellationToken)>>>,

    // context id -> cancellation token (weak: dies with the flow's contexts)
    pub(crate) context_tokens: Arc<Mutex<FnvIndexMap<usize, Weak<CancellationToken>>>>,

    // message sender
    pub(crate) tx: Arc<Mutex<Option<mpsc::Sender<AgentEventMessage>>>>,

    // observers
    pub(crate) observers: broadcast::Sender<EventEnvelope>,

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
    /// Create a new `ModularAgent` instance without registering agents.
    ///
    /// For most use cases, prefer [`init()`](Self::init) which also registers
    /// all agent definitions from the inventory.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            agents: Default::default(),
            agent_txs: Default::default(),
            external_input_agents: Default::default(),
            external_values: Default::default(),
            connections: Default::default(),
            defs: Default::default(),
            patches: Default::default(),
            patch_names: Default::default(),
            global_configs_map: Default::default(),
            patch_tokens: Default::default(),
            agent_tokens: Default::default(),
            context_tokens: Default::default(),
            tx: Arc::new(Mutex::new(None)),
            observers: tx,
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
    /// Invariant: every handle stored beyond the current call (agent data,
    /// spawned loops) must be created through this method. Otherwise runtime
    /// events emitted later would be attributed to whichever tagged entry
    /// point happened to create the agent or loop.
    pub(crate) fn base(&self) -> Self {
        Self {
            origin: None,
            ..self.clone()
        }
    }

    pub(crate) fn tx(&self) -> Result<mpsc::Sender<AgentEventMessage>, AgentError> {
        self.tx.lock().clone().ok_or(AgentError::TxNotInitialized)
    }

    /// Initialize a new `ModularAgent` instance.
    ///
    /// This creates a new `ModularAgent` and registers all available agent definitions
    /// from the inventory. Call [`ready`](Self::ready) after this to start the message loop.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use modular_agent_core::ModularAgent;
    ///
    /// let ma = ModularAgent::init().unwrap();
    /// ```
    pub fn init() -> Result<Self, AgentError> {
        let ma = Self::new();
        ma.register_agents();
        Ok(ma)
    }

    fn register_agents(&self) {
        registry::register_inventory_agents(self);
    }

    /// Start the internal message loop.
    ///
    /// This must be called after [`init`](Self::init) before loading patches or sending messages.
    /// The message loop handles routing between agents and external output events.
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
    pub async fn ready(&self) -> Result<(), AgentError> {
        self.spawn_message_loop().await?;
        Ok(())
    }

    /// Shut down the `ModularAgent`.
    ///
    /// This stops the internal message loop. Call [`stop_patch`](Self::stop_patch)
    /// for each running patch before calling this method for graceful shutdown.
    ///
    /// This does not release external resources such as MCP server child processes.
    /// Use [`shutdown`](Self::shutdown) instead when full cleanup is required.
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

    /// Shut down the `ModularAgent` and release external resources.
    ///
    /// Calls [`quit`](Self::quit) to stop the internal message loop, then closes any
    /// pooled MCP server connections so their child processes do not leak. Call
    /// [`stop_patch`](Self::stop_patch) for each running patch before this method.
    /// An MCP tool call still in flight during shutdown may reconnect and respawn its
    /// server process afterwards, so quiesce all workflows first.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use modular_agent_core::ModularAgent;
    /// # async fn example(ma: ModularAgent, patch_id: &str) {
    /// ma.stop_patch(patch_id).await.unwrap();
    /// ma.shutdown().await.unwrap();
    /// # }
    /// ```
    pub async fn shutdown(&self) -> Result<(), AgentError> {
        self.quit();
        #[cfg(feature = "mcp")]
        crate::mcp::shutdown_all_mcp_connections().await?;
        Ok(())
    }

    // Patch management

    /// Create a new empty patch.
    ///
    /// Returns the id of the new patch. The patch is created with default settings
    /// and contains no agents or connections initially.
    pub fn new_patch(&self) -> Result<String, AgentError> {
        let spec = PatchSpec::default();
        let id = self.add_patch(spec)?;
        Ok(id)
    }

    /// Create a new empty patch with the given name.
    ///
    /// Returns the id of the new patch.
    pub fn new_patch_with_name(&self, name: String) -> Result<String, AgentError> {
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
    /// The ids of the given spec, including agents and connections, are changed to new unique ids.
    /// This allows the same spec to be added multiple times without id conflicts.
    pub fn add_patch(&self, spec: PatchSpec) -> Result<String, AgentError> {
        self.add_patch_raw(spec, None)
    }

    /// Add a new patch with the given name and spec, and returns the id of the new patch.
    ///
    /// The ids of the given spec, including agents and connections, are changed to new unique ids.
    pub fn add_patch_with_name(&self, spec: PatchSpec, name: String) -> Result<String, AgentError> {
        self.add_patch_raw(spec, Some(name))
    }

    fn add_patch_raw(&self, spec: PatchSpec, name: Option<String>) -> Result<String, AgentError> {
        let mut patch = Patch::new(spec);
        if let Some(name) = &name {
            patch.set_name(name.clone());
        }
        let id = patch.id().to_string();

        // Reserve the name first so a duplicate fails before any agents are
        // created; the reservation is rolled back if a later step fails.
        if let Some(name) = &name {
            let mut names = self.patch_names.lock();
            if names.contains_key(name) {
                return Err(AgentError::PatchNameExists(name.clone()));
            }
            names.insert(name.clone(), id.clone());
        }

        // add agents
        for agent in &patch.spec().agents {
            if let Err(e) = self.add_agent_internal(id.clone(), agent.clone()) {
                log::error!("Failed to add_agent {}: {}", agent.id, e);
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
            return Err(AgentError::DuplicateId(id));
        }

        self.emit_patch_added(id.clone(), name);

        Ok(id)
    }

    /// Rename a patch by id.
    ///
    /// Fails with [`AgentError::PatchNameExists`] when another patch
    /// already uses `new_name`. Renaming a patch to its current name is a
    /// no-op and succeeds. Emits [`ModularAgentEvent::PatchRenamed`].
    pub async fn rename_patch(&self, id: &str, new_name: String) -> Result<(), AgentError> {
        let patch = self
            .get_patch(id)
            .ok_or_else(|| AgentError::PatchNotFound(id.to_string()))?;

        {
            let mut names = self.patch_names.lock();
            if let Some(owner) = names.get(&new_name)
                && owner != id
            {
                return Err(AgentError::PatchNameExists(new_name));
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
            return Err(AgentError::PatchNotFound(id.to_string()));
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
    /// Stops the patch if running, then removes all associated agents and connections.
    /// Emits [`ModularAgentEvent::PatchRemoved`] after teardown.
    pub async fn remove_patch(&self, id: &str) -> Result<(), AgentError> {
        let patch = self
            .get_patch(id)
            .ok_or_else(|| AgentError::PatchNotFound(id.to_string()))?;

        let mut patch = patch.lock().await;
        let name = patch.name().map(str::to_string);
        patch.stop(self).await.unwrap_or_else(|e| {
            log::error!("Failed to stop patch {}: {}", id, e);
        });

        // Remove all agents and connections associated with the patch
        for agent in &patch.spec().agents {
            self.remove_agent_internal(&agent.id)
                .await
                .unwrap_or_else(|e| {
                    log::error!("Failed to remove_agent {}: {}", agent.id, e);
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
    /// This starts all agents in the patch, enabling message flow between them.
    /// Each agent's [`start()`](crate::AsAgent::start) method is called.
    ///
    /// Emits [`ModularAgentEvent::PatchStarted`] when the patch was not
    /// already running.
    pub async fn start_patch(&self, id: &str) -> Result<(), AgentError> {
        let patch = self
            .get_patch(id)
            .ok_or_else(|| AgentError::PatchNotFound(id.to_string()))?;
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
    /// This stops all agents in the patch, terminating message processing.
    /// Each agent's [`stop()`](crate::AsAgent::stop) method is called.
    ///
    /// Emits [`ModularAgentEvent::PatchStopped`] when the patch was running.
    pub async fn stop_patch(&self, id: &str) -> Result<(), AgentError> {
        let patch = self
            .get_patch(id)
            .ok_or_else(|| AgentError::PatchNotFound(id.to_string()))?;
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
    pub async fn open_patch_from_file(
        &self,
        path: &str,
        name: Option<String>,
    ) -> Result<String, AgentError> {
        let json_str =
            std::fs::read_to_string(path).map_err(|e| AgentError::IoError(e.to_string()))?;
        let spec = PatchSpec::from_json(&json_str)?;
        let id = self.add_patch_raw(spec, name)?;
        Ok(id)
    }

    /// Save a patch to a JSON file.
    ///
    /// Serializes the current patch state (including agent configs) to JSON
    /// and writes it to the specified path. Emits
    /// [`ModularAgentEvent::PatchSaved`] when the patch has a name; unnamed
    /// patches have no list entry to refresh, so no event is emitted for them.
    #[cfg(feature = "file")]
    pub async fn save_patch(&self, id: &str, path: &str) -> Result<(), AgentError> {
        let Some(patch_spec) = self.get_patch_spec(id).await else {
            return Err(AgentError::PatchNotFound(id.to_string()));
        };
        let json_str = patch_spec.to_json()?;
        std::fs::write(path, json_str).map_err(|e| AgentError::IoError(e.to_string()))?;
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

        // Overlay live agent specs onto the stored entries. An agent whose
        // definition is not registered in this build has no live instance;
        // keep its stored spec so it survives the editor round-trip and the
        // save that follows (save_patch writes exactly what this returns).
        for agent in &mut patch_spec.agents {
            if let Some(spec) = self.get_agent_spec(&agent.id).await {
                *agent = spec;
            }
        }

        // No need to change connections

        Some(patch_spec)
    }

    /// Update the patch spec
    pub async fn update_patch_spec(&self, id: &str, value: &Value) -> Result<(), AgentError> {
        let patch = self
            .get_patch(id)
            .ok_or_else(|| AgentError::PatchNotFound(id.to_string()))?;
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

    // Agents

    /// Register an agent definition.
    ///
    /// This makes the agent type available for use in patches. The definition
    /// includes metadata (title, category), input/output ports, and config specs.
    ///
    /// Note: Agents using `#[modular_agent]` macro are registered automatically via inventory.
    pub fn register_agent_definiton(&self, def: AgentDefinition) {
        let def_name = def.name.clone();
        let def_global_configs = def.global_configs.clone();

        let mut defs = self.defs.lock();
        defs.insert(def.name.clone(), def);

        // if there is a global config, set it
        if let Some(def_global_configs) = def_global_configs {
            let mut new_configs = AgentConfigs::default();
            for (key, config_entry) in def_global_configs.iter() {
                new_configs.set(key.clone(), config_entry.value.clone());
            }
            self.set_global_configs(def_name, new_configs);
        }
    }

    /// Get all registered agent definitions.
    ///
    /// Returns a map of definition name to [`AgentDefinition`].
    pub fn get_agent_definitions(&self) -> AgentDefinitions {
        let defs = self.defs.lock();
        defs.clone()
    }

    /// Get an agent definition by name.
    ///
    /// The name is typically in the format `module::path::StructName`.
    pub fn get_agent_definition(&self, def_name: &str) -> Option<AgentDefinition> {
        let defs = self.defs.lock();
        defs.get(def_name).cloned()
    }

    /// Get the config specs of an agent definition by name.
    pub fn get_agent_config_specs(&self, def_name: &str) -> Option<AgentConfigSpecs> {
        let defs = self.defs.lock();
        let def = defs.get(def_name)?;
        def.configs.clone()
    }

    /// Get the agent spec by id.
    pub async fn get_agent_spec(&self, agent_id: &str) -> Option<AgentSpec> {
        let agent = {
            let agents = self.agents.lock();
            agents.get(agent_id)?.clone()
        };
        let agent = agent.lock().await;
        Some(agent.spec().clone())
    }

    /// Look up the stored patch spec entry of an agent by id.
    ///
    /// Unlike [`Self::get_agent_spec`] this also finds spec-only agents
    /// (whose definition is not registered in this build), which have no
    /// live instance. For a live agent it returns the stored entry, not the
    /// instance spec.
    pub(crate) async fn find_stored_agent_spec(&self, agent_id: &str) -> Option<AgentSpec> {
        let patches = {
            let patches = self.patches.lock();
            patches.values().cloned().collect::<Vec<_>>()
        };
        for patch in patches {
            let patch = patch.lock().await;
            if let Some(agent) = patch.spec().agents.iter().find(|a| a.id == agent_id) {
                return Some(agent.clone());
            }
        }
        None
    }

    /// Update the agent spec by id.
    ///
    /// A patch containing `configs` calls the agent's
    /// [`AsAgent::configs_changed`], so agents that derive ports or further
    /// configs from their config values rebuild them; an error it reports is
    /// propagated, as with [`ModularAgent::set_agent_configs`].
    ///
    /// Emits [`ModularAgentEvent::AgentSpecUpdated`], and additionally
    /// [`ModularAgentEvent::PatchStructureChanged`] when the patch contains
    /// keys other than `configs`. The events are emitted even when an error
    /// is returned: the agent may have committed the patch before failing
    /// (`configs_changed` runs after the merge), and a spec change must never
    /// go unannounced to hosts.
    ///
    /// An agent with no live instance (its definition is not registered in
    /// this build) is patched in the patch spec that holds it, with the same
    /// events; [`AgentError::AgentNotFound`] is returned only when no patch
    /// holds the id either.
    pub async fn update_agent_spec(&self, agent_id: &str, value: &Value) -> Result<(), AgentError> {
        let agent = {
            let agents = self.agents.lock();
            agents.get(agent_id).cloned()
        };
        let Some(agent) = agent else {
            // No live instance: the agent may still exist as a spec-only
            // entry, whose stored spec is the only place a patch can land.
            return self.update_spec_only_agent(agent_id, value).await;
        };
        let (patch_id, updated) = {
            let mut agent = agent.lock().await;
            let updated = agent.update_spec(value);
            (agent.patch_id().to_string(), updated)
        };

        // A failure may have left the patch committed (an agent can reject a
        // value in configs_changed after storing it, as Switch does with an
        // unparsable condition), so announce first and propagate after: a
        // spurious refresh is harmless, an unannounced spec change is not.
        self.emit_agent_spec_updated(agent_id.to_string());

        if is_structural_spec_patch(value) {
            self.emit_patch_structure_changed(patch_id);
        }
        updated
    }

    /// Patch the stored spec entry of an agent that has no live instance.
    ///
    /// A spec-only agent (its definition is not registered in this build)
    /// never got instantiated, so the patch spec is the only place its
    /// layout, ports or configs can be recorded. The event contract matches
    /// the live path so hosts cannot tell the two apart.
    async fn update_spec_only_agent(
        &self,
        agent_id: &str,
        value: &Value,
    ) -> Result<(), AgentError> {
        let Some((patch_id, updated)) = self.patch_stored_agent_spec(agent_id, value).await else {
            return Err(AgentError::AgentNotFound(agent_id.to_string()));
        };

        // A rejected key can follow keys that were already merged, so a
        // failed patch still has to announce the change.
        self.emit_agent_spec_updated(agent_id.to_string());
        if is_structural_spec_patch(value) {
            self.emit_patch_structure_changed(patch_id);
        }
        updated
    }

    /// Applies a patch to an agent's stored spec entry, emitting no events.
    ///
    /// Returns the id of the patch that holds the agent together with the
    /// patch result, or `None` when no patch spec contains the id. The
    /// patch id is returned even when the patch failed, so callers can
    /// still announce a partially merged change.
    async fn patch_stored_agent_spec(
        &self,
        agent_id: &str,
        value: &Value,
    ) -> Option<(String, Result<(), AgentError>)> {
        // Take a snapshot and release the patches lock: a patch's async
        // mutex must never be awaited while the sync map lock is held.
        let patches = {
            let patches = self.patches.lock();
            patches.values().cloned().collect::<Vec<_>>()
        };

        for patch in patches {
            // One patch at a time, so no two patch locks are ever held.
            let mut patch = patch.lock().await;
            match patch.update_agent_spec(agent_id, value) {
                Ok(false) => continue,
                result => return Some((patch.id().to_string(), result.map(|_| ()))),
            }
        }
        None
    }

    /// Create a new agent spec from the given agent definition name.
    pub fn new_agent_spec(&self, def_name: &str) -> Result<AgentSpec, AgentError> {
        let def = self
            .get_agent_definition(def_name)
            .ok_or_else(|| AgentError::AgentDefinitionNotFound(def_name.to_string()))?;
        Ok(def.to_spec())
    }

    /// Add an agent to the specified patch.
    ///
    /// Creates a new agent instance from the given spec and adds it to the patch.
    /// Returns the id of the newly created agent. The agent is not started automatically;
    /// call [`start_patch`](Self::start_patch) or [`start_agent`](Self::start_agent) to start it.
    pub async fn add_agent(
        &self,
        patch_id: String,
        mut spec: AgentSpec,
    ) -> Result<String, AgentError> {
        let patch = self
            .get_patch(&patch_id)
            .ok_or_else(|| AgentError::PatchNotFound(patch_id.to_string()))?;

        let id = new_id();
        spec.id = id.clone();
        // Register the constructed spec: new() may have generated dynamic
        // configs/ports via update_spec, and the patch must reflect them.
        let constructed = self.add_agent_internal(patch_id.clone(), spec)?;

        let mut patch = patch.lock().await;
        patch.add_agent(constructed);
        drop(patch);

        self.emit_patch_structure_changed(patch_id);

        Ok(id)
    }

    fn add_agent_internal(
        &self,
        patch_id: String,
        spec: AgentSpec,
    ) -> Result<AgentSpec, AgentError> {
        let mut agents = self.agents.lock();
        if agents.contains_key(&spec.id) {
            return Err(AgentError::AgentAlreadyExists(spec.id.to_string()));
        }
        let spec_id = spec.id.clone();
        // base(): the agent keeps this handle for its lifetime, so runtime
        // events it emits later must not inherit the creator's origin tag.
        let mut agent = agent_new(self.base(), spec_id.clone(), spec)?;
        agent.set_patch_id(patch_id);
        let constructed = agent.spec().clone();
        agents.insert(spec_id, Arc::new(AsyncMutex::new(agent)));
        Ok(constructed)
    }

    /// Get the agent by id.
    pub fn get_agent(&self, agent_id: &str) -> Option<SharedAgent> {
        let agents = self.agents.lock();
        agents.get(agent_id).cloned()
    }

    /// Add a connection between two agents in the specified patch.
    ///
    /// When the source agent outputs a value on the source handle (port),
    /// it will be delivered to the target agent's target handle (port).
    pub async fn add_connection(
        &self,
        patch_id: &str,
        connection: ConnectionSpec,
    ) -> Result<(), AgentError> {
        // check if the source and target agents exist
        {
            let agents = self.agents.lock();
            if !agents.contains_key(&connection.source) {
                return Err(AgentError::AgentNotFound(connection.source.to_string()));
            }
            if !agents.contains_key(&connection.target) {
                return Err(AgentError::AgentNotFound(connection.target.to_string()));
            }
        }

        // check if handles are valid
        if connection.source_handle.is_empty() {
            return Err(AgentError::EmptySourceHandle);
        }
        if connection.target_handle.is_empty() {
            return Err(AgentError::EmptyTargetHandle);
        }

        let patch = self
            .get_patch(patch_id)
            .ok_or_else(|| AgentError::PatchNotFound(patch_id.to_string()))?;
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

    fn add_connection_internal(&self, connection: ConnectionSpec) -> Result<(), AgentError> {
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
                return Err(AgentError::ConnectionAlreadyExists);
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

    /// Returns true if any connection originates from `source_agent`'s `port`.
    ///
    /// Producers can use this to skip building expensive values for ports
    /// nobody listens to; `agent_out` would only drop them after the
    /// conversion cost has already been paid.
    pub fn has_connections(&self, source_agent: &str, port: &str) -> bool {
        let connections = self.connections.lock();
        connections.get(source_agent).is_some_and(|targets| {
            targets
                .iter()
                .any(|(_, source_port, _)| source_port == port)
        })
    }

    /// Add agents and connections to the specified patch.
    ///
    /// The ids of the given agents and connections are changed to new unique ids.
    /// The agents are not started automatically, even if the patch is running.
    pub async fn add_agents_and_connections(
        &self,
        patch_id: &str,
        agents: &Vec<AgentSpec>,
        connections: &Vec<ConnectionSpec>,
    ) -> Result<(Vec<AgentSpec>, Vec<ConnectionSpec>), AgentError> {
        let (agents, connections) = update_ids(agents, connections);

        let patch = self
            .get_patch(patch_id)
            .ok_or_else(|| AgentError::PatchNotFound(patch_id.to_string()))?;
        let mut patch = patch.lock().await;

        // Track progress so a mid-batch failure can be rolled back: a
        // partial batch must not leave agents in the spec (or the runtime
        // maps) while returning an error without any event.
        let mut added_agents = 0;
        let mut added_connections = 0;
        let mut result = Ok(());

        // Collect the constructed specs (with dynamic configs/ports from
        // new()) so the patch and the caller both see the real state.
        let mut constructed_agents = Vec::with_capacity(agents.len());
        for agent in &agents {
            match self.add_agent_internal(patch_id.to_string(), agent.clone()) {
                Ok(constructed) => {
                    patch.add_agent(constructed.clone());
                    constructed_agents.push(constructed);
                    added_agents += 1;
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
            // The rolled-back agents were never started, so no stop or
            // channel teardown is needed; dropping the map entries undoes
            // add_agent_internal completely.
            let mut agents_map = self.agents.lock();
            for agent in agents.iter().take(added_agents) {
                patch.remove_agent(&agent.id);
                agents_map.swap_remove(&agent.id);
            }
            return Err(e);
        }
        drop(patch);

        self.emit_patch_structure_changed(patch_id.to_string());

        Ok((constructed_agents, connections))
    }

    /// Remove an agent from the specified patch.
    ///
    /// If the agent is running, it will be stopped first.
    pub async fn remove_agent(&self, patch_id: &str, agent_id: &str) -> Result<(), AgentError> {
        let patch = self
            .get_patch(patch_id)
            .ok_or_else(|| AgentError::PatchNotFound(patch_id.to_string()))?;

        // Tear down the runtime instance before touching the spec so a
        // failure leaves the spec unchanged and no spec change ever goes
        // unannounced. An agent can exist in the spec without a runtime
        // instance (its definition was unknown when the patch was added);
        // such an agent is still removable from the spec.
        let runtime_removed = match self.remove_agent_internal(agent_id).await {
            Ok(()) => true,
            Err(AgentError::AgentNotFound(_)) => false,
            Err(e) => return Err(e),
        };

        let spec_removed = {
            let mut patch = patch.lock().await;
            let count_before = patch.spec().agents.len();
            patch.remove_agent(agent_id);
            patch.spec().agents.len() != count_before
        };

        if !runtime_removed && !spec_removed {
            return Err(AgentError::AgentNotFound(agent_id.to_string()));
        }
        self.emit_patch_structure_changed(patch_id.to_string());
        Ok(())
    }

    async fn remove_agent_internal(&self, agent_id: &str) -> Result<(), AgentError> {
        self.stop_agent(agent_id).await?;

        // remove from connections
        {
            let mut connections = self.connections.lock();
            let mut sources_to_remove = Vec::new();
            for (source, targets) in connections.iter_mut() {
                targets.retain(|(target, _, _)| target != agent_id);
                if targets.is_empty() {
                    sources_to_remove.push(source.clone());
                }
            }
            for source in sources_to_remove {
                connections.swap_remove(&source);
            }
            connections.swap_remove(agent_id);
        }

        // remove from agents
        {
            let mut agents = self.agents.lock();
            agents.swap_remove(agent_id);
        }

        Ok(())
    }

    /// Remove a connection from the specified patch.
    pub async fn remove_connection(
        &self,
        patch_id: &str,
        connection: &ConnectionSpec,
    ) -> Result<(), AgentError> {
        let patch = self
            .get_patch(patch_id)
            .ok_or_else(|| AgentError::PatchNotFound(patch_id.to_string()))?;
        let mut patch = patch.lock().await;
        let Some(connection) = patch.remove_connection(connection) else {
            return Err(AgentError::ConnectionNotFound(format!(
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
    /// of every agent in the patch at once.
    ///
    /// The entry is kept (in its cancelled state) for the duration of the
    /// stop sequence so agent tokens renewed while agents are still being
    /// stopped are born cancelled and queued inputs are skipped instead of
    /// processed. [`Patch::stop`](crate::patch::Patch::stop) removes the
    /// entry once every agent has stopped, so a later `start_agent` derives
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

    /// Creates and tracks a fresh cancellation token for an agent as a child
    /// of its patch's parent token. The returned generation identifies the
    /// agent-loop incarnation that owns the slot.
    fn create_agent_token(&self, patch_id: &str, agent_id: &str) -> (u64, CancellationToken) {
        let generation = AGENT_TOKEN_GENERATION.fetch_add(1, Ordering::Relaxed);
        let token = self.patch_token(patch_id).child_token();
        self.agent_tokens
            .lock()
            .insert(agent_id.to_string(), (generation, token.clone()));
        (generation, token)
    }

    /// Replaces a fired agent token with a fresh child of the patch token.
    ///
    /// Called by the agent loop after its token fired. Returns `None` when
    /// the slot no longer belongs to the calling loop — either
    /// [`stop_agent`](Self::stop_agent) removed the entry, a restarted
    /// agent's new loop installed its own token (different generation), or
    /// the whole patch was removed. The caller then keeps its fired token
    /// so queued inputs are skipped until the `Stop` message arrives.
    fn renew_agent_token(
        &self,
        patch_id: &str,
        agent_id: &str,
        generation: u64,
    ) -> Option<CancellationToken> {
        // Look up (never create) the parent: a lagging loop must not
        // resurrect the token entry of a removed patch.
        let parent = self.patch_tokens.lock().get(patch_id).cloned()?;
        let fresh = parent.child_token();
        let mut tokens = self.agent_tokens.lock();
        let slot = tokens.get_mut(agent_id)?;
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
    /// Cancels the context's cancellation token, which every agent handling
    /// the flow received via [`AgentContext::cancel_token`]. Cancellation is
    /// cooperative for work already in flight: agents that `select!` on the
    /// token (LLM streaming loops, [`PatchToolAgent`](crate::tool::PatchToolAgent)
    /// result waits) abort promptly with [`AgentError::Cancelled`], while
    /// agents that ignore it run to completion. Inputs dispatched after the
    /// token fires are skipped before `process()` is called. The cancelled
    /// token stays alive as long as any context of the flow does, so queued
    /// and cyclic inputs for the flow are skipped too.
    ///
    /// Returns `false` when no live flow is tracked under `ctx_id` (the flow
    /// already finished, or never reached an agent): nothing is cancelled.
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

    /// Start an agent by id.
    ///
    /// Creates a message channel for the agent and spawns its event loop.
    /// The agent's [`start()`](crate::AsAgent::start) method is called, then
    /// the agent begins processing incoming messages.
    pub async fn start_agent(&self, agent_id: &str) -> Result<(), AgentError> {
        let agent = {
            let agents = self.agents.lock();
            let Some(a) = agents.get(agent_id) else {
                return Err(AgentError::AgentNotFound(agent_id.to_string()));
            };
            a.clone()
        };
        let (def_name, patch_id) = {
            let agent = agent.lock().await;
            (agent.def_name().to_string(), agent.patch_id().to_string())
        };
        if !self.defs.lock().contains_key(&def_name) {
            return Err(AgentError::AgentDefinitionNotFound(def_name));
        }
        let agent_status = {
            // This will not block since the agent is not started yet.
            let agent = agent.lock().await;
            agent.status().clone()
        };
        if agent_status == AgentStatus::Init {
            log::info!("Starting agent {}", agent_id);

            let (tx, mut rx) = mpsc::channel(MESSAGE_LIMIT);

            {
                let mut agent_txs = self.agent_txs.lock();
                agent_txs.insert(agent_id.to_string(), tx.clone());
            };

            let agent_clone = agent.clone();
            let agent_id_clone = agent_id.to_string();
            // base(): the agent loop outlives this call, so it must not
            // stamp runtime events with the caller's origin.
            let ma = self.base();
            // Created before spawning so stop_agent can cancel it immediately.
            let (generation, mut token) = self.create_agent_token(&patch_id, agent_id);

            let agent_loop = async move {
                // Race start() against the token too: a start() stuck on
                // slow I/O holds the agent lock, and without the race
                // stop_agent would block on that lock until start() returns
                // on its own.
                let start = async {
                    let mut agent_guard = agent_clone.lock().await;
                    agent_guard.start().await
                };
                tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        log::info!("Start cancelled: {}", agent_id_clone);
                        return;
                    }
                    r = start => {
                        if let Err(e) = r {
                            log::error!("Failed to start agent {}: {}", agent_id_clone, e);
                            return;
                        }
                    }
                }

                while let Some(message) = rx.recv().await {
                    match message {
                        AgentMessage::Input { ctx, port, value } => {
                            // Attach the flow's cancellation token so
                            // downstream awaits (tool result waits, LLM
                            // streams) can observe per-context aborts.
                            let ctx = if ctx.cancel_token().is_none() {
                                ctx.with_cancel_token(ma.context_token(ctx.id()))
                            } else {
                                ctx
                            };
                            let fut =
                                async { agent_clone.lock().await.process(ctx, port, value).await };
                            tokio::select! {
                                biased;
                                _ = token.cancelled() => {
                                    log::info!("Process cancelled: {}", agent_id_clone);
                                    // Dropping the future aborts any in-flight
                                    // I/O and releases the agent lock. A fired
                                    // token cannot be reset, so install a fresh
                                    // one unless this loop no longer owns the
                                    // token slot (agent stopping or restarted).
                                    if let Some(fresh) = ma.renew_agent_token(
                                        &patch_id,
                                        &agent_id_clone,
                                        generation,
                                    ) {
                                        token = fresh;
                                    }
                                }
                                r = fut => r.unwrap_or_else(|e| {
                                    log::error!("Process Error {}: {}", agent_id_clone, e);
                                }),
                            }
                        }
                        AgentMessage::Config { key, value } => {
                            agent_clone
                                .lock()
                                .await
                                .set_config(key, value)
                                .unwrap_or_else(|e| {
                                    log::error!("Config Error {}: {}", agent_id_clone, e);
                                });
                        }
                        AgentMessage::Configs { configs } => {
                            agent_clone
                                .lock()
                                .await
                                .set_configs(configs)
                                .unwrap_or_else(|e| {
                                    log::error!("Configs Error {}: {}", agent_id_clone, e);
                                });
                        }
                        AgentMessage::Stop => {
                            rx.close();
                            break;
                        }
                    }
                }
            };

            tokio::spawn(agent_loop);
        }
        Ok(())
    }

    /// Stop an agent by id.
    ///
    /// Sends a stop message to the agent, closes its message channel,
    /// and calls the agent's [`stop()`](crate::AsAgent::stop) method.
    pub async fn stop_agent(&self, agent_id: &str) -> Result<(), AgentError> {
        {
            // remove the sender first to prevent new messages being sent
            let mut agent_txs = self.agent_txs.lock();
            if let Some(tx) = agent_txs.swap_remove(agent_id)
                && let Err(e) = tx.try_send(AgentMessage::Stop)
            {
                log::warn!("Failed to send stop message to agent {}: {}", agent_id, e);
            }
        }

        // Cancel BEFORE awaiting the agent lock: a long-running process()
        // holds the lock, and cancelling makes the agent loop drop that
        // future (releasing the lock) instead of blocking stop until it
        // completes. Removing the entry first keeps the fired token in the
        // loop so inputs queued ahead of Stop are skipped rather than
        // processed with a renewed token.
        let token = self.agent_tokens.lock().swap_remove(agent_id);
        if let Some((_, token)) = token {
            token.cancel();
        }

        let agent = {
            let agents = self.agents.lock();
            let Some(a) = agents.get(agent_id) else {
                return Err(AgentError::AgentNotFound(agent_id.to_string()));
            };
            a.clone()
        };
        let mut agent_guard = agent.lock().await;
        if *agent_guard.status() == AgentStatus::Start {
            log::info!("Stopping agent {}", agent_id);
            agent_guard.stop().await?;
        }

        Ok(())
    }

    /// Set configs for an agent by id.
    ///
    /// Emits [`ModularAgentEvent::AgentConfigUpdated`] for each key once the
    /// configs have been handed to the agent. When the agent is running, the
    /// configs travel through its message channel and are applied
    /// asynchronously: the events report successful delivery, not completed
    /// application. Events are emitted regardless of whether a key's value
    /// actually changed.
    ///
    /// An agent with no live instance (its definition is not registered in
    /// this build) has the configs merged into its stored patch spec entry,
    /// so the edit survives a save.
    pub async fn set_agent_configs(
        &self,
        agent_id: String,
        configs: AgentConfigs,
    ) -> Result<(), AgentError> {
        let tx = {
            let agent_txs = self.agent_txs.lock();
            agent_txs.get(&agent_id).cloned()
        };

        let Some(tx) = tx else {
            // The agent is not running. We can set the configs directly.
            let agent = {
                let agents = self.agents.lock();
                agents.get(&agent_id).cloned()
            };
            let Some(agent) = agent else {
                // A spec-only agent has no instance to configure, so write
                // through to its stored spec entry instead; otherwise the
                // edit would be lost on the next save. Same event contract
                // as the live branch below - per-key AgentConfigUpdated, no
                // AgentSpecUpdated - so hosts cannot tell the two apart.
                let configs_value = serde_json::to_value(&configs)
                    .map_err(|e| AgentError::SerializationError(e.to_string()))?;
                let patch = serde_json::json!({ "configs": configs_value });
                let Some((_, updated)) = self.patch_stored_agent_spec(&agent_id, &patch).await
                else {
                    return Err(AgentError::AgentNotFound(agent_id.to_string()));
                };
                updated?;
                for (key, value) in configs {
                    self.emit_agent_config_updated(agent_id.clone(), key, value);
                }
                return Ok(());
            };
            agent.lock().await.set_configs(configs.clone())?;
            for (key, value) in configs {
                self.emit_agent_config_updated(agent_id.clone(), key, value);
            }
            return Ok(());
        };
        let message = AgentMessage::Configs {
            configs: configs.clone(),
        };
        tx.send(message).await.map_err(|_| {
            AgentError::SendMessageFailed("Failed to send config message".to_string())
        })?;
        for (key, value) in configs {
            self.emit_agent_config_updated(agent_id.clone(), key, value);
        }
        Ok(())
    }

    /// Get global configs for the agent definition by name.
    pub fn get_global_configs(&self, def_name: &str) -> Option<AgentConfigs> {
        let global_configs_map = self.global_configs_map.lock();
        global_configs_map.get(def_name).cloned()
    }

    /// Set global configs for the agent definition by name.
    pub fn set_global_configs(&self, def_name: String, configs: AgentConfigs) {
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
    pub fn get_global_configs_map(&self) -> AgentConfigsMap {
        let global_configs_map = self.global_configs_map.lock();
        global_configs_map.clone()
    }

    /// Set the global configs map.
    pub fn set_global_configs_map(&self, new_configs_map: AgentConfigsMap) {
        for (agent_name, new_configs) in new_configs_map {
            self.set_global_configs(agent_name, new_configs);
        }
    }

    /// Send input to an agent.
    pub(crate) async fn agent_input(
        &self,
        agent_id: String,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let message = if let Some(config_key) = port.strip_prefix("config:") {
            AgentMessage::Config {
                key: config_key.to_string(),
                value,
            }
        } else {
            AgentMessage::Input {
                ctx,
                port: port.clone(),
                value,
            }
        };

        let tx = {
            let agent_txs = self.agent_txs.lock();
            agent_txs.get(&agent_id).cloned()
        };

        let Some(tx) = tx else {
            // The agent is not running. If it's a config message, we can set it directly.
            let agent: SharedAgent = {
                let agents = self.agents.lock();
                let Some(a) = agents.get(&agent_id) else {
                    return Err(AgentError::AgentNotFound(agent_id.to_string()));
                };
                a.clone()
            };
            if let AgentMessage::Config { key, value } = message {
                agent.lock().await.set_config(key, value)?;
            }
            return Ok(());
        };
        tx.send(message).await.map_err(|_| {
            AgentError::SendMessageFailed("Failed to send input message".to_string())
        })?;

        self.emit_agent_input(agent_id.to_string(), port);

        Ok(())
    }

    /// Send output from an agent. (Async version)
    pub async fn send_agent_out(
        &self,
        agent_id: String,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        message::send_agent_out(self, agent_id, ctx, port, value).await
    }

    /// Send output from an agent.
    pub fn try_send_agent_out(
        &self,
        agent_id: String,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        message::try_send_agent_out(self, agent_id, ctx, port, value)
    }

    /// Write a value to a named channel.
    ///
    /// This is the primary method for sending external input into the agent network.
    /// The value will be delivered to all [`ExternalInputAgent`](crate::external_agent::ExternalInputAgent)
    /// instances listening to the specified channel name, which will then forward it to
    /// their connected agents.
    ///
    /// # Arguments
    ///
    /// * `name` - The channel name to write to. Must match the `name` config of an `ExternalInputAgent`.
    /// * `value` - The value to send.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use modular_agent_core::{ModularAgent, AgentValue};
    /// # async fn example(ma: ModularAgent) {
    /// // Send a string to the "input" channel
    /// ma.write_external_input("input".to_string(), AgentValue::string("hello")).await.unwrap();
    ///
    /// // Send an integer
    /// ma.write_external_input("numbers".to_string(), AgentValue::integer(42)).await.unwrap();
    /// # }
    /// ```
    pub async fn write_external_input(
        &self,
        name: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.send_external_output(name, AgentContext::new(), value)
            .await
    }

    /// Write a value to the local variable channel.
    pub async fn write_local_input(
        &self,
        patch_id: &str,
        name: &str,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let channel_name = format!("%{}/{}", patch_id, name);
        self.send_external_output(channel_name, AgentContext::new(), value)
            .await
    }

    pub(crate) async fn send_external_output(
        &self,
        name: String,
        ctx: AgentContext,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        message::send_external_output(self, name, ctx, value).await
    }

    async fn spawn_message_loop(&self) -> Result<(), AgentError> {
        // TODO: settings for the channel size
        let (tx, mut rx) = mpsc::channel(4096);
        {
            let mut tx_lock = self.tx.lock();
            *tx_lock = Some(tx);
        }

        // spawn the main loop; base() so events emitted while routing
        // messages are never attributed to the caller of ready().
        let ma = self.base();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                use AgentEventMessage::*;

                match message {
                    AgentOut {
                        agent,
                        ctx,
                        port,
                        value,
                    } => {
                        message::agent_out(&ma, agent, ctx, port, value).await;
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
    /// use modular_agent_core::{ModularAgent, ModularAgentEvent, AgentValue};
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

        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(envelope) => {
                        if let Some(mapped_event) = filter_map(envelope)
                            && tx.send(mapped_event).is_err()
                        {
                            // Receiver dropped, task can exit
                            break;
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        log::warn!("Event subscriber lagged by {} events", n);
                    }
                    Err(RecvError::Closed) => {
                        // Sender dropped, task can exit
                        break;
                    }
                }
            }
        });
        rx
    }

    pub(crate) fn emit_agent_config_updated(
        &self,
        agent_id: String,
        key: String,
        value: AgentValue,
    ) {
        self.notify_observers(ModularAgentEvent::AgentConfigUpdated(agent_id, key, value));
    }

    pub(crate) fn emit_agent_error(&self, agent_id: String, message: String) {
        self.notify_observers(ModularAgentEvent::AgentError(agent_id, message));
    }

    pub(crate) fn emit_agent_input(&self, agent_id: String, port: String) {
        self.notify_observers(ModularAgentEvent::AgentIn(agent_id, port));
    }

    pub(crate) fn emit_agent_spec_updated(&self, agent_id: String) {
        self.notify_observers(ModularAgentEvent::AgentSpecUpdated(agent_id));
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

    pub(crate) fn emit_external_output(&self, name: String, value: AgentValue) {
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

/// Whether an agent spec patch warrants a `PatchStructureChanged`.
///
/// Any non-config key (ports, title, layout, ...) may change how hosts render
/// the patch, so treat those patches as structural. Config-only patches stay
/// quiet here; they are covered by `AgentSpecUpdated`.
fn is_structural_spec_patch(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|map| map.keys().any(|key| key != "configs"))
}

/// Carrier for a [`ModularAgentEvent`] together with the origin of the change.
///
/// `origin` identifies the entry point that performed the mutation which
/// produced the event (see [`ModularAgent::with_origin`]). `None` means the
/// event originated inside the agent runtime itself.
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
    /// An agent's configuration was updated.
    ///
    /// Fields: `(agent_id, config_key, new_value)`
    AgentConfigUpdated(String, String, AgentValue),

    /// An agent encountered an error.
    ///
    /// Fields: `(agent_id, error_message)`
    AgentError(String, String),

    /// An agent received input on a port.
    ///
    /// Fields: `(agent_id, port_name)`
    AgentIn(String, String),

    /// An agent's spec was updated.
    ///
    /// Fields: `(agent_id)`
    AgentSpecUpdated(String),

    /// A patch's structure (agents, connections, or non-config spec keys)
    /// was changed.
    ///
    /// Emitted by [`ModularAgent::add_agent`], [`ModularAgent::remove_agent`],
    /// [`ModularAgent::add_connection`], [`ModularAgent::remove_connection`],
    /// [`ModularAgent::add_agents_and_connections`],
    /// [`ModularAgent::update_patch_spec`], and by
    /// [`ModularAgent::update_agent_spec`] when the patch contains keys other
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
    /// agents have been torn down, so hosts can close any view of it.
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
    /// - An [`ExternalOutputAgent`](crate::external_agent::ExternalOutputAgent) receives a value
    ///
    /// Fields: `(channel_name, value)`
    ExternalOutput(String, AgentValue),
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
}
