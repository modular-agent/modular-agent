use agent_stream_kit::{async_trait, AgentConfigs, AgentError, AgentValue, AsAgent, AsAgentData};
use agent_stream_kit::AgentContext;
use askit_macros::askit_agent;

#[askit_agent(title = "No Kind", category = "Tests")]
struct NoKindAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for NoKindAgent {
    fn new(
        askit: agent_stream_kit::ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, configs),
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
