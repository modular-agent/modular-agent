#![cfg(feature = "yaml")]

use std::vec;

use modular_agent_core::{
    AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent, ModularAgent,
    async_trait, modular_agent,
};

const CATEGORY: &str = "Std/Yaml";

const PORT_DATA: &str = "data";
const PORT_YAML: &str = "yaml";

// To YAML
#[modular_agent(
    title = "To YAML",
    category = CATEGORY,
    inputs = [PORT_DATA],
    outputs = [PORT_YAML],
    hint(color=5),
)]
struct ToYamlAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ToYamlAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let yaml = serde_yaml_ng::to_string(&value)
            .map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        self.output(ctx, PORT_YAML, AgentValue::string(yaml))
            .await?;
        Ok(())
    }
}

// From YAML
#[modular_agent(
    title = "From YAML",
    category = CATEGORY,
    inputs = [PORT_YAML],
    outputs = [PORT_DATA],
    hint(color=5),
)]
struct FromYamlAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for FromYamlAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let s = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("not a string".to_string()))?;
        let v: serde_json::Value =
            serde_yaml_ng::from_str(s).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let value = AgentValue::from_json(v)?;
        self.output(ctx, PORT_DATA, value).await?;
        Ok(())
    }
}
