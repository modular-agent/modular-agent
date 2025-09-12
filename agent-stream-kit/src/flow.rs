use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::askit::ASKit;
use super::config::AgentConfig;
use super::definition::AgentDefinition;
use super::error::AgentError;

pub type AgentFlows = HashMap<String, AgentFlow>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentFlow {
    name: String,

    nodes: Vec<AgentFlowNode>,

    edges: Vec<AgentFlowEdge>,

    #[serde(flatten)]
    pub extensions: HashMap<String, Value>,
}

impl AgentFlow {
    pub fn new(name: String) -> Self {
        Self {
            name,
            nodes: Vec::new(),
            edges: Vec::new(),
            extensions: HashMap::new(),
        }
    }

    pub fn nodes(&self) -> &Vec<AgentFlowNode> {
        &self.nodes
    }

    pub fn edges(&self) -> &Vec<AgentFlowEdge> {
        &self.edges
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, new_name: String) {
        self.name = new_name;
    }

    pub fn add_node(&mut self, node: AgentFlowNode) {
        self.nodes.push(node);
    }

    pub fn remove_node(&mut self, node_id: &str) {
        self.nodes.retain(|node| node.id != node_id);
    }

    pub fn set_nodes(&mut self, nodes: Vec<AgentFlowNode>) {
        self.nodes = nodes;
    }

    pub fn add_edge(&mut self, edge: AgentFlowEdge) {
        self.edges.push(edge);
    }

    pub fn remove_edge(&mut self, edge_id: &str) -> Option<AgentFlowEdge> {
        if let Some(edge) = self.edges.iter().find(|edge| edge.id == edge_id).cloned() {
            self.edges.retain(|e| e.id != edge_id);
            Some(edge)
        } else {
            None
        }
    }

    pub fn set_edges(&mut self, edges: Vec<AgentFlowEdge>) {
        self.edges = edges;
    }

    pub async fn start(&self, askit: &ASKit) -> Result<(), AgentError> {
        for agent in self.nodes.iter() {
            if !agent.enabled {
                continue;
            }
            askit.start_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to start agent {}: {}", agent.id, e);
            });
        }
        Ok(())
    }

    pub async fn stop(&self, askit: &ASKit) -> Result<(), AgentError> {
        for agent in self.nodes.iter() {
            if !agent.enabled {
                continue;
            }
            askit.stop_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to stop agent {}: {}", agent.id, e);
            });
        }
        Ok(())
    }

    pub fn disable_all_nodes(&mut self) {
        for node in self.nodes.iter_mut() {
            node.enabled = false;
        }
    }

    pub fn to_json(&self) -> Result<String, AgentError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        Ok(json)
    }

    pub fn from_json(json_str: &str) -> Result<Self, AgentError> {
        let flow: AgentFlow = serde_json::from_str(json_str)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        Ok(flow)
    }
}

pub fn copy_sub_flow(
    nodes: &Vec<AgentFlowNode>,
    edges: &Vec<AgentFlowEdge>,
) -> (Vec<AgentFlowNode>, Vec<AgentFlowEdge>) {
    let mut new_nodes = Vec::new();
    let mut node_id_map = HashMap::new();
    for node in nodes {
        let new_id = new_id();
        node_id_map.insert(node.id.clone(), new_id.clone());
        let mut new_node = node.clone();
        new_node.id = new_id;
        new_nodes.push(new_node);
    }

    let mut new_edges = Vec::new();
    for edge in edges {
        let Some(source) = node_id_map.get(&edge.source) else {
            continue;
        };
        let Some(target) = node_id_map.get(&edge.target) else {
            continue;
        };
        let mut new_edge = edge.clone();
        new_edge.id = new_id();
        new_edge.source = source.clone();
        new_edge.target = target.clone();
        new_edges.push(new_edge);
    }

    (new_nodes, new_edges)
}

// AgentFlowNode

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentFlowNode {
    pub id: String,
    pub def_name: String,
    pub enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<AgentConfig>,

    #[serde(flatten)]
    pub extensions: HashMap<String, Value>,
}

impl AgentFlowNode {
    pub fn new(def: &AgentDefinition) -> Result<Self, AgentError> {
        let config = if let Some(default_config) = &def.default_config {
            let mut config = AgentConfig::new();
            for (key, entry) in default_config {
                config.set(key.clone(), entry.value.clone());
            }
            Some(config)
        } else {
            None
        };

        Ok(Self {
            id: new_id(),
            def_name: def.name.clone(),
            enabled: false,
            config,
            extensions: HashMap::new(),
        })
    }
}

static NODE_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn new_id() -> String {
    return NODE_ID_COUNTER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .to_string();
}

// AgentFlowEdge

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentFlowEdge {
    pub id: String,
    pub source: String,
    pub source_handle: String,
    pub target: String,
    pub target_handle: String,
}
