use std::vec;

use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};

static CATEGORY: &str = "Std/Display";
static DISPLAY_VALUE: &str = "value";

// Display Value
#[askit_agent(
    title = "Display Value",
    category = CATEGORY,
    inputs = ["*"],
    custom_config(
        name = DISPLAY_VALUE,
        readonly,
        type_="*",
        default=AgentValue::unit(),
        hide_title,
    )
)]
struct DisplayValueAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for DisplayValueAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
    object_config(
        name = DISPLAY_VALUE,
        readonly,
        hide_title,
    )
)]
struct DebugValueAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for DebugValueAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let ctx_json =
            serde_json::to_value(&ctx).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let ctx = AgentValue::from_json(ctx_json)?;
        let debug_value =
            AgentValue::object([("ctx".to_string(), ctx), ("value".to_string(), value)].into());
        self.emit_display(DISPLAY_VALUE, debug_value);
        Ok(())
    }
}
