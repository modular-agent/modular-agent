use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::sync::{broadcast, broadcast::error::RecvError, Mutex as AsyncMutex, mpsc};

use crate::FnvIndexMap;
use crate::agent::{Agent, AgentMessage, AgentStatus, agent_new};
use crate::config::{AgentConfigs, AgentConfigsMap};
use crate::context::AgentContext;
use crate::definition::{AgentConfigSpecs, AgentDefinition, AgentDefinitions};
use crate::error::AgentError;
use crate::id::{new_id, update_ids};
use crate::message::{self, AgentEventMessage};
use crate::registry;
use crate::spec::{AgentSpec, AgentStreamSpec, ChannelSpec};
use crate::stream::{AgentStream, AgentStreamInfo, AgentStreams};
use crate::value::AgentValue;

const MESSAGE_LIMIT: usize = 1024;
const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct ASKit {
    // agent id -> agent
    pub(crate) agents: Arc<Mutex<FnvIndexMap<String, Arc<AsyncMutex<Box<dyn Agent>>>>>>,

    // agent id -> sender
    pub(crate) agent_txs: Arc<Mutex<FnvIndexMap<String, mpsc::Sender<AgentMessage>>>>,

    // board name -> [board out agent id]
    pub(crate) board_out_agents: Arc<Mutex<FnvIndexMap<String, Vec<String>>>>,

    // board name -> value
    pub(crate) board_value: Arc<Mutex<FnvIndexMap<String, AgentValue>>>,

    // source agent id -> [target agent id / source handle / target handle]
    pub(crate) channels: Arc<Mutex<FnvIndexMap<String, Vec<(String, String, String)>>>>,

    // agent def name -> agent definition
    pub(crate) defs: Arc<Mutex<AgentDefinitions>>,

    // agent streams (stream id -> stream)
    pub(crate) streams: Arc<Mutex<AgentStreams>>,

    // agent def name -> config
    pub(crate) global_configs_map: Arc<Mutex<FnvIndexMap<String, AgentConfigs>>>,

    // message sender
    pub(crate) tx: Arc<Mutex<Option<mpsc::Sender<AgentEventMessage>>>>,

    // observers
    pub(crate) observers: broadcast::Sender<ASKitEvent>,
}

impl ASKit {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            agents: Default::default(),
            agent_txs: Default::default(),
            board_out_agents: Default::default(),
            board_value: Default::default(),
            channels: Default::default(),
            defs: Default::default(),
            streams: Default::default(),
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

    /// Initialize ASKit.
    pub fn init() -> Result<Self, AgentError> {
        let askit = Self::new();
        askit.register_agents();
        Ok(askit)
    }

    fn register_agents(&self) {
        registry::register_inventory_agents(self);
    }

    /// Prepare ASKit to be ready.
    pub async fn ready(&self) -> Result<(), AgentError> {
        self.spawn_message_loop().await?;
        Ok(())
    }

    /// Quit ASKit.
    pub fn quit(&self) {
        let mut tx_lock = self.tx.lock().unwrap();
        *tx_lock = None;
    }

    /// Register an agent definition.
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

    /// Get all agent definitions.
    pub fn get_agent_definitions(&self) -> AgentDefinitions {
        let defs = self.defs.lock().unwrap();
        defs.clone()
    }

    /// Get an agent definition by name.
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

    // streams

    /// Get info of the agent stream by id.
    pub fn get_agent_stream_info(&self, id: &str) -> Option<AgentStreamInfo> {
        let streams = self.streams.lock().unwrap();
        streams.get(id).map(|stream| stream.into())
    }

    /// Get infos of all agent streams.
    pub fn get_agent_stream_infos(&self) -> Vec<AgentStreamInfo> {
        let streams = self.streams.lock().unwrap();
        streams.values().map(|s| s.into()).collect()
    }

    /// Get the agent stream spec by id.
    pub async fn get_agent_stream_spec(&self, id: &str) -> Option<AgentStreamSpec> {
        let stream_spec = {
            let streams = self.streams.lock().unwrap();
            streams.get(id).map(|stream| stream.spec().clone())
        };
        let Some(mut stream_spec) = stream_spec else {
            return None;
        };

        // collect agent specs in the stream
        let mut agent_specs = Vec::new();
        for agent in &stream_spec.agents {
            if let Some(spec) = self.get_agent_spec(&agent.id).await {
                agent_specs.push(spec);
            }
        }
        stream_spec.agents = agent_specs;

        // No need to change channels

        Some(stream_spec)
    }

    /// Update the agent stream spec
    pub fn update_agent_stream_spec(&self, id: &str, value: &Value) -> Result<(), AgentError> {
        let mut streams = self.streams.lock().unwrap();
        let Some(stream) = streams.get_mut(id) else {
            return Err(AgentError::StreamNotFound(id.to_string()));
        };
        stream.update_spec(value)?;
        Ok(())
    }

    /// Create a new agent stream with the given name.
    /// If the name already exists, a unique name will be generated by appending a number suffix.
    /// Returns the id of the new agent stream.
    pub fn new_agent_stream(&self, name: &str) -> Result<String, AgentError> {
        if !is_valid_stream_name(name) {
            return Err(AgentError::InvalidStreamName(name.into()));
        }
        let new_name = self.unique_stream_name(name);
        let spec = AgentStreamSpec::default();
        let id = self.add_agent_stream(new_name, spec)?;
        Ok(id)
    }

    /// Rename an existing agent stream.
    pub fn rename_agent_stream(&self, id: &str, new_name: &str) -> Result<String, AgentError> {
        if !is_valid_stream_name(new_name) {
            return Err(AgentError::InvalidStreamName(new_name.into()));
        }

        // check if the new name is already used
        let new_name = self.unique_stream_name(new_name);

        let mut streams = self.streams.lock().unwrap();

        // remove the original stream
        let Some(mut stream) = streams.swap_remove(id) else {
            return Err(AgentError::RenameStreamFailed(id.into()));
        };

        // insert renamed stream
        stream.set_name(new_name.clone());
        streams.insert(stream.id().to_string(), stream);
        Ok(new_name)
    }

    /// Generate a unique stream name by appending a number suffix if needed.
    pub fn unique_stream_name(&self, name: &str) -> String {
        let mut new_name = name.trim().to_string();
        let mut i = 2;
        let streams = self.streams.lock().unwrap();
        while streams.values().any(|stream| stream.name() == new_name) {
            new_name = format!("{}{}", name, i);
            i += 1;
        }
        new_name
    }

    /// Add a new agent stream with the given name and spec, and returns the id of the new agent stream.
    ///
    /// The ids of the given spec, including agents and channels, are changed to new unique ids.
    pub fn add_agent_stream(
        &self,
        name: String,
        spec: AgentStreamSpec,
    ) -> Result<String, AgentError> {
        let stream = AgentStream::new(name, spec);
        let id = stream.id().to_string();

        // add agents
        for agent in &stream.spec().agents {
            if let Err(e) = self.add_agent_internal(id.clone(), agent.clone()) {
                log::error!("Failed to add_agent {}: {}", agent.id, e);
            }
        }

        // add channels
        for channel in &stream.spec().channels {
            self.add_channel_internal(channel.clone())
                .unwrap_or_else(|e| {
                    log::error!("Failed to add_channel {}: {}", channel.source, e);
                });
        }

        // add the given stream into streams
        let mut streams = self.streams.lock().unwrap();
        if streams.contains_key(&id) {
            return Err(AgentError::DuplicateId(id.into()));
        }
        streams.insert(id.to_string(), stream);

        Ok(id)
    }

    /// Remove an agent stream by id.
    pub async fn remove_agent_stream(&self, id: &str) -> Result<(), AgentError> {
        let mut stream = {
            let mut streams = self.streams.lock().unwrap();
            let Some(stream) = streams.swap_remove(id) else {
                return Err(AgentError::StreamNotFound(id.to_string()));
            };
            stream
        };

        stream.stop(self).await.unwrap_or_else(|e| {
            log::error!("Failed to stop stream {}: {}", id, e);
        });

        // Remove all agents and channels associated with the stream
        for agent in &stream.spec().agents {
            self.remove_agent_internal(&agent.id)
                .await
                .unwrap_or_else(|e| {
                    log::error!("Failed to remove_agent {}: {}", agent.id, e);
                });
        }
        for channel in &stream.spec().channels {
            self.remove_channel_internal(channel);
        }

        Ok(())
    }

    /// Start an agent stream by id.
    pub async fn start_agent_stream(&self, id: &str) -> Result<(), AgentError> {
        let mut stream = {
            let mut streams = self.streams.lock().unwrap();
            let Some(stream) = streams.swap_remove(id) else {
                return Err(AgentError::StreamNotFound(id.to_string()));
            };
            stream
        };

        stream.start(self).await?;

        let mut streams = self.streams.lock().unwrap();
        streams.insert(id.to_string(), stream);
        Ok(())
    }

    /// Stop an agent stream by id.
    pub async fn stop_agent_stream(&self, id: &str) -> Result<(), AgentError> {
        let mut stream = {
            let mut streams = self.streams.lock().unwrap();
            let Some(stream) = streams.swap_remove(id) else {
                return Err(AgentError::StreamNotFound(id.to_string()));
            };
            stream
        };

        stream.stop(self).await?;

        let mut streams = self.streams.lock().unwrap();
        streams.insert(id.to_string(), stream);
        Ok(())
    }

    // Agents

    /// Create a new agent spec from the given agent definition name.
    pub fn new_agent_spec(&self, def_name: &str) -> Result<AgentSpec, AgentError> {
        let def = self
            .get_agent_definition(def_name)
            .ok_or_else(|| AgentError::AgentDefinitionNotFound(def_name.to_string()))?;
        Ok(def.to_spec())
    }

    /// Add an agent to the specified stream, and returns the id of the newly added agent.
    pub fn add_agent(&self, stream_id: String, mut spec: AgentSpec) -> Result<String, AgentError> {
        let mut streams = self.streams.lock().unwrap();
        let Some(stream) = streams.get_mut(&stream_id) else {
            return Err(AgentError::StreamNotFound(stream_id.to_string()));
        };
        let id = new_id();
        spec.id = id.clone();
        self.add_agent_internal(stream_id, spec.clone())?;
        stream.add_agent(spec.clone());
        Ok(id)
    }

    fn add_agent_internal(&self, stream_id: String, spec: AgentSpec) -> Result<(), AgentError> {
        let mut agents = self.agents.lock().unwrap();
        if agents.contains_key(&spec.id) {
            return Err(AgentError::AgentAlreadyExists(spec.id.to_string()));
        }
        let spec_id = spec.id.clone();
        let mut agent = agent_new(self.clone(), spec_id.clone(), spec)?;
        agent.set_stream_id(stream_id);
        agents.insert(spec_id, Arc::new(AsyncMutex::new(agent)));
        Ok(())
    }

    /// Get the agent by id.
    pub fn get_agent(&self, agent_id: &str) -> Option<Arc<AsyncMutex<Box<dyn Agent>>>> {
        let agents = self.agents.lock().unwrap();
        agents.get(agent_id).cloned()
    }

    /// Add a channel to the specified stream.
    pub fn add_channel(&self, stream_id: &str, channel: ChannelSpec) -> Result<(), AgentError> {
        // check if the source and target agents exist
        {
            let agents = self.agents.lock().unwrap();
            if !agents.contains_key(&channel.source) {
                return Err(AgentError::AgentNotFound(channel.source.to_string()));
            }
            if !agents.contains_key(&channel.target) {
                return Err(AgentError::AgentNotFound(channel.target.to_string()));
            }
        }

        // check if handles are valid
        if channel.source_handle.is_empty() {
            return Err(AgentError::EmptySourceHandle);
        }
        if channel.target_handle.is_empty() {
            return Err(AgentError::EmptyTargetHandle);
        }

        let mut streams = self.streams.lock().unwrap();
        let Some(stream) = streams.get_mut(stream_id) else {
            return Err(AgentError::StreamNotFound(stream_id.to_string()));
        };
        stream.add_channel(channel.clone());
        self.add_channel_internal(channel)?;
        Ok(())
    }

    fn add_channel_internal(&self, channel: ChannelSpec) -> Result<(), AgentError> {
        let mut channels = self.channels.lock().unwrap();
        if let Some(targets) = channels.get_mut(&channel.source) {
            if targets
                .iter()
                .any(|(target, source_handle, target_handle)| {
                    *target == channel.target
                        && *source_handle == channel.source_handle
                        && *target_handle == channel.target_handle
                })
            {
                return Err(AgentError::ChannelAlreadyExists);
            }
            targets.push((channel.target, channel.source_handle, channel.target_handle));
        } else {
            channels.insert(
                channel.source,
                vec![(channel.target, channel.source_handle, channel.target_handle)],
            );
        }
        Ok(())
    }

    /// Add agents and channels to the specified stream.
    ///
    /// The ids of the given agents and channels are changed to new unique ids.
    /// The agents are not started automatically, even if the stream is running.
    pub fn add_agents_and_channels(
        &self,
        stream_id: &str,
        agents: &Vec<AgentSpec>,
        channels: &Vec<ChannelSpec>,
    ) -> Result<(Vec<AgentSpec>, Vec<ChannelSpec>), AgentError> {
        let (agents, channels) = update_ids(agents, channels);

        let mut streams = self.streams.lock().unwrap();
        let Some(stream) = streams.get_mut(stream_id) else {
            return Err(AgentError::StreamNotFound(stream_id.to_string()));
        };

        for agent in &agents {
            self.add_agent_internal(stream_id.to_string(), agent.clone())?;
            stream.add_agent(agent.clone());
        }

        for channel in &channels {
            self.add_channel_internal(channel.clone())?;
            stream.add_channel(channel.clone());
        }

        Ok((agents, channels))
    }

    /// Remove an agent from the specified stream.
    ///
    /// If the agent is running, it will be stopped first.
    pub async fn remove_agent(&self, stream_id: &str, agent_id: &str) -> Result<(), AgentError> {
        {
            let mut streams = self.streams.lock().unwrap();
            let Some(stream) = streams.get_mut(stream_id) else {
                return Err(AgentError::StreamNotFound(stream_id.to_string()));
            };
            stream.remove_agent(agent_id);
        }
        if let Err(e) = self.remove_agent_internal(agent_id).await {
            return Err(e);
        }
        Ok(())
    }

    async fn remove_agent_internal(&self, agent_id: &str) -> Result<(), AgentError> {
        self.stop_agent(agent_id).await?;

        // remove from channels
        {
            let mut channels = self.channels.lock().unwrap();
            let mut sources_to_remove = Vec::new();
            for (source, targets) in channels.iter_mut() {
                targets.retain(|(target, _, _)| target != agent_id);
                if targets.is_empty() {
                    sources_to_remove.push(source.clone());
                }
            }
            for source in sources_to_remove {
                channels.swap_remove(&source);
            }
            channels.swap_remove(agent_id);
        }

        // remove from agents
        {
            let mut agents = self.agents.lock().unwrap();
            agents.swap_remove(agent_id);
        }

        Ok(())
    }

    /// Remove a channel from the specified stream.
    pub fn remove_channel(&self, stream_id: &str, channel: &ChannelSpec) -> Result<(), AgentError> {
        let mut stream = {
            let mut streams = self.streams.lock().unwrap();
            let Some(stream) = streams.swap_remove(stream_id) else {
                return Err(AgentError::StreamNotFound(stream_id.to_string()));
            };
            stream
        };

        let Some(channel) = stream.remove_channel(channel) else {
            let mut streams = self.streams.lock().unwrap();
            streams.insert(stream_id.to_string(), stream);
            return Err(AgentError::ChannelNotFound(format!(
                "{}:{}->{}:{}",
                channel.source, channel.source_handle, channel.target, channel.target_handle
            )));
        };
        let mut streams = self.streams.lock().unwrap();
        streams.insert(stream_id.to_string(), stream);

        self.remove_channel_internal(&channel);
        Ok(())
    }

    fn remove_channel_internal(&self, channel: &ChannelSpec) {
        let mut channels = self.channels.lock().unwrap();
        if let Some(targets) = channels.get_mut(&channel.source) {
            targets.retain(|(target, source_handle, target_handle)| {
                *target != channel.target
                    || *source_handle != channel.source_handle
                    || *target_handle != channel.target_handle
            });
            if targets.is_empty() {
                channels.swap_remove(&channel.source);
            }
        }
    }

    /// Start an agent by id.
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
                        AgentMessage::Input { ctx, pin, value } => {
                            agent_clone
                                .lock()
                                .await
                                .process(ctx, pin, value)
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
                            agent_clone.lock().await.set_configs(configs).unwrap_or_else(|e| {
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
    pub async fn stop_agent(&self, agent_id: &str) -> Result<(), AgentError> {
        {
            // remove the sender first to prevent new messages being sent
            let mut agent_txs = self.agent_txs.lock().unwrap();
                    if let Some(tx) = agent_txs.swap_remove(agent_id) {
                        if let Err(e) = tx.try_send(AgentMessage::Stop) {
                            log::warn!("Failed to send stop message to agent {}: {}", agent_id, e);
                        }
                    }        }

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
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let message = if pin.starts_with("config:") {
            let config_key = pin[7..].to_string();
            AgentMessage::Config {
                key: config_key,
                value,
            }
        } else {
            AgentMessage::Input {
                ctx,
                pin: pin.clone(),
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

        self.emit_agent_input(agent_id.to_string(), pin);

        Ok(())
    }

    /// Send output from an agent. (Async version)
    pub async fn send_agent_out(
        &self,
        agent_id: String,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        message::send_agent_out(self, agent_id, ctx, pin, value).await
    }

    /// Send output from an agent.
    pub fn try_send_agent_out(
        &self,
        agent_id: String,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        message::try_send_agent_out(self, agent_id, ctx, pin, value)
    }

    /// Write a value to the board.
    pub async fn write_board_value(
        &self,
        name: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.send_board_out(name, AgentContext::new(), value).await
    }

    /// Write a value to the variable board.
    pub async fn write_var_value(
        &self,
        stream_id: &str,
        name: &str,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let var_name = format!("%{}/{}", stream_id, name);
        self.send_board_out(var_name, AgentContext::new(), value)
            .await
    }

    pub(crate) async fn send_board_out(
        &self,
        name: String,
        ctx: AgentContext,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        message::send_board_out(self, name, ctx, value).await
    }

    async fn spawn_message_loop(&self) -> Result<(), AgentError> {
        // TODO: settings for the channel size
        let (tx, mut rx) = mpsc::channel(4096);
        {
            let mut tx_lock = self.tx.lock().unwrap();
            *tx_lock = Some(tx);
        }

        // spawn the main loop
        let askit = self.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                use AgentEventMessage::*;

                match message {
                    AgentOut {
                        agent,
                        ctx,
                        pin,
                        value,
                    } => {
                        message::agent_out(&askit, agent, ctx, pin, value).await;
                    }
                    BoardOut { name, ctx, value } => {
                        message::board_out(&askit, name, ctx, value).await;
                    }
                }
            }
        });

        tokio::task::yield_now().await;

        Ok(())
    }

    /// Subscribe to all ASKit events.
    pub fn subscribe(&self) -> broadcast::Receiver<ASKitEvent> {
        self.observers.subscribe()
    }

    /// Subscribe to a specific type of `ASKitEvent`.
    ///
    /// It takes a closure that filters and maps the events, and returns an `mpsc::UnboundedReceiver`
    /// that will receive only the successfully mapped events.
    pub fn subscribe_to_event<F, T>(&self, mut filter_map: F) -> mpsc::UnboundedReceiver<T>
    where
        F: FnMut(ASKitEvent) -> Option<T> + Send + 'static,
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
        self.notify_observers(ASKitEvent::AgentConfigUpdated(agent_id, key, value));
    }

    pub(crate) fn emit_agent_error(&self, agent_id: String, message: String) {
        self.notify_observers(ASKitEvent::AgentError(agent_id, message));
    }

    pub(crate) fn emit_agent_input(&self, agent_id: String, pin: String) {
        self.notify_observers(ASKitEvent::AgentIn(agent_id, pin));
    }

    pub(crate) fn emit_agent_spec_updated(&self, agent_id: String) {
        self.notify_observers(ASKitEvent::AgentSpecUpdated(agent_id));
    }

    pub(crate) fn emit_board(&self, name: String, value: AgentValue) {
        // // ignore variables
        // if name.starts_with('%') {
        //     return;
        // }
        self.notify_observers(ASKitEvent::Board(name, value));
    }

    fn notify_observers(&self, event: ASKitEvent) {
        let _ = self.observers.send(event);
    }
}

fn is_valid_stream_name(new_name: &str) -> bool {
    // Check if the name is empty
    if new_name.trim().is_empty() {
        return false;
    }

    // Checks for path-like names:
    if new_name.contains('/') {
        // Disallow leading, trailing, or consecutive slashes
        if new_name.starts_with('/') || new_name.ends_with('/') || new_name.contains("//") {
            return false;
        }
        // Disallow segments that are "." or ".."
        if new_name
            .split('/')
            .any(|segment| segment == "." || segment == "..")
        {
            return false;
        }
    }

    // Check if the name contains invalid characters
    let invalid_chars = ['\\', ':', '*', '?', '"', '<', '>', '|'];
    for c in invalid_chars {
        if new_name.contains(c) {
            return false;
        }
    }

    true
}

#[derive(Clone, Debug)]
pub enum ASKitEvent {
    AgentConfigUpdated(String, String, AgentValue), // (agent_id, key, value)
    AgentError(String, String),                     // (agent_id, message)
    AgentIn(String, String),                        // (agent_id, pin)
    AgentSpecUpdated(String),                       // (agent_id)
    Board(String, AgentValue),                      // (board name, value)
}
