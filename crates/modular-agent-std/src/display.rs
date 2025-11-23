use std::vec;

use agent_stream_kit::{
    ASKit, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentDisplayConfigEntry,
    AgentError, AgentOutput, AgentValue, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};

// Display Data
struct DisplayDataAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for DisplayDataAgent {
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

    fn start(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        self.emit_display(DISPLAY_DATA, data);
        Ok(())
    }
}

// Debug Data
struct DebugDataAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for DebugDataAgent {
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
        let value = AgentValue::object(
            [
                ("kind".to_string(), data.kind.into()),
                ("value".to_string(), data.value),
            ]
            .into(),
        );
        let ctx_json =
            serde_json::to_value(&ctx).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let ctx = AgentValue::from_json(ctx_json)?;
        let debug_data =
            AgentData::object([("ctx".to_string(), ctx), ("data".to_string(), value)].into());
        self.emit_display(DISPLAY_DATA, debug_data);
        Ok(())
    }
}

static KIND: &str = "agent";
static CATEGORY: &str = "Core/Display";

static DISPLAY_DATA: &str = "data";

pub fn register_agents(askit: &ASKit) {
    // Display Data Agent
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_display_data",
            Some(new_agent_boxed::<DisplayDataAgent>),
        )
        .title("Display Data")
        .category(CATEGORY)
        .inputs(vec!["*"])
        .display_configs(vec![(
            DISPLAY_DATA,
            AgentDisplayConfigEntry::new("*").hide_title(),
        )]),
    );

    // Debug Data Agent
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_debug_data",
            Some(new_agent_boxed::<DebugDataAgent>),
        )
        .title("Debug Data")
        .category(CATEGORY)
        .inputs(vec!["*"])
        .display_configs(vec![(
            DISPLAY_DATA,
            AgentDisplayConfigEntry::new("object").hide_title(),
        )]),
    );
}
