use std::sync::atomic::AtomicUsize;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::FnvIndexMap;
use crate::agent::AgentSpec;
use crate::askit::ASKit;
use crate::definition::{AgentDefinition, AgentDefinitions};
use crate::error::AgentError;

pub type AgentStreams = FnvIndexMap<String, AgentStream>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStream {
    #[serde(skip_serializing_if = "String::is_empty")]
    id: String,

    name: String,

    nodes: Vec<AgentStreamNode>,

    edges: Vec<AgentStreamEdge>,

    #[serde(flatten)]
    pub extensions: FnvIndexMap<String, Value>,
}

impl AgentStream {
    pub fn new(name: String) -> Self {
        Self {
            id: new_id(),
            name,
            nodes: Vec::new(),
            edges: Vec::new(),
            extensions: FnvIndexMap::default(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, new_name: String) {
        self.name = new_name;
    }

    pub fn nodes(&self) -> &Vec<AgentStreamNode> {
        &self.nodes
    }

    pub fn add_node(&mut self, node: AgentStreamNode) {
        self.nodes.push(node);
    }

    pub fn remove_node(&mut self, node_id: &str) {
        self.nodes.retain(|node| node.id != node_id);
    }

    pub fn set_nodes(&mut self, nodes: Vec<AgentStreamNode>) {
        self.nodes = nodes;
    }

    pub fn edges(&self) -> &Vec<AgentStreamEdge> {
        &self.edges
    }

    pub fn add_edge(&mut self, edge: AgentStreamEdge) {
        self.edges.push(edge);
    }

    pub fn remove_edge(&mut self, edge_id: &str) -> Option<AgentStreamEdge> {
        if let Some(edge) = self.edges.iter().find(|edge| edge.id == edge_id).cloned() {
            self.edges.retain(|e| e.id != edge_id);
            Some(edge)
        } else {
            None
        }
    }

    pub fn set_edges(&mut self, edges: Vec<AgentStreamEdge>) {
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
        let mut stream: AgentStream = serde_json::from_str(json_str)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        stream.id = new_id();
        Ok(stream)
    }

    /// Deserialize a stream with a compatibility layer for legacy node formats.
    /// Falls back to parsing the old shape and populates spec.inputs/outputs from AgentDefinitions.
    pub fn from_json_with_defs(
        json_str: &str,
        defs: &AgentDefinitions,
    ) -> Result<Self, AgentError> {
        match serde_json::from_str::<AgentStream>(json_str) {
            Ok(mut stream) => {
                stream.id = new_id();
                Ok(stream)
            }
            Err(deserialize_err) => {
                let legacy_json: Value = serde_json::from_str(json_str).map_err(|e| {
                    AgentError::SerializationError(format!("Failed to parse AgentStream json: {}", e))
                })?;

                let converted_json = convert_legacy_stream(legacy_json, defs).map_err(|e| {
                    AgentError::SerializationError(format!(
                        "Failed to deserialize AgentStream ({}); legacy format conversion failed: {}",
                        deserialize_err, e
                    ))
                })?;

                let mut stream: AgentStream = serde_json::from_value(converted_json).map_err(|e| {
                    AgentError::SerializationError(format!(
                        "Failed to deserialize converted AgentStream: {}",
                        e
                    ))
                })?;
                stream.id = new_id();
                Ok(stream)
            }
        }
    }
}

pub fn copy_sub_stream(
    nodes: &Vec<AgentStreamNode>,
    edges: &Vec<AgentStreamEdge>,
) -> (Vec<AgentStreamNode>, Vec<AgentStreamEdge>) {
    let mut new_nodes = Vec::new();
    let mut node_id_map = FnvIndexMap::default();
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

// AgentStreamNode

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentStreamNode {
    pub id: String,

    pub enabled: bool,

    pub spec: AgentSpec,

    #[serde(flatten)]
    pub extensions: FnvIndexMap<String, Value>,
}

impl AgentStreamNode {
    pub fn new(def: &AgentDefinition) -> Result<Self, AgentError> {
        let spec = def.to_spec();

        Ok(Self {
            id: new_id(),
            enabled: false,
            spec,
            extensions: FnvIndexMap::default(),
        })
    }
}

static NODE_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn new_id() -> String {
    return NODE_ID_COUNTER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .to_string();
}

fn convert_legacy_stream(
    mut legacy_json: Value,
    defs: &AgentDefinitions,
) -> Result<Value, AgentError> {
    let Some(obj) = legacy_json.as_object_mut() else {
        return Err(AgentError::SerializationError(
            "AgentStream json is not an object".to_string(),
        ));
    };

    let Some(nodes_val) = obj.get_mut("nodes") else {
        return Err(AgentError::SerializationError(
            "AgentStream json missing nodes".to_string(),
        ));
    };

    let Some(nodes) = nodes_val.as_array_mut() else {
        return Err(AgentError::SerializationError(
            "AgentStream nodes is not an array".to_string(),
        ));
    };

    for node in nodes.iter_mut() {
        convert_legacy_node(node, defs)?;
    }

    Ok(legacy_json)
}

fn convert_legacy_node(node_val: &mut Value, defs: &AgentDefinitions) -> Result<(), AgentError> {
    let Some(node_obj) = node_val.as_object_mut() else {
        return Err(AgentError::SerializationError(
            "AgentStream node is not an object".to_string(),
        ));
    };

    if node_obj.contains_key("spec") {
        return Ok(());
    }

    let def_name = node_obj
        .get("def_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            AgentError::SerializationError("Legacy node missing def_name".to_string())
        })?;

    let (inputs, outputs, def_display_configs) = defs
        .get(def_name.as_str())
        .map(|def| {
            (
                def.inputs.clone().unwrap_or_default(),
                def.outputs.clone().unwrap_or_default(),
                def.display_configs.clone(),
            )
        })
        .unwrap_or((Vec::new(), Vec::new(), None));

    let configs = node_obj.remove("configs").unwrap_or(Value::Null);
    let display_configs = node_obj
        .remove("display_configs")
        .or_else(|| def_display_configs.and_then(|cfg| serde_json::to_value(cfg).ok()));

    let mut spec_map = Map::new();
    spec_map.insert("def_name".into(), Value::String(def_name.to_string()));
    if !inputs.is_empty() {
        spec_map.insert(
            "inputs".into(),
            serde_json::to_value(inputs).map_err(|e| {
                AgentError::SerializationError(format!(
                    "Failed to serialize inputs for legacy node {}: {}",
                    def_name, e
                ))
            })?,
        );
    }
    if !outputs.is_empty() {
        spec_map.insert(
            "outputs".into(),
            serde_json::to_value(outputs).map_err(|e| {
                AgentError::SerializationError(format!(
                    "Failed to serialize outputs for legacy node {}: {}",
                    def_name, e
                ))
            })?,
        );
    }
    if !configs.is_null() {
        spec_map.insert("configs".into(), configs);
    }
    if let Some(display_configs) = display_configs {
        spec_map.insert("display_configs".into(), display_configs);
    }

    node_obj.remove("def_name");
    node_obj.insert("spec".into(), Value::Object(spec_map));

    Ok(())
}

// AgentStreamEdge

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentStreamEdge {
    pub id: String,
    pub source: String,
    pub source_handle: String,
    pub target: String,
    pub target_handle: String,
}
