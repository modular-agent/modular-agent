#![cfg(feature = "yaml")]

use std::vec;

use agent_stream_kit::{
    ASKit, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentError, AgentOutput, AsAgent,
    AsAgentData, async_trait, new_agent_boxed,
};

// To YAML
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
        data: AgentData,
    ) -> Result<(), AgentError> {
        let yaml = serde_yaml_ng::to_string(&data.value)
            .map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        self.try_output(ctx, PIN_YAML, AgentData::string(yaml))?;
        Ok(())
    }
}

// From YAML
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
        data: AgentData,
    ) -> Result<(), AgentError> {
        let s = data
            .value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("not a string".to_string()))?;
        let value: serde_json::Value =
            serde_yaml_ng::from_str(s).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let data = AgentData::from_json(value)?;
        self.try_output(ctx, PIN_DATA, data)?;
        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/Data";

static PIN_DATA: &str = "data";
static PIN_YAML: &str = "yaml";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_to_yaml",
            Some(new_agent_boxed::<ToYamlAgent>),
        )
        .title("To YAML")
        .category(CATEGORY)
        .inputs(vec![PIN_DATA])
        .outputs(vec![PIN_YAML]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_from_yaml",
            Some(new_agent_boxed::<FromYamlAgent>),
        )
        .title("From YAML")
        .category(CATEGORY)
        .inputs(vec![PIN_YAML])
        .outputs(vec![PIN_DATA]),
    );
}
