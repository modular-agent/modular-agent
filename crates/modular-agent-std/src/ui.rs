use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};

const CATEGORY: &str = "Std/UI";

const COMMENT: &str = "comment";
const PIN_SP: &str = " ";

#[askit_agent(
    kind = "UI",
    title = "Comment",
    hide_title,
    category = CATEGORY,
    text_config(name = COMMENT, hide_title)
)]
struct CommentAgent {
    data: AgentData,
}

impl AsAgent for CommentAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }
}

#[askit_agent(
    kind = "UI",
    title = "Router",
    hide_title,
    category = CATEGORY,
    inputs=[PIN_SP],
    outputs=[PIN_SP],
)]
struct RouterAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for RouterAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.output(ctx, pin, value).await
    }
}
