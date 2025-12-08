use agent_stream_kit::AgentContext;
use agent_stream_kit::{AgentData, AgentError, AgentSpec, AgentValue, AsAgent, async_trait};
use askit_macros::askit_agent;

#[askit_agent(title = "No Kind", category = "Tests")]
struct NoKindAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for NoKindAgent {
    fn new(
        askit: agent_stream_kit::ASKit,
        id: String,
        spec: AgentSpec,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _pin: String,
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
