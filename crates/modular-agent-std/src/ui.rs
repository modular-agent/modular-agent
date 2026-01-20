use modular_agent_kit::{
    MAK, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    mak_agent, async_trait,
};

const CATEGORY: &str = "Std/UI";

const COMMENT: &str = "comment";
const PORT_SP: &str = " ";

#[mak_agent(
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
    fn new(mak: MAK, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
        })
    }
}

#[mak_agent(
    kind = "UI",
    title = "Router",
    hide_title,
    category = CATEGORY,
    inputs=[PORT_SP],
    outputs=[PORT_SP],
)]
struct RouterAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for RouterAgent {
    fn new(mak: MAK, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.output(ctx, port, value).await
    }
}
