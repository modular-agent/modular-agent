use std::vec;

use agent_stream_kit::{
    ASKit, AgentConfigs, AgentContext, AgentError, AgentOutput, AgentValue, AsAgent, AgentData,
    async_trait,
};
use askit_macros::askit_agent;

static CATEGORY: &str = "Std/Display";
static DISPLAY_VALUE: &str = "value";

// Display Value
#[askit_agent(
    title = "Display Value",
    category = CATEGORY,
    inputs = ["*"],
    any_display(name = DISPLAY_VALUE, hide_title)
)]
struct DisplayValueAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for DisplayValueAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, config),
        })
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
        self.emit_display(DISPLAY_VALUE, value);
        Ok(())
    }
}

// Debug Value
#[askit_agent(
    title = "Debug Value",
    category = CATEGORY,
    inputs = ["*"],
    object_display(name = DISPLAY_VALUE, hide_title)
)]
struct DebugValueAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for DebugValueAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, config),
        })
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
        self.emit_display(DISPLAY_VALUE, debug_value);
        Ok(())
    }
}
