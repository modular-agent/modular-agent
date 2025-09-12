use std::vec;

use async_trait::async_trait;

use agent_stream_kit::{
    ASKit, AgentConfig, AgentContext, AgentData, AgentDefinition, AgentDisplayConfigEntry,
    AgentError, AgentOutput, AgentValue, AgentValueMap, AsAgent, AsAgentData, new_boxed,
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
        config: Option<AgentConfig>,
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

    async fn process(&mut self, _ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
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
        config: Option<AgentConfig>,
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

    async fn process(&mut self, _ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let value = AgentValue::new_object(AgentValueMap::from([
            ("kind".to_string(), AgentValue::new_string(data.kind)),
            ("value".to_string(), data.value),
        ]));
        let json =
            serde_json::to_value(&value).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let ctx = AgentValue::from_json_value(json)?;
        let debug_data = AgentData::new_object(AgentValueMap::from([
            ("ctx".to_string(), ctx),
            ("data".to_string(), value),
        ]));
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
            Some(new_boxed::<DisplayDataAgent>),
        )
        .with_title("Display Data")
        .with_category(CATEGORY)
        .with_inputs(vec!["data"])
        .with_display_config(vec![(
            DISPLAY_DATA.into(),
            AgentDisplayConfigEntry::new("*").with_hide_title(),
        )]),
    );

    // Debug Data Agent
    askit.register_agent(
        AgentDefinition::new(KIND, "std_debug_data", Some(new_boxed::<DebugDataAgent>))
            .with_title("Debug Data")
            .with_category(CATEGORY)
            .with_inputs(vec!["*"])
            .with_display_config(vec![(
                DISPLAY_DATA.into(),
                AgentDisplayConfigEntry::new("object").with_hide_title(),
            )]),
    );
}
