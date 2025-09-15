use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};

use super::data::AgentValue;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentContext {
    port: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    vars: Option<Arc<BTreeMap<String, AgentValue>>>,
}

impl AgentContext {
    pub fn new() -> Self {
        Self::default()
    }

    // Port

    pub fn new_with_port(port: impl Into<String>) -> Self {
        Self {
            port: port.into(),
            vars: None,
        }
    }

    pub fn with_port(&self, port: impl Into<String>) -> Self {
        Self {
            port: port.into(),
            vars: self.vars.clone(),
        }
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
            port: self.port.clone(),
            vars: Some(Arc::new(vars)),
        }
    }
}
