use modular_agent_core::{
    AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent, ModularAgent,
    async_trait, modular_agent,
};

const CATEGORY: &str = "Core/Utils";

const PORT_IN: &str = "in";
const PORT_RESET: &str = "reset";
const PORT_COUNT: &str = "count";
const CONFIG_INITIAL_COUNT: &str = "initial_count";
const GLOBAL_STRING: &str = "global_string";

/// Counter
#[modular_agent(
    title = "Counter",
    category = CATEGORY,
    inputs = [PORT_IN, PORT_RESET],
    outputs = [PORT_COUNT],
    integer_config(name = CONFIG_INITIAL_COUNT, default = 1),
    string_global_config(name = GLOBAL_STRING, default = "gs"),
)]
pub struct CounterAgent {
    data: AgentData,
    pub count: i64,
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
        // self.emit_display(DISPLAY_COUNT, AgentData::new_integer(0))?;
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
        self.output(ctx, PORT_COUNT, AgentValue::integer(self.count))
            .await?;
        // self.emit_display(DISPLAY_COUNT, AgentValue::integer(self.count))?;
        Ok(())
    }
}
