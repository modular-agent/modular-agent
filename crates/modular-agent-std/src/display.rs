use std::vec;

use im::hashmap;
use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent, ModularAgent,
    async_trait, modular_agent,
};

const CATEGORY: &str = "Std/Display";

const PORT_VALUE: &str = "value";

const DISPLAY_VALUE: &str = "value";

// Display Value
#[modular_agent(
    kind = "Display",
    title = "Display Value",
    category = CATEGORY,
    inputs = [PORT_VALUE],
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
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.set_config(DISPLAY_VALUE.to_string(), value.clone())?;
        self.emit_config_updated(DISPLAY_VALUE, value);
        Ok(())
    }
}

// Debug Value
#[modular_agent(
    kind = "Display",
    title = "Debug Value",
    category = CATEGORY,
    inputs = [PORT_VALUE],
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
        let ctx_json =
            serde_json::to_value(&ctx).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let ctx = AgentValue::from_json(ctx_json)?;
        let debug_value =
            AgentValue::object(hashmap! { "ctx".into() => ctx, "value".into() => value });
        self.set_config(DISPLAY_VALUE.to_string(), debug_value.clone())?;
        self.emit_config_updated(DISPLAY_VALUE, debug_value);
        Ok(())
    }
}
