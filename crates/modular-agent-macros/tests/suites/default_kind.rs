use modular_agent_core::{
    AgentContext, AgentData, AgentError, AgentSpec, AgentValue, AsAgent, modular_agent, async_trait,
};

#[modular_agent(title = "No Kind", category = "Tests")]
struct NoKindAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for NoKindAgent {
    fn new(
        ma: modular_agent_core::ModularAgent,
        id: String,
        spec: AgentSpec,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
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
