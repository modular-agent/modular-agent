use std::vec;

use agent_stream_kit::{
    ASKit, AgentConfigs, AgentContext, AgentError, AgentOutput, AgentValue, AsAgent, AsAgentData,
    async_trait,
};
use askit_macros::askit_agent;

static CATEGORY: &str = "Std/Utils";

static PIN_IN: &str = "in";
static PIN_RESET: &str = "reset";
static PIN_COUNT: &str = "count";

static DISPLAY_COUNT: &str = "count";

/// Counter
#[askit_agent(
    title = "Counter",
    category = CATEGORY,
    inputs = [PIN_IN, PIN_RESET],
    outputs = [PIN_COUNT],
    integer_display(name = DISPLAY_COUNT, hide_title)
)]
struct CounterAgent {
    data: AsAgentData,
    count: i64,
}

#[async_trait]
impl AsAgent for CounterAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            count: 0,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        self.count = 0;
        self.emit_display(DISPLAY_COUNT, AgentValue::integer(0));
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
        self.try_output(ctx, PIN_COUNT, AgentValue::integer(self.count))?;
        self.emit_display(DISPLAY_COUNT, AgentValue::integer(self.count));

        Ok(())
    }
}
