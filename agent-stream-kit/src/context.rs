use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use serde::{Deserialize, Serialize};

use super::data::AgentValue;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentContext {
    id: usize,

    port: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    vars: Option<Arc<BTreeMap<String, AgentValue>>>,
}

impl AgentContext {
    pub fn new() -> Self {
        Self {
            id: new_id(),
            port: "".to_string(),
            vars: None,
        }
    }

    // Port

    pub fn new_with_port(port: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            port: port.into(),
            vars: None,
        }
    }

    pub fn with_port(&self, port: impl Into<String>) -> Self {
        Self {
            id: self.id,
            port: port.into(),
            vars: self.vars.clone(),
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn port(&self) -> &str {
        &self.port
    }

    // Variables

    pub fn get_var(&self, key: &str) -> Option<&AgentValue> {
        self.vars.as_ref().and_then(|vars| vars.get(key))
    }

    pub fn with_var(&self, key: String, value: AgentValue) -> Self {
        let mut vars = if let Some(vars) = &self.vars {
            vars.as_ref().clone()
        } else {
            BTreeMap::new()
        };
        vars.insert(key, value);
        Self {
            id: self.id,
            port: self.port.clone(),
            vars: Some(Arc::new(vars)),
        }
    }
}

static CONTEXT_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn new_id() -> usize {
    CONTEXT_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}
