#[cfg(feature = "file")]
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::sync::{Mutex as AsyncMutex, broadcast, broadcast::error::RecvError, mpsc};

use crate::FnvIndexMap;
use crate::agent::{Agent, AgentMessage, AgentStatus, agent_new};
use crate::config::{AgentConfigs, AgentConfigsMap};
use crate::context::AgentContext;
use crate::definition::{AgentConfigSpecs, AgentDefinition, AgentDefinitions};
use crate::error::AgentError;
use crate::id::{new_id, update_ids};
use crate::message::{self, AgentEventMessage};
use crate::preset::{Preset, PresetInfo};
use crate::registry;
use crate::spec::{AgentSpec, ConnectionSpec, PresetSpec};
use crate::value::AgentValue;

const MESSAGE_LIMIT: usize = 1024;
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// The central orchestrator for the modular agent system.
///
/// `ModularAgent` manages agent lifecycle, connections, and message routing.
/// It maintains agent instances, connection maps, and handles [`ModularAgentEvent`]s.
///
/// # Lifecycle
///
/// 1. [`init()`](Self::init) - Create instance and register agent definitions
/// 2. [`ready()`](Self::ready) - Start the internal message loop
/// 3. Load presets with [`open_preset_from_file()`](Self::open_preset_from_file) or [`add_preset()`](Self::add_preset)
/// 4. [`start_preset()`](Self::start_preset) - Start agents in a preset
/// 5. Interact via [`write_external_input()`](Self::write_external_input) and [`subscribe()`](Self::subscribe)
/// 6. [`stop_preset()`](Self::stop_preset) - Stop agents
/// 7. [`quit()`](Self::quit) - Shut down
///
/// # Example
///
/// ```rust,no_run
/// use modular_agent_core::{ModularAgent, AgentValue, ModularAgentEvent};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Initialize and start
///     let ma = ModularAgent::init()?;
///     ma.ready().await?;
///
///     // Load a preset
///     let preset_id = ma.open_preset_from_file("my_preset.json", None).await?;
///     ma.start_preset(&preset_id).await?;
///
///     // Send external input
///     ma.write_external_input("input".to_string(), AgentValue::string("hello")).await?;
///
///     // Cleanup
///     ma.stop_preset(&preset_id).await?;
///     ma.quit();
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct ModularAgent {
    // agent id -> agent
    pub(crate) agents: Arc<Mutex<FnvIndexMap<String, Arc<AsyncMutex<Box<dyn Agent>>>>>>,

    // agent id -> sender
    pub(crate) agent_txs: Arc<Mutex<FnvIndexMap<String, mpsc::Sender<AgentMessage>>>>,

    // channel name -> [external input agent id]
    pub(crate) external_input_agents: Arc<Mutex<FnvIndexMap<String, Vec<String>>>>,

    // channel name -> value
    pub(crate) external_values: Arc<Mutex<FnvIndexMap<String, AgentValue>>>,

    // source agent id -> [target agent id / source handle / target handle]
    pub(crate) connections: Arc<Mutex<FnvIndexMap<String, Vec<(String, String, String)>>>>,

    // agent def name -> agent definition
    pub(crate) defs: Arc<Mutex<AgentDefinitions>>,

    // presets (preset id -> preset)
    pub(crate) presets: Arc<Mutex<FnvIndexMap<String, Arc<AsyncMutex<Preset>>>>>,

    // agent def name -> config
    pub(crate) global_configs_map: Arc<Mutex<FnvIndexMap<String, AgentConfigs>>>,

    // message sender
    pub(crate) tx: Arc<Mutex<Option<mpsc::Sender<AgentEventMessage>>>>,

    // observers
    pub(crate) observers: broadcast::Sender<ModularAgentEvent>,
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
            presets: Default::default(),
            global_configs_map: Default::default(),
            tx: Arc::new(Mutex::new(None)),
            observers: tx,
        }
    }

    pub(crate) fn tx(&self) -> Result<mpsc::Sender<AgentEventMessage>, AgentError> {
        self.tx
            .lock()
            .unwrap()
            .clone()
            .ok_or(AgentError::TxNotInitialized)
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
    /// This must be called after [`init`](Self::init) before loading presets or sending messages.
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
    /// This stops the internal message loop. Call [`stop_preset`](Self::stop_preset)
    /// for each running preset before calling this method for graceful shutdown.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use modular_agent_core::ModularAgent;
    /// # async fn example(ma: ModularAgent, preset_id: &str) {
    /// // Stop all presets first
    /// ma.stop_preset(preset_id).await.unwrap();
    /// // Then quit
    /// ma.quit();
    /// # }
    /// ```
    pub fn quit(&self) {
        let mut tx_lock = self.tx.lock().unwrap();
        *tx_lock = None;
    }

    // Preset management

    /// Create a new empty preset.
    ///
    /// Returns the id of the new preset. The preset is created with default settings
    /// and contains no agents or connections initially.
    pub fn new_preset(&self) -> Result<String, AgentError> {
        let spec = PresetSpec::default();
        let id = self.add_preset(spec)?;
        Ok(id)
    }

    /// Create a new empty preset with the given name.
    ///
    /// Returns the id of the new preset.
    pub fn new_preset_with_name(&self, name: String) -> Result<String, AgentError> {
        let spec = PresetSpec::default();
        let id = self.add_preset_with_name(spec, name)?;
        Ok(id)
    }

    /// Get a preset by id.
    ///
    /// Returns `None` if no preset exists with the given id.
    pub fn get_preset(&self, id: &str) -> Option<Arc<AsyncMutex<Preset>>> {
        let presets = self.presets.lock().unwrap();
        presets.get(id).cloned()
    }

    /// Add a new preset with the given spec, and returns the id of the new preset.
    ///
    /// The ids of the given spec, including agents and connections, are changed to new unique ids.
    /// This allows the same spec to be added multiple times without id conflicts.
    pub fn add_preset(&self, spec: PresetSpec) -> Result<String, AgentError> {
        self.add_preset_raw(spec, None)
    }

    /// Add a new preset with the given name and spec, and returns the id of the new preset.
    ///
    /// The ids of the given spec, including agents and connections, are changed to new unique ids.
    pub fn add_preset_with_name(
        &self,
        spec: PresetSpec,
        name: String,
    ) -> Result<String, AgentError> {
        self.add_preset_raw(spec, Some(name))
    }

    fn add_preset_raw(&self, spec: PresetSpec, name: Option<String>) -> Result<String, AgentError> {
        let mut preset = Preset::new(spec);
        if let Some(name) = name {
            preset.set_name(name);
        }
        let id = preset.id().to_string();

        // add agents
        for agent in &preset.spec().agents {
            if let Err(e) = self.add_agent_internal(id.clone(), agent.clone()) {
                log::error!("Failed to add_agent {}: {}", agent.id, e);
            }
        }

        // add connections
        for connection in &preset.spec().connections {
            self.add_connection_internal(connection.clone())
                .unwrap_or_else(|e| {
                    log::error!("Failed to add_connection {}: {}", connection.source, e);
                });
        }

        // add the given preset into presets
        let mut presets = self.presets.lock().unwrap();
        if presets.contains_key(&id) {
            return Err(AgentError::DuplicateId(id.into()));
        }
        presets.insert(id.to_string(), Arc::new(AsyncMutex::new(preset)));

        Ok(id)
    }

    /// Rename a preset by id.
    pub async fn rename_preset(&self, id: &str, new_name: String) -> Result<(), AgentError> {
        let preset = self
            .get_preset(id)
            .ok_or_else(|| AgentError::PresetNotFound(id.to_string()))?;
        let mut preset = preset.lock().await;
        preset.set_name(new_name);
        Ok(())
    }

    /// Remove a preset by id.
    ///
    /// Stops the preset if running, then removes all associated agents and connections.
    pub async fn remove_preset(&self, id: &str) -> Result<(), AgentError> {
        let preset = self
            .get_preset(id)
            .ok_or_else(|| AgentError::PresetNotFound(id.to_string()))?;

        let mut preset = preset.lock().await;
        preset.stop(self).await.unwrap_or_else(|e| {
            log::error!("Failed to stop preset {}: {}", id, e);
        });

        // Remove all agents and connections associated with the preset
        for agent in &preset.spec().agents {
            self.remove_agent_internal(&agent.id)
                .await
                .unwrap_or_else(|e| {
                    log::error!("Failed to remove_agent {}: {}", agent.id, e);
                });
        }
        for connection in &preset.spec().connections {
            self.remove_connection_internal(connection);
        }

        // Drop the preset lock before modifying the presets map
        drop(preset);

        // Remove the preset entry from the map
        {
            let mut presets = self.presets.lock().unwrap();
            presets.swap_remove(id);
        }

        Ok(())
    }

    /// Start a preset by id.
    ///
    /// This starts all agents in the preset, enabling message flow between them.
    /// Each agent's [`start()`](crate::AsAgent::start) method is called.
    pub async fn start_preset(&self, id: &str) -> Result<(), AgentError> {
        let preset = self
            .get_preset(id)
            .ok_or_else(|| AgentError::PresetNotFound(id.to_string()))?;
        let mut preset = preset.lock().await;
        preset.start(self).await?;

        Ok(())
    }

    /// Stop a preset by id.
    ///
    /// This stops all agents in the preset, terminating message processing.
    /// Each agent's [`stop()`](crate::AsAgent::stop) method is called.
    pub async fn stop_preset(&self, id: &str) -> Result<(), AgentError> {
        let preset = self
            .get_preset(id)
            .ok_or_else(|| AgentError::PresetNotFound(id.to_string()))?;
        let mut preset = preset.lock().await;
        preset.stop(self).await?;

        Ok(())
    }

    /// Open a preset from a JSON file.
    ///
    /// Reads the file, parses the JSON as a [`PresetSpec`], and adds it to the system.
    /// Optionally provide a custom name for the preset.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON preset file
    /// * `name` - Optional custom name for the preset
    #[cfg(feature = "file")]
    pub async fn open_preset_from_file(
        &self,
        path: &str,
        name: Option<String>,
    ) -> Result<String, AgentError> {
        let json_str =
            std::fs::read_to_string(path).map_err(|e| AgentError::IoError(e.to_string()))?;
        let spec = PresetSpec::from_json(&json_str)?;
        let id = self.add_preset_raw(spec, name)?;
        Ok(id)
    }

    /// Save a preset to a JSON file.
    ///
    /// Serializes the current preset state (including agent configs) to JSON
    /// and writes it to the specified path.
    #[cfg(feature = "file")]
    pub async fn save_preset(&self, id: &str, path: &str) -> Result<(), AgentError> {
        let Some(preset_spec) = self.get_preset_spec(id).await else {
            return Err(AgentError::PresetNotFound(id.to_string()));
        };
        let json_str = preset_spec.to_json()?;
        std::fs::write(path, json_str).map_err(|e| AgentError::IoError(e.to_string()))?;
        Ok(())
    }

    // PresetSpec

    /// Get the current preset spec by id.
    pub async fn get_preset_spec(&self, id: &str) -> Option<PresetSpec> {
        let Some(preset) = self.get_preset(id) else {
            return None;
        };
        let mut preset_spec = {
            let preset = preset.lock().await;
            preset.spec().clone()
        };

        // collect current agent specs in the preset
        let mut agent_specs = Vec::new();
        for agent in &preset_spec.agents {
            if let Some(spec) = self.get_agent_spec(&agent.id).await {
                agent_specs.push(spec);
            }
        }
        preset_spec.agents = agent_specs;

        // No need to change connections

        Some(preset_spec)
    }

    /// Update the preset spec
    pub async fn update_preset_spec(&self, id: &str, value: &Value) -> Result<(), AgentError> {
        let preset = self
            .get_preset(id)
            .ok_or_else(|| AgentError::PresetNotFound(id.to_string()))?;
        let mut preset = preset.lock().await;
        preset.update_spec(value)?;
        Ok(())
    }

    // PresetInfo

    /// Get info of the preset by id.
    pub async fn get_preset_info(&self, id: &str) -> Option<PresetInfo> {
        let Some(preset) = self.get_preset(id) else {
            return None;
        };
        Some(PresetInfo::from(&*preset.lock().await))
    }

    /// Get infos of all presets.
    pub async fn get_preset_infos(&self) -> Vec<PresetInfo> {
        let presets = {
            let presets = self.presets.lock().unwrap();
            presets.values().cloned().collect::<Vec<_>>()
        };
        let mut preset_infos = Vec::new();
        for preset in presets {
            let preset_guard = preset.lock().await;
            preset_infos.push(PresetInfo::from(&*preset_guard));
        }
        preset_infos
    }

    // Agents

    /// Register an agent definition.
    ///
    /// This makes the agent type available for use in presets. The definition
    /// includes metadata (title, category), input/output ports, and config specs.
    ///
    /// Note: Agents using `#[modular_agent]` macro are registered automatically via inventory.
    pub fn register_agent_definiton(&self, def: AgentDefinition) {
        let def_name = def.name.clone();
        let def_global_configs = def.global_configs.clone();

        let mut defs = self.defs.lock().unwrap();
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
        let defs = self.defs.lock().unwrap();
        defs.clone()
    }

    /// Get an agent definition by name.
    ///
    /// The name is typically in the format `module::path::StructName`.
    pub fn get_agent_definition(&self, def_name: &str) -> Option<AgentDefinition> {
        let defs = self.defs.lock().unwrap();
        defs.get(def_name).cloned()
    }

    /// Get the config specs of an agent definition by name.
    pub fn get_agent_config_specs(&self, def_name: &str) -> Option<AgentConfigSpecs> {
        let defs = self.defs.lock().unwrap();
        let Some(def) = defs.get(def_name) else {
            return None;
        };
        def.configs.clone()
    }

    /// Get the agent spec by id.
    pub async fn get_agent_spec(&self, agent_id: &str) -> Option<AgentSpec> {
        let agent = {
            let agents = self.agents.lock().unwrap();
            let Some(agent) = agents.get(agent_id) else {
                return None;
            };
            agent.clone()
        };
        let agent = agent.lock().await;
        Some(agent.spec().clone())
    }

    /// Update the agent spec by id.
    pub async fn update_agent_spec(&self, agent_id: &str, value: &Value) -> Result<(), AgentError> {
        let agent = {
            let agents = self.agents.lock().unwrap();
            let Some(agent) = agents.get(agent_id) else {
                return Err(AgentError::AgentNotFound(agent_id.to_string()));
            };
            agent.clone()
        };
        let mut agent = agent.lock().await;
        agent.update_spec(value)?;
        Ok(())
    }

    /// Create a new agent spec from the given agent definition name.
    pub fn new_agent_spec(&self, def_name: &str) -> Result<AgentSpec, AgentError> {
        let def = self
            .get_agent_definition(def_name)
            .ok_or_else(|| AgentError::AgentDefinitionNotFound(def_name.to_string()))?;
        Ok(def.to_spec())
    }

    /// Add an agent to the specified preset.
    ///
    /// Creates a new agent instance from the given spec and adds it to the preset.
    /// Returns the id of the newly created agent. The agent is not started automatically;
    /// call [`start_preset`](Self::start_preset) or [`start_agent`](Self::start_agent) to start it.
    pub async fn add_agent(
        &self,
        preset_id: String,
        mut spec: AgentSpec,
    ) -> Result<String, AgentError> {
        let preset = self
            .get_preset(&preset_id)
            .ok_or_else(|| AgentError::PresetNotFound(preset_id.to_string()))?;

        let id = new_id();
        spec.id = id.clone();
        self.add_agent_internal(preset_id, spec.clone())?;

        let mut preset = preset.lock().await;
        preset.add_agent(spec.clone());

        Ok(id)
    }

    fn add_agent_internal(&self, preset_id: String, spec: AgentSpec) -> Result<(), AgentError> {
        let mut agents = self.agents.lock().unwrap();
        if agents.contains_key(&spec.id) {
            return Err(AgentError::AgentAlreadyExists(spec.id.to_string()));
        }
        let spec_id = spec.id.clone();
        let mut agent = agent_new(self.clone(), spec_id.clone(), spec)?;
        agent.set_preset_id(preset_id);
        agents.insert(spec_id, Arc::new(AsyncMutex::new(agent)));
        Ok(())
    }

    /// Get the agent by id.
    pub fn get_agent(&self, agent_id: &str) -> Option<Arc<AsyncMutex<Box<dyn Agent>>>> {
        let agents = self.agents.lock().unwrap();
        agents.get(agent_id).cloned()
    }

    /// Add a connection between two agents in the specified preset.
    ///
    /// When the source agent outputs a value on the source handle (port),
    /// it will be delivered to the target agent's target handle (port).
    pub async fn add_connection(
        &self,
        preset_id: &str,
        connection: ConnectionSpec,
    ) -> Result<(), AgentError> {
        // check if the source and target agents exist
        {
            let agents = self.agents.lock().unwrap();
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

        let preset = self
            .get_preset(preset_id)
            .ok_or_else(|| AgentError::PresetNotFound(preset_id.to_string()))?;
        let mut preset = preset.lock().await;
        preset.add_connection(connection.clone());
        self.add_connection_internal(connection)?;
        Ok(())
    }

    fn add_connection_internal(&self, connection: ConnectionSpec) -> Result<(), AgentError> {
        let mut connections = self.connections.lock().unwrap();
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

    /// Add agents and connections to the specified preset.
    ///
    /// The ids of the given agents and connections are changed to new unique ids.
    /// The agents are not started automatically, even if the preset is running.
    pub async fn add_agents_and_connections(
        &self,
        preset_id: &str,
        agents: &Vec<AgentSpec>,
        connections: &Vec<ConnectionSpec>,
    ) -> Result<(Vec<AgentSpec>, Vec<ConnectionSpec>), AgentError> {
        let (agents, connections) = update_ids(agents, connections);

        let preset = self
            .get_preset(preset_id)
            .ok_or_else(|| AgentError::PresetNotFound(preset_id.to_string()))?;
        let mut preset = preset.lock().await;

        for agent in &agents {
            self.add_agent_internal(preset_id.to_string(), agent.clone())?;
            preset.add_agent(agent.clone());
        }

        for connection in &connections {
            self.add_connection_internal(connection.clone())?;
            preset.add_connection(connection.clone());
        }

        Ok((agents, connections))
    }

    /// Remove an agent from the specified preset.
    ///
    /// If the agent is running, it will be stopped first.
    pub async fn remove_agent(&self, preset_id: &str, agent_id: &str) -> Result<(), AgentError> {
        {
            let preset = self
                .get_preset(preset_id)
                .ok_or_else(|| AgentError::PresetNotFound(preset_id.to_string()))?;
            let mut preset = preset.lock().await;
            preset.remove_agent(agent_id);
        }
        if let Err(e) = self.remove_agent_internal(agent_id).await {
            return Err(e);
        }
        Ok(())
    }

    async fn remove_agent_internal(&self, agent_id: &str) -> Result<(), AgentError> {
        self.stop_agent(agent_id).await?;

        // remove from connections
        {
            let mut connections = self.connections.lock().unwrap();
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
            let mut agents = self.agents.lock().unwrap();
            agents.swap_remove(agent_id);
        }

        Ok(())
    }

    /// Remove a connection from the specified preset.
    pub async fn remove_connection(
        &self,
        preset_id: &str,
        connection: &ConnectionSpec,
    ) -> Result<(), AgentError> {
        let preset = self
            .get_preset(preset_id)
            .ok_or_else(|| AgentError::PresetNotFound(preset_id.to_string()))?;
        let mut preset = preset.lock().await;
        let Some(connection) = preset.remove_connection(connection) else {
            return Err(AgentError::ConnectionNotFound(format!(
                "{}:{}->{}:{}",
                connection.source,
                connection.source_handle,
                connection.target,
                connection.target_handle
            )));
        };
        self.remove_connection_internal(&connection);
        Ok(())
    }

    fn remove_connection_internal(&self, connection: &ConnectionSpec) {
        let mut connections = self.connections.lock().unwrap();
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

    /// Start an agent by id.
    ///
    /// Creates a message channel for the agent and spawns its event loop.
    /// The agent's [`start()`](crate::AsAgent::start) method is called, then
    /// the agent begins processing incoming messages.
    ///
    /// If the agent's definition has `native_thread = true`, the agent runs
    /// on a dedicated OS thread instead of the tokio runtime.
    pub async fn start_agent(&self, agent_id: &str) -> Result<(), AgentError> {
        let agent = {
            let agents = self.agents.lock().unwrap();
            let Some(a) = agents.get(agent_id) else {
                return Err(AgentError::AgentNotFound(agent_id.to_string()));
            };
            a.clone()
        };
        let def_name = {
            let agent = agent.lock().await;
            agent.def_name().to_string()
        };
        let uses_native_thread = {
            let defs = self.defs.lock().unwrap();
            let Some(def) = defs.get(&def_name) else {
                return Err(AgentError::AgentDefinitionNotFound(agent_id.to_string()));
            };
            def.native_thread
        };
        let agent_status = {
            // This will not block since the agent is not started yet.
            let agent = agent.lock().await;
            agent.status().clone()
        };
        if agent_status == AgentStatus::Init {
            log::info!("Starting agent {}", agent_id);

            let (tx, mut rx) = mpsc::channel(MESSAGE_LIMIT);

            {
                let mut agent_txs = self.agent_txs.lock().unwrap();
                agent_txs.insert(agent_id.to_string(), tx.clone());
            };

            let agent_clone = agent.clone();
            let agent_id_clone = agent_id.to_string();

            let agent_loop = async move {
                {
                    let mut agent_guard = agent_clone.lock().await;
                    if let Err(e) = agent_guard.start().await {
                        log::error!("Failed to start agent {}: {}", agent_id_clone, e);
                        return;
                    }
                }

                while let Some(message) = rx.recv().await {
                    match message {
                        AgentMessage::Input { ctx, port, value } => {
                            agent_clone
                                .lock()
                                .await
                                .process(ctx, port, value)
                                .await
                                .unwrap_or_else(|e| {
                                    log::error!("Process Error {}: {}", agent_id_clone, e);
                                });
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

            if uses_native_thread {
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    rt.block_on(agent_loop);
                });
            } else {
                tokio::spawn(agent_loop);
            }
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
            let mut agent_txs = self.agent_txs.lock().unwrap();
            if let Some(tx) = agent_txs.swap_remove(agent_id) {
                if let Err(e) = tx.try_send(AgentMessage::Stop) {
                    log::warn!("Failed to send stop message to agent {}: {}", agent_id, e);
                }
            }
        }

        let agent = {
            let agents = self.agents.lock().unwrap();
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
    pub async fn set_agent_configs(
        &self,
        agent_id: String,
        configs: AgentConfigs,
    ) -> Result<(), AgentError> {
        let tx = {
            let agent_txs = self.agent_txs.lock().unwrap();
            agent_txs.get(&agent_id).cloned()
        };

        let Some(tx) = tx else {
            // The agent is not running. We can set the configs directly.
            let agent = {
                let agents = self.agents.lock().unwrap();
                let Some(a) = agents.get(&agent_id) else {
                    return Err(AgentError::AgentNotFound(agent_id.to_string()));
                };
                a.clone()
            };
            agent.lock().await.set_configs(configs.clone())?;
            return Ok(());
        };
        let message = AgentMessage::Configs { configs };
        tx.send(message).await.map_err(|_| {
            AgentError::SendMessageFailed("Failed to send config message".to_string())
        })?;
        Ok(())
    }

    /// Get global configs for the agent definition by name.
    pub fn get_global_configs(&self, def_name: &str) -> Option<AgentConfigs> {
        let global_configs_map = self.global_configs_map.lock().unwrap();
        global_configs_map.get(def_name).cloned()
    }

    /// Set global configs for the agent definition by name.
    pub fn set_global_configs(&self, def_name: String, configs: AgentConfigs) {
        let mut global_configs_map = self.global_configs_map.lock().unwrap();

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
        let global_configs_map = self.global_configs_map.lock().unwrap();
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
        let message = if port.starts_with("config:") {
            let config_key = port[7..].to_string();
            AgentMessage::Config {
                key: config_key,
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
            let agent_txs = self.agent_txs.lock().unwrap();
            agent_txs.get(&agent_id).cloned()
        };

        let Some(tx) = tx else {
            // The agent is not running. If it's a config message, we can set it directly.
            let agent: Arc<AsyncMutex<Box<dyn Agent>>> = {
                let agents = self.agents.lock().unwrap();
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
        self.send_external_output(name, AgentContext::new(), value).await
    }

    /// Write a value to the local variable channel.
    pub async fn write_local_input(
        &self,
        preset_id: &str,
        name: &str,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let channel_name = format!("%{}/{}", preset_id, name);
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
            let mut tx_lock = self.tx.lock().unwrap();
            *tx_lock = Some(tx);
        }

        // spawn the main loop
        let ma = self.clone();
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
    /// Returns a broadcast receiver that receives all [`ModularAgentEvent`]s.
    /// For filtered subscriptions, use [`subscribe_to_event`](Self::subscribe_to_event).
    ///
    /// **Note**: Subscribe before starting presets to avoid missing events.
    pub fn subscribe(&self) -> broadcast::Receiver<ModularAgentEvent> {
        self.observers.subscribe()
    }

    /// Subscribe to filtered [`ModularAgentEvent`]s.
    ///
    /// This method creates a filtered subscription to events. The provided closure
    /// filters and maps events, and only successfully mapped events are forwarded
    /// to the returned receiver.
    ///
    /// **Important**: Subscribe to events BEFORE starting presets to avoid missing
    /// events due to race conditions.
    ///
    /// # Arguments
    ///
    /// * `filter_map` - A closure that receives each event and returns `Some(T)` for
    ///   events you want to receive, or `None` to skip them.
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
    /// let mut output_rx = ma.subscribe_to_event(move |event| {
    ///     if let ModularAgentEvent::ExternalOutput(name, value) = event {
    ///         if name == output_channel {
    ///             return Some(value);
    ///         }
    ///     }
    ///     None
    /// });
    ///
    /// // Now start the preset and receive events
    /// while let Some(value) = output_rx.recv().await {
    ///     println!("Received: {:?}", value);
    /// }
    /// # }
    /// ```
    pub fn subscribe_to_event<F, T>(&self, mut filter_map: F) -> mpsc::UnboundedReceiver<T>
    where
        F: FnMut(ModularAgentEvent) -> Option<T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut event_rx = self.subscribe();

        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        if let Some(mapped_event) = filter_map(event) {
                            if tx.send(mapped_event).is_err() {
                                // Receiver dropped, task can exit
                                break;
                            }
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

    pub(crate) fn emit_external_output(&self, name: String, value: AgentValue) {
        // // ignore local variables
        // if name.starts_with('%') {
        //     return;
        // }
        self.notify_observers(ModularAgentEvent::ExternalOutput(name, value));
    }

    fn notify_observers(&self, event: ModularAgentEvent) {
        let _ = self.observers.send(event);
    }
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
/// let mut rx = ma.subscribe_to_event(|event| {
///     if let ModularAgentEvent::ExternalOutput(name, value) = event {
///         Some((name, value))
///     } else {
///         None
///     }
/// });
/// # }
/// ```
#[derive(Clone, Debug)]
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

    /// A value was written to an external output channel.
    ///
    /// This event is emitted when:
    /// - [`ModularAgent::write_external_input`] is called and flows through the network
    /// - An [`ExternalOutputAgent`](crate::external_agent::ExternalOutputAgent) receives a value
    ///
    /// Fields: `(channel_name, value)`
    ExternalOutput(String, AgentValue),
}
