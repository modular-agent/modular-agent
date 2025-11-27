use std::vec;

use agent_stream_kit::{
    ASKit, AgentConfigs, AgentContext, AgentError, AgentOutput, AgentValue, AsAgent, AsAgentData,
    async_trait,
};
use askit_macros::askit_agent;

static CATEGORY: &str = "Std/Display";
static DISPLAY_DATA: &str = "data";

// Display Data
#[askit_agent(
    title = "Display Data",
    category = CATEGORY,
    inputs = ["*"],
    any_display(name = DISPLAY_DATA, hide_title)
)]
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

    async fn start(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.emit_display(DISPLAY_DATA, value);
        Ok(())
    }
}

// Debug Data
#[askit_agent(
    title = "Debug Data",
    category = CATEGORY,
    inputs = ["*"],
    object_display(name = DISPLAY_DATA, hide_title)
)]
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
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let value = AgentValue::object([("value".to_string(), value)].into());
        let ctx_json =
            serde_json::to_value(&ctx).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let ctx = AgentValue::from_json(ctx_json)?;
        let debug_value =
            AgentValue::object([("ctx".to_string(), ctx), ("value".to_string(), value)].into());
        self.emit_display(DISPLAY_DATA, debug_value);
        Ok(())
    }
}
