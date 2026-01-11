use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};

static CATEGORY: &str = "Core/Utils";

static PIN_IN: &str = "in";
static PIN_RESET: &str = "reset";
static PIN_COUNT: &str = "count";
static CONFIG_INITIAL_COUNT: &str = "initial_count";
static GLOBAL_STRING: &str = "global_string";

/// Counter
#[askit_agent(
    title = "Counter",
    category = CATEGORY,
    inputs = [PIN_IN, PIN_RESET],
    outputs = [PIN_COUNT],
    integer_config(name = CONFIG_INITIAL_COUNT, default = 1),
    string_global_config(name = GLOBAL_STRING, default = "gs"),
)]
pub struct CounterAgent {
    data: AgentData,
    pub count: i64,
}

#[async_trait]
impl AsAgent for CounterAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PIN_RESET {
            self.count = 0;
        } else if pin == PIN_IN {
            self.count += 1;
        }
        self.output(ctx, PIN_COUNT, AgentValue::integer(self.count))
            .await?;
        // self.emit_display(DISPLAY_COUNT, AgentValue::integer(self.count))?;
        Ok(())
    }
}
