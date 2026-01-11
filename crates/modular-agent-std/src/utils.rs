use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};

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
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PIN_RESET {
            self.count = 0;
        } else if pin == PIN_IN {
            self.count += 1;
        }
        self.set_config(DISPLAY_COUNT.to_string(), AgentValue::integer(self.count))?;
        self.output(ctx, PIN_COUNT, AgentValue::integer(self.count))
            .await?;
        self.emit_config_updated(DISPLAY_COUNT, AgentValue::integer(self.count));

        Ok(())
    }
}

/// Comment Agent
/// Show a comment in the stream editor.
#[askit_agent(
    title = "Comment",
    category = CATEGORY,
    text_config(name = "comment", hide_title),
)]
struct CommentAgent {
    data: AgentData,
}

impl AsAgent for CommentAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let data = AgentData::new(askit, id, spec);
        Ok(Self { data })
    }
}
