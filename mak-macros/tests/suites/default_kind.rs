use modular_agent_kit::{
    AgentContext, AgentData, AgentError, AgentSpec, AgentValue, AsAgent, mak_agent, async_trait,
};

#[mak_agent(title = "No Kind", category = "Tests")]
struct NoKindAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for NoKindAgent {
    fn new(
        mak: modular_agent_kit::MAK,
        id: String,
        spec: AgentSpec,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
        })
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[test]
fn default_kind_is_agent() {
    let def = NoKindAgent::agent_definition();
    assert_eq!(def.kind, "Agent");
    assert_eq!(def.title.as_deref(), Some("No Kind"));
    assert_eq!(def.category.as_deref(), Some("Tests"));
}
