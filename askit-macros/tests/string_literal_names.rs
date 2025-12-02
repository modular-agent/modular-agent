use agent_stream_kit::{
    AgentConfigs, AgentContext, AgentData, AgentError, AgentValue, AsAgent, async_trait,
};
use askit_macros::askit_agent;

#[askit_agent(
    title = "Literal Name Agent",
    category = "Tests",
    string_config(name = "literal_config", default = "val"),
    string_global_config(name = "literal_global", default = "global_val"),
    string_display(name = "literal_display", title = "Literal Title"),
)]
struct LiteralNameAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for LiteralNameAgent {
    fn new(
        askit: agent_stream_kit::ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, configs),
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
fn string_literal_names_are_kept() {
    let def = LiteralNameAgent::agent_definition();

    let cfgs = def.default_configs.expect("default configs exist");
    let (cfg_key, cfg_entry) = cfgs.first().expect("config entry exists");
    assert_eq!(cfg_key, "literal_config");
    assert_eq!(cfg_entry.value, AgentValue::string("val"));

    let global_cfgs = def.global_configs.expect("global configs exist");
    let (g_key, g_entry) = global_cfgs.first().expect("global entry exists");
    assert_eq!(g_key, "literal_global");
    assert_eq!(g_entry.value, AgentValue::string("global_val"));

    let display_cfgs = def.display_configs.expect("display configs exist");
    let (d_key, d_entry) = display_cfgs.first().expect("display entry exists");
    assert_eq!(d_key, "literal_display");
    assert_eq!(d_entry.title.as_deref(), Some("Literal Title"));
}
