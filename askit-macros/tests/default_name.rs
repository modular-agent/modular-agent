use agent_stream_kit::AgentContext;
use agent_stream_kit::{AgentConfigs, AgentError, AgentValue, AsAgent, AsAgentData, async_trait};
use askit_macros::askit_agent;

static CONFIG_KEY: &str = "config_key";

#[askit_agent(kind = "Test", title = "DefaultName", category = "Tests")]
struct MyAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for MyAgent {
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

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
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
fn default_name_uses_module_path_and_ident() {
    let def = MyAgent::agent_definition();
    assert_eq!(def.name, concat!(module_path!(), "::", stringify!(MyAgent)));
}

#[askit_agent(
    kind = "CustomAgent",
    name = "custom_name",
    title = "Custom Title",
    category = "Custom Category",
    inputs = ["in_a", "in_b"],
    outputs = ["out_x"],
    string_config(
        name = CONFIG_KEY,
        default = "default_value",
        title = "Config Title",
        description = "Config Description"
    )
)]
struct MyAgentExplicit {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for MyAgentExplicit {
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

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
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
fn explicit_fields_and_configs_are_set() {
    let def = MyAgentExplicit::agent_definition();
    assert_eq!(def.kind, "CustomAgent");
    assert_eq!(def.name, "custom_name");
    assert_eq!(def.title.as_deref(), Some("Custom Title"));
    assert_eq!(def.category.as_deref(), Some("Custom Category"));
    assert_eq!(
        def.inputs.as_deref(),
        Some(&["in_a".into(), "in_b".into()][..])
    );
    assert_eq!(def.outputs.as_deref(), Some(&["out_x".into()][..]));

    let cfgs = def.default_configs.expect("default configs exist");
    let (key, entry) = cfgs.first().expect("one config entry");
    assert_eq!(key, CONFIG_KEY);
    assert_eq!(entry.value, AgentValue::string("default_value"));
    assert_eq!(entry.title.as_deref(), Some("Config Title"));
    assert_eq!(entry.description.as_deref(), Some("Config Description"));
}
