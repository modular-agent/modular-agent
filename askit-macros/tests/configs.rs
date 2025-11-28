use agent_stream_kit::AgentContext;
use agent_stream_kit::{AgentConfigs, AgentError, AgentValue, AsAgent, AsAgentData, async_trait};
use askit_macros::askit_agent;
use std::collections::HashMap;

const UNIT_KEY: &str = "unit";
const BOOLEAN_KEY: &str = "boolean";
const INTEGER_KEY: &str = "integer";
const NUMBER_KEY: &str = "number";
const STRING_KEY: &str = "string";
const TEXT_KEY: &str = "text";
const OBJECT_KEY: &str = "object";
const GLOBAL_UNIT_KEY: &str = "global_unit";
const GLOBAL_BOOLEAN_KEY: &str = "global_boolean";
const GLOBAL_INTEGER_KEY: &str = "global_integer";
const GLOBAL_NUMBER_KEY: &str = "global_number";
const GLOBAL_STRING_KEY: &str = "global_string";
const GLOBAL_TEXT_KEY: &str = "global_text";
const GLOBAL_OBJECT_KEY: &str = "global_object";

#[askit_agent(
    kind = "Test",
    title = "Config Agent",
    category = "Tests",
    unit_config(name = UNIT_KEY),
    boolean_config(name = BOOLEAN_KEY, default = true, title = "Bool Title"),
    integer_config(name = INTEGER_KEY, default = 7),
    number_config(name = NUMBER_KEY, default = 3.14, description = "pi"),
    string_config(name = STRING_KEY, default = "hello"),
    text_config(name = TEXT_KEY, default = "long"),
    object_config(
        name = OBJECT_KEY,
        default = AgentValue::object_default(),
        title = "Obj",
        description = "Obj desc"
    ),
    unit_global_config(name = GLOBAL_UNIT_KEY),
    boolean_global_config(name = GLOBAL_BOOLEAN_KEY, title = "Global Bool"),
    integer_global_config(name = GLOBAL_INTEGER_KEY, default = -1),
    number_global_config(name = GLOBAL_NUMBER_KEY, default = 2.71, description = "e"),
    string_global_config(name = GLOBAL_STRING_KEY, default = "gs"),
    text_global_config(name = GLOBAL_TEXT_KEY, default = "gt"),
    object_global_config(
        name = GLOBAL_OBJECT_KEY,
        default = AgentValue::object_default(),
        title = "GObj",
        description = "Global obj"
    )
)]
struct ConfigAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ConfigAgent {
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
fn config_entries_are_generated() {
    let def = ConfigAgent::agent_definition();
    let configs: HashMap<_, _> = def
        .default_configs
        .expect("default configs")
        .into_iter()
        .collect();

    assert_eq!(configs[UNIT_KEY].type_.as_deref(), Some("unit"));
    assert_eq!(configs[UNIT_KEY].value, AgentValue::unit());

    let bool_entry = &configs[BOOLEAN_KEY];
    assert_eq!(bool_entry.type_.as_deref(), Some("boolean"));
    assert_eq!(bool_entry.value, AgentValue::boolean(true));
    assert_eq!(bool_entry.title.as_deref(), Some("Bool Title"));

    assert_eq!(configs[INTEGER_KEY].value, AgentValue::integer(7));
    assert_eq!(configs[NUMBER_KEY].description.as_deref(), Some("pi"));
    assert_eq!(configs[STRING_KEY].value, AgentValue::string("hello"));
    assert_eq!(configs[TEXT_KEY].value, AgentValue::string("long"));

    let obj_entry = &configs[OBJECT_KEY];
    assert_eq!(obj_entry.type_.as_deref(), Some("object"));
    assert_eq!(obj_entry.title.as_deref(), Some("Obj"));
    assert_eq!(obj_entry.description.as_deref(), Some("Obj desc"));
}

#[test]
fn global_config_entries_are_generated() {
    let def = ConfigAgent::agent_definition();
    let configs: HashMap<_, _> = def
        .global_configs
        .expect("global configs")
        .into_iter()
        .collect();

    assert_eq!(configs[GLOBAL_UNIT_KEY].type_.as_deref(), Some("unit"));

    let bool_entry = &configs[GLOBAL_BOOLEAN_KEY];
    assert_eq!(bool_entry.type_.as_deref(), Some("boolean"));
    assert_eq!(bool_entry.value, AgentValue::boolean(false));
    assert_eq!(bool_entry.title.as_deref(), Some("Global Bool"));

    assert_eq!(configs[GLOBAL_INTEGER_KEY].value, AgentValue::integer(-1));
    assert_eq!(configs[GLOBAL_NUMBER_KEY].description.as_deref(), Some("e"));
    assert_eq!(configs[GLOBAL_STRING_KEY].value, AgentValue::string("gs"));
    assert_eq!(configs[GLOBAL_TEXT_KEY].value, AgentValue::string("gt"));

    let obj_entry = &configs[GLOBAL_OBJECT_KEY];
    assert_eq!(obj_entry.type_.as_deref(), Some("object"));
    assert_eq!(obj_entry.title.as_deref(), Some("GObj"));
    assert_eq!(obj_entry.description.as_deref(), Some("Global obj"));
}
