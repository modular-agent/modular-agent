#![cfg(feature = "yaml")]

use std::vec;

use agent_stream_kit::{
    ASKit, AgentConfigs, AgentContext, AgentError, AgentOutput, AgentValue, AsAgent, AsAgentData,
    async_trait,
};
use askit_macros::askit_agent;

static CATEGORY: &str = "Std/Yaml";

static PIN_DATA: &str = "data";
static PIN_YAML: &str = "yaml";

// To YAML
#[askit_agent(
    title = "To YAML",
    category = CATEGORY,
    inputs = [PIN_DATA],
    outputs = [PIN_YAML]
)]
struct ToYamlAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ToYamlAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let yaml = serde_yaml_ng::to_string(&value)
            .map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        self.try_output(ctx, PIN_YAML, AgentValue::string(yaml))?;
        Ok(())
    }
}

// From YAML
#[askit_agent(
    title = "From YAML",
    category = CATEGORY,
    inputs = [PIN_YAML],
    outputs = [PIN_DATA]
)]
struct FromYamlAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for FromYamlAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let s = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("not a string".to_string()))?;
        let v: serde_json::Value =
            serde_yaml_ng::from_str(s).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let value = AgentValue::from_json(v)?;
        self.try_output(ctx, PIN_DATA, value)?;
        Ok(())
    }
}
