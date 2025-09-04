use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::board_agent;

use super::agent::{AgentMessage, AgentStatus, AsyncAgent, agent_new};
use super::config::AgentConfig;
use super::context::AgentContext;
use super::data::AgentData;
use super::definition::{AgentDefaultConfig, AgentDefinition, AgentDefinitions};
use super::error::AgentError;
use super::flow::{self, AgentFlow, AgentFlowEdge, AgentFlowNode, AgentFlows};
use super::message::{self, AgentEventMessage};

#[derive(Clone)]
pub struct ASKit {
    // agent id -> agent
    pub(crate) agents: Arc<Mutex<HashMap<String, Arc<Mutex<Box<dyn AsyncAgent>>>>>>,

    // agent id -> sender
    pub(crate) agent_txs: Arc<Mutex<HashMap<String, AgentMessageSender>>>,

    // board name -> [board out agent id]
    pub(crate) board_out_agents: Arc<Mutex<HashMap<String, Vec<String>>>>,

    // board name -> data
    pub(crate) board_data: Arc<Mutex<HashMap<String, AgentData>>>,

    // sourece agent id -> [target agent id / source handle / target handle]
    pub(crate) edges: Arc<Mutex<HashMap<String, Vec<(String, String, String)>>>>,

    // agent def name -> agent definition
    pub(crate) defs: Arc<Mutex<AgentDefinitions>>,

    // agent flows
    pub(crate) flows: Arc<Mutex<AgentFlows>>,

    // message sender
    pub(crate) tx: Arc<Mutex<Option<mpsc::Sender<AgentEventMessage>>>>,

    // observers
    pub(crate) observers: Arc<Mutex<HashMap<usize, Box<dyn ASKitObserver + Sync + Send>>>>,
}

impl ASKit {
    pub fn new() -> Self {
        Self {
            agents: Default::default(),

            agent_txs: Default::default(),

            board_out_agents: Default::default(),

            board_data: Default::default(),

            edges: Default::default(),

            defs: Default::default(),

            flows: Default::default(),

            tx: Arc::new(Mutex::new(None)),

            observers: Default::default(),
        }
    }

    pub(crate) fn tx(&self) -> Result<mpsc::Sender<AgentEventMessage>, AgentError> {
        self.tx
            .lock()
            .unwrap()
            .clone()
            .ok_or(AgentError::TxNotInitialized)
    }

    pub fn init() -> Result<Self, AgentError> {
        let askit = Self::new();
        askit.register_agents();
        Ok(askit)
    }

    fn register_agents(&self) {
        board_agent::register_agents(self);
    }

    pub fn ready(&self) -> Result<(), AgentError> {
        self.spawn_message_loop()?;
        self.start_agent_flows()?;
        Ok(())
    }

    pub fn quit(&self) {
        let mut tx_lock = self.tx.lock().unwrap();
        *tx_lock = None;
    }

    pub fn register_agent(&self, def: AgentDefinition) {
        let mut defs = self.defs.lock().unwrap();
        defs.insert(def.name.clone(), def);
    }

    pub fn get_agent_definitions(&self) -> AgentDefinitions {
        let defs = self.defs.lock().unwrap();
        defs.clone()
    }

    pub fn get_agent_definition(&self, def_name: &str) -> Option<AgentDefinition> {
        let defs = self.defs.lock().unwrap();
        defs.get(def_name).cloned()
    }

    pub fn get_agent_default_config(&self, def_name: &str) -> Option<AgentDefaultConfig> {
        let defs = self.defs.lock().unwrap();
        let Some(def) = defs.get(def_name) else {
            return None;
        };
        def.default_config.clone()
    }

    // // flow

    pub fn get_agent_flows(&self) -> AgentFlows {
        let flows = self.flows.lock().unwrap();
        flows.clone()
    }

    pub fn new_agent_flow(&self, name: &str) -> Result<AgentFlow, AgentError> {
        if !Self::is_valid_flow_name(name) {
            return Err(AgentError::InvalidFlowName(name.into()));
        }

        let new_name = self.unique_flow_name(name);
        let mut flows = self.flows.lock().unwrap();
        let flow = AgentFlow::new(new_name.clone());
        flows.insert(new_name, flow.clone());
        Ok(flow)
    }

    pub fn rename_agent_flow(&self, old_name: &str, new_name: &str) -> Result<String, AgentError> {
        if !Self::is_valid_flow_name(new_name) {
            return Err(AgentError::InvalidFlowName(new_name.into()));
        }

        // check if the new name is already used
        let new_name = self.unique_flow_name(new_name);

        let mut flows = self.flows.lock().unwrap();

        // remove the original flow
        let Some(mut flow) = flows.remove(old_name) else {
            return Err(AgentError::RenameFlowFailed(old_name.into()));
        };

        // insert renamed flow
        flow.set_name(new_name.clone());
        flows.insert(new_name.clone(), flow);
        Ok(new_name)
    }

    fn is_valid_flow_name(new_name: &str) -> bool {
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

    pub fn unique_flow_name(&self, name: &str) -> String {
        let mut new_name = name.trim().to_string();
        let mut i = 2;
        let flows = self.flows.lock().unwrap();
        while flows.contains_key(&new_name) {
            new_name = format!("{}{}", name, i);
            i += 1;
        }
        new_name
    }

    pub fn add_agent_flow(&self, agent_flow: &AgentFlow) -> Result<(), AgentError> {
        let name = agent_flow.name();

        // add the given flow into flows
        {
            let mut flows = self.flows.lock().unwrap();
            if flows.contains_key(name) {
                return Err(AgentError::DuplicateFlowName(name.into()));
            }
            flows.insert(name.into(), agent_flow.clone());
        }

        // add nodes into agents
        for node in agent_flow.nodes().iter() {
            self.add_agent(node).unwrap_or_else(|e| {
                log::error!("Failed to add_agent_node {}: {}", node.id, e);
            });
        }

        // add edges into edges
        for edge in agent_flow.edges().iter() {
            self.add_edge(edge).unwrap_or_else(|e| {
                log::error!("Failed to add_edge {}: {}", edge.source, e);
            });
        }

        Ok(())
    }

    pub fn remove_agent_flow(&self, flow_name: &str) -> Result<(), AgentError> {
        let mut flows = self.flows.lock().unwrap();
        let Some(flow) = flows.remove(flow_name) else {
            return Err(AgentError::FlowNotFound(flow_name.to_string()));
        };

        flow.stop(self)?;

        // Remove all nodes and edges associated with the flow
        for node in flow.nodes() {
            self.remove_agent(&node.id)?;
        }
        for edge in flow.edges() {
            self.remove_edge(edge);
        }

        Ok(())
    }

    pub fn insert_agent_flow(&self, flow: AgentFlow) -> Result<(), AgentError> {
        let flow_name = flow.name();

        let mut flows = self.flows.lock().unwrap();
        flows.insert(flow_name.to_string(), flow);
        Ok(())
    }

    pub fn add_agent_flow_node(
        &self,
        flow_name: &str,
        node: &AgentFlowNode,
    ) -> Result<(), AgentError> {
        let mut flows = self.flows.lock().unwrap();
        let Some(flow) = flows.get_mut(flow_name) else {
            return Err(AgentError::FlowNotFound(flow_name.to_string()));
        };
        flow.add_node(node.clone());
        self.add_agent(node)?;
        Ok(())
    }

    pub(crate) fn add_agent(&self, node: &AgentFlowNode) -> Result<(), AgentError> {
        let mut agents = self.agents.lock().unwrap();
        if agents.contains_key(&node.id) {
            return Err(AgentError::AgentAlreadyExists(node.id.to_string()));
        }
        if let Ok(agent) = agent_new(
            self.clone(),
            node.id.clone(),
            &node.def_name,
            node.config.clone(),
        ) {
            agents.insert(node.id.clone(), Arc::new(Mutex::new(agent)));
            log::info!("Agent {} created", node.id);
        } else {
            return Err(AgentError::AgentCreationFailed(node.id.to_string()));
        }
        Ok(())
    }

    pub fn add_agent_flow_edge(
        &self,
        flow_name: &str,
        edge: &AgentFlowEdge,
    ) -> Result<(), AgentError> {
        let mut flows = self.flows.lock().unwrap();
        let Some(flow) = flows.get_mut(flow_name) else {
            return Err(AgentError::FlowNotFound(flow_name.to_string()));
        };
        flow.add_edge(edge.clone());
        self.add_edge(edge)?;
        Ok(())
    }

    pub(crate) fn add_edge(&self, edge: &AgentFlowEdge) -> Result<(), AgentError> {
        // check if the source agent exists
        {
            let agents = self.agents.lock().unwrap();
            if !agents.contains_key(&edge.source) {
                return Err(AgentError::SourceAgentNotFound(edge.source.to_string()));
            }
        }

        // check if handles are valid
        if edge.source_handle.is_empty() {
            return Err(AgentError::EmptySourceHandle);
        }
        if edge.target_handle.is_empty() {
            return Err(AgentError::EmptyTargetHandle);
        }

        let mut edges = self.edges.lock().unwrap();
        if let Some(targets) = edges.get_mut(&edge.source) {
            if targets
                .iter()
                .any(|(target, source_handle, target_handle)| {
                    *target == edge.target
                        && *source_handle == edge.source_handle
                        && *target_handle == edge.target_handle
                })
            {
                return Err(AgentError::EdgeAlreadyExists);
            }
            targets.push((
                edge.target.clone(),
                edge.source_handle.clone(),
                edge.target_handle.clone(),
            ));
        } else {
            edges.insert(
                edge.source.clone(),
                vec![(
                    edge.target.clone(),
                    edge.source_handle.clone(),
                    edge.target_handle.clone(),
                )],
            );
        }
        Ok(())
    }

    pub fn remove_agent_flow_node(&self, flow_name: &str, node_id: &str) -> Result<(), AgentError> {
        let mut flows = self.flows.lock().unwrap();
        let Some(flow) = flows.get_mut(flow_name) else {
            return Err(AgentError::FlowNotFound(flow_name.to_string()));
        };
        flow.remove_node(node_id);
        self.remove_agent(node_id)?;
        Ok(())
    }

    pub(crate) fn remove_agent(&self, agent_id: &str) -> Result<(), AgentError> {
        self.stop_agent(agent_id)?;

        // remove from edges
        {
            let mut edges = self.edges.lock().unwrap();
            let mut sources_to_remove = Vec::new();
            for (source, targets) in edges.iter_mut() {
                targets.retain(|(target, _, _)| target != agent_id);
                if targets.is_empty() {
                    sources_to_remove.push(source.clone());
                }
            }
            for source in sources_to_remove {
                edges.remove(&source);
            }
            edges.remove(agent_id);
        }

        // remove from agents
        {
            let mut agents = self.agents.lock().unwrap();
            agents.remove(agent_id);
        }

        Ok(())
    }

    pub fn remove_agent_flow_edge(&self, flow_name: &str, edge_id: &str) -> Result<(), AgentError> {
        let mut flows = self.flows.lock().unwrap();
        let Some(flow) = flows.get_mut(flow_name) else {
            return Err(AgentError::FlowNotFound(flow_name.to_string()));
        };
        let Some(edge) = flow.remove_edge(edge_id) else {
            return Err(AgentError::EdgeNotFound(edge_id.to_string()));
        };
        self.remove_edge(&edge);
        Ok(())
    }

    pub(crate) fn remove_edge(&self, edge: &AgentFlowEdge) {
        let mut edges = self.edges.lock().unwrap();
        if let Some(targets) = edges.get_mut(&edge.source) {
            targets.retain(|(target, source_handle, target_handle)| {
                *target != edge.target
                    || *source_handle != edge.source_handle
                    || *target_handle != edge.target_handle
            });
            if targets.is_empty() {
                edges.remove(&edge.source);
            }
        }
    }

    pub fn copy_sub_flow(
        &self,
        nodes: &Vec<AgentFlowNode>,
        edges: &Vec<AgentFlowEdge>,
    ) -> (Vec<AgentFlowNode>, Vec<AgentFlowEdge>) {
        flow::copy_sub_flow(nodes, edges)
    }

    pub fn start_agent_flow(&self, name: &str) -> Result<(), AgentError> {
        let flows = self.flows.lock().unwrap();
        let Some(flow) = flows.get(name) else {
            return Err(AgentError::FlowNotFound(name.to_string()));
        };
        flow.start(self)?;
        Ok(())
    }

    pub fn start_agent(&self, agent_id: &str) -> Result<(), AgentError> {
        let agent = {
            let agents = self.agents.lock().unwrap();
            let Some(a) = agents.get(agent_id) else {
                return Err(AgentError::AgentNotFound(agent_id.to_string()));
            };
            a.clone()
        };
        let def_name = {
            let agent = agent.lock().unwrap();
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
            let agent = agent.lock().unwrap();
            agent.status().clone()
        };
        if agent_status == AgentStatus::Init {
            log::info!("Starting agent {}", agent_id);

            if uses_native_thread {
                let (tx, rx) = std::sync::mpsc::channel();

                {
                    let mut agent_txs = self.agent_txs.lock().unwrap();
                    agent_txs.insert(agent_id.to_string(), AgentMessageSender::Sync(tx.clone()));
                };

                let agent_id = agent_id.to_string();
                std::thread::spawn(move || {
                    if let Err(e) = agent.lock().unwrap().start() {
                        log::error!("Failed to start agent {}: {}", agent_id, e);
                    }

                    while let Ok(message) = rx.recv() {
                        match message {
                            AgentMessage::Input { ctx, data } => {
                                agent
                                    .lock()
                                    .unwrap()
                                    .process(ctx, data)
                                    .unwrap_or_else(|e| {
                                        log::error!("Process Error {}: {}", agent_id, e);
                                    });
                            }
                            AgentMessage::Config { config } => {
                                agent
                                    .lock()
                                    .unwrap()
                                    .set_config(config)
                                    .unwrap_or_else(|e| {
                                        log::error!("Config Error {}: {}", agent_id, e);
                                    });
                            }
                            AgentMessage::Stop => {
                                break;
                            }
                        }
                    }
                });
            } else {
                let (tx, mut rx) = mpsc::channel(32);

                {
                    let mut agent_txs = self.agent_txs.lock().unwrap();
                    agent_txs.insert(agent_id.to_string(), AgentMessageSender::Async(tx.clone()));
                };

                let agent_id = agent_id.to_string();
                tokio::spawn(async move {
                    if let Err(e) = agent.lock().unwrap().start() {
                        log::error!("Failed to start agent {}: {}", agent_id, e);
                    }

                    while let Some(message) = rx.recv().await {
                        match message {
                            AgentMessage::Input { ctx, data } => {
                                agent
                                    .lock()
                                    .unwrap()
                                    .process(ctx, data)
                                    .unwrap_or_else(|e| {
                                        log::error!("Process Error {}: {}", agent_id, e);
                                    });
                            }
                            AgentMessage::Config { config } => {
                                agent
                                    .lock()
                                    .unwrap()
                                    .set_config(config)
                                    .unwrap_or_else(|e| {
                                        log::error!("Config Error {}: {}", agent_id, e);
                                    });
                            }
                            AgentMessage::Stop => {
                                rx.close();
                                return;
                            }
                        }
                    }
                });
            }
        }
        Ok(())
    }

    pub fn stop_agent(&self, agent_id: &str) -> Result<(), AgentError> {
        let agent = {
            let agents = self.agents.lock().unwrap();
            let Some(a) = agents.get(agent_id) else {
                return Err(AgentError::AgentNotFound(agent_id.to_string()));
            };
            a.clone()
        };

        let agent_status = {
            let agent = agent.lock().unwrap();
            agent.status().clone()
        };
        if agent_status == AgentStatus::Start {
            log::info!("Stopping agent {}", agent_id);

            {
                let mut agent_txs = self.agent_txs.lock().unwrap();
                if let Some(tx) = agent_txs.remove(agent_id) {
                    match tx {
                        AgentMessageSender::Sync(tx) => {
                            tx.send(AgentMessage::Stop).unwrap_or_else(|e| {
                                log::error!(
                                    "Failed to send stop message to agent {}: {}",
                                    agent_id,
                                    e
                                );
                            });
                        }
                        AgentMessageSender::Async(tx) => {
                            tx.try_send(AgentMessage::Stop).unwrap_or_else(|e| {
                                log::error!(
                                    "Failed to send stop message to agent {}: {}",
                                    agent_id,
                                    e
                                );
                            });
                        }
                    }
                }
            }

            agent.lock().unwrap().stop()?;
        }

        Ok(())
    }

    pub async fn set_agent_config(
        &self,
        agent_id: String,
        config: AgentConfig,
    ) -> Result<(), AgentError> {
        let agent = {
            let agents = self.agents.lock().unwrap();
            let Some(a) = agents.get(&agent_id) else {
                return Err(AgentError::AgentNotFound(agent_id.to_string()));
            };
            a.clone()
        };

        let agent_status = {
            let agent = agent.lock().unwrap();
            agent.status().clone()
        };
        if agent_status == AgentStatus::Init {
            agent.lock().unwrap().set_config(config.clone())?;
        } else if agent_status == AgentStatus::Start {
            let tx = {
                let agent_txs = self.agent_txs.lock().unwrap();
                let Some(tx) = agent_txs.get(&agent_id) else {
                    return Err(AgentError::AgentTxNotFound(agent_id.to_string()));
                };
                tx.clone()
            };
            let message = AgentMessage::Config { config };
            match tx {
                AgentMessageSender::Sync(tx) => {
                    tx.send(message).map_err(|_| {
                        AgentError::SendMessageFailed("Failed to send config message".to_string())
                    })?;
                }
                AgentMessageSender::Async(tx) => {
                    tx.send(message).await.map_err(|_| {
                        AgentError::SendMessageFailed("Failed to send config message".to_string())
                    })?;
                }
            }
        }
        Ok(())
    }

    pub async fn agent_input(
        &self,
        agent_id: String,
        ctx: AgentContext,
        data: AgentData,
    ) -> Result<(), AgentError> {
        let agent: Arc<Mutex<Box<dyn AsyncAgent>>> = {
            let agents = self.agents.lock().unwrap();
            let Some(a) = agents.get(&agent_id) else {
                return Err(AgentError::AgentNotFound(agent_id.to_string()));
            };
            a.clone()
        };

        let agent_status = {
            let agent = agent.lock().unwrap();
            agent.status().clone()
        };
        if agent_status == AgentStatus::Start {
            let ch = ctx.ch().to_string();
            let message = AgentMessage::Input { ctx, data };

            let tx = {
                let agent_txs = self.agent_txs.lock().unwrap();
                let Some(tx) = agent_txs.get(&agent_id) else {
                    return Err(AgentError::AgentTxNotFound(agent_id.to_string()));
                };
                tx.clone()
            };
            match tx {
                AgentMessageSender::Sync(tx) => {
                    tx.send(message).map_err(|_| {
                        AgentError::SendMessageFailed("Failed to send input message".to_string())
                    })?;
                }
                AgentMessageSender::Async(tx) => {
                    tx.send(message).await.map_err(|_| {
                        AgentError::SendMessageFailed("Failed to send input message".to_string())
                    })?;
                }
            }

            self.emit_input(agent_id.to_string(), ch);
        }
        Ok(())
    }

    pub async fn send_agent_out(
        &self,
        agent_id: String,
        ctx: AgentContext,
        data: AgentData,
    ) -> Result<(), AgentError> {
        message::send_agent_out(self, agent_id, ctx, data).await
    }

    pub fn try_send_agent_out(
        &self,
        agent_id: String,
        ctx: AgentContext,
        data: AgentData,
    ) -> Result<(), AgentError> {
        message::try_send_agent_out(self, agent_id, ctx, data)
    }

    pub fn try_send_board_out(
        &self,
        name: String,
        ctx: AgentContext,
        data: AgentData,
    ) -> Result<(), AgentError> {
        message::try_send_board_out(self, name, ctx, data)
    }

    fn spawn_message_loop(&self) -> Result<(), AgentError> {
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
                    AgentOut { agent, ctx, data } => {
                        message::agent_out(&askit, agent, ctx, data).await;
                    }
                    BoardOut { name, ctx, data } => {
                        message::board_out(&askit, name, ctx, data).await;
                    }
                }
            }
        });

        Ok(())
    }

    fn start_agent_flows(&self) -> Result<(), AgentError> {
        let agent_flow_names;
        {
            let agent_flows = self.flows.lock().unwrap();
            agent_flow_names = agent_flows.keys().cloned().collect::<Vec<_>>();
        }
        for name in agent_flow_names {
            self.start_agent_flow(&name).unwrap_or_else(|e| {
                log::error!("Failed to start agent flow: {}", e);
            });
        }
        Ok(())
    }

    pub fn subscribe(&self, observer: Box<dyn ASKitObserver + Sync + Send>) -> usize {
        let mut observers = self.observers.lock().unwrap();
        let observer_id = new_observer_id();
        observers.insert(observer_id, observer);
        observer_id
    }

    pub fn unsubscribe(&self, observer_id: usize) {
        let mut observers = self.observers.lock().unwrap();
        observers.remove(&observer_id);
    }

    pub(crate) fn emit_error(&self, agent_id: String, message: String) {
        self.notify_observers(ASKitEvent::AgentError(agent_id.clone(), message.clone()));
    }

    pub(crate) fn emit_input(&self, agent_id: String, ch: String) {
        self.notify_observers(ASKitEvent::AgentIn(agent_id.clone(), ch.clone()));
    }

    pub(crate) fn emit_display(&self, agent_id: String, key: String, data: AgentData) {
        self.notify_observers(ASKitEvent::AgentDisplay(
            agent_id.clone(),
            key.clone(),
            data.clone(),
        ));
    }

    fn notify_observers(&self, event: ASKitEvent) {
        let observers = self.observers.lock().unwrap();
        for (_id, observer) in observers.iter() {
            observer.notify(event.clone());
        }
    }
}

#[derive(Clone, Debug)]
pub enum ASKitEvent {
    AgentIn(String, String),                 // (agent_id, channel)
    AgentDisplay(String, String, AgentData), // (agent_id, key, data)
    AgentError(String, String),              // (agent_id, message)
}

pub trait ASKitObserver {
    fn notify(&self, event: ASKitEvent);
}

static OBSERVER_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn new_observer_id() -> usize {
    OBSERVER_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// Agent Message

#[derive(Clone)]
pub enum AgentMessageSender {
    Sync(std::sync::mpsc::Sender<AgentMessage>),
    Async(mpsc::Sender<AgentMessage>),
}
