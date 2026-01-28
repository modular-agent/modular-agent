use std::vec;

use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent, ModularAgent,
    async_trait, modular_agent,
};

const CATEGORY: &str = "Std/Utils";

const PORT_IN: &str = "in";
const PORT_RESET: &str = "reset";
const PORT_COUNT: &str = "count";

const DISPLAY_COUNT: &str = "count";

/// Counter
#[modular_agent(
    title = "Counter",
    category = CATEGORY,
    inputs = [PORT_IN, PORT_RESET],
    outputs = [PORT_COUNT],
    integer_config(
        name = DISPLAY_COUNT,
        readonly,
        hide_title,
    )
)]
struct CounterAgent {
    data: AgentData,
    count: i64,
}

#[async_trait]
impl AsAgent for CounterAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            count: 0,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        self.count = 0;
        self.set_config(DISPLAY_COUNT.to_string(), AgentValue::integer(0))?;
        self.emit_config_updated(DISPLAY_COUNT, AgentValue::integer(0));
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        if port == PORT_RESET {
            self.count = 0;
        } else if port == PORT_IN {
            self.count += 1;
        }
        self.set_config(DISPLAY_COUNT.to_string(), AgentValue::integer(self.count))?;
        self.output(ctx, PORT_COUNT, AgentValue::integer(self.count))
            .await?;
        self.emit_config_updated(DISPLAY_COUNT, AgentValue::integer(self.count));

        Ok(())
    }
}
