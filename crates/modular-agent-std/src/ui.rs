use modular_agent_core::{
    AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent, ModularAgent,
    async_trait, modular_agent,
};

const CATEGORY: &str = "Std/UI";

const NOTE: &str = "note";
const PORT_SP: &str = " ";

#[modular_agent(
    kind = "UI",
    title = "Note",
    category = CATEGORY,
    text_config(name = NOTE, hide_title)
)]
struct NoteAgent {
    data: AgentData,
}

impl AsAgent for NoteAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }
}

#[modular_agent(
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
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
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
