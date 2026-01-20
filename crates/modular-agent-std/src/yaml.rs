#![cfg(feature = "yaml")]

use std::vec;

use modular_agent_kit::{
    MAK, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    mak_agent, async_trait,
};

const CATEGORY: &str = "Std/Yaml";

const PORT_DATA: &str = "data";
const PORT_YAML: &str = "yaml";

// To YAML
#[mak_agent(
    title = "To YAML",
    category = CATEGORY,
    inputs = [PORT_DATA],
    outputs = [PORT_YAML]
)]
struct ToYamlAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ToYamlAgent {
    fn new(mak: MAK, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
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
        self.output(ctx, PORT_YAML, AgentValue::string(yaml)).await?;
        Ok(())
    }
}

// From YAML
#[mak_agent(
    title = "From YAML",
    category = CATEGORY,
    inputs = [PORT_YAML],
    outputs = [PORT_DATA]
)]
struct FromYamlAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for FromYamlAgent {
    fn new(mak: MAK, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
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
