use agent_stream_kit::{AgentConfigs, AgentError, AsAgent, AsAgentData, async_trait};
use askit_macros::askit_agent;

const UNIT_KEY: &str = "unit_disp";
const BOOLEAN_KEY: &str = "boolean_disp";
const INTEGER_KEY: &str = "integer_disp";
const NUMBER_KEY: &str = "number_disp";
const STRING_KEY: &str = "string_disp";
const TEXT_KEY: &str = "text_disp";
const OBJECT_KEY: &str = "object_disp";
const ANY_KEY: &str = "any";

#[askit_agent(
    title = "Display Agent",
    category = "Tests",
    unit_display(name = UNIT_KEY),
    boolean_display(name = BOOLEAN_KEY),
    integer_display(name = INTEGER_KEY, description = "Integer description"),
    number_display(name = NUMBER_KEY),
    string_display(name = STRING_KEY, title = "String Title"),
    text_display(name = TEXT_KEY),
    object_display(name = OBJECT_KEY),
    any_display(name = ANY_KEY, hide_title)
)]
struct DisplayAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for DisplayAgent {
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
}

#[test]
fn display_entries_are_generated() {
    let def = DisplayAgent::agent_definition();
    let displays = def.display_configs.expect("display configs");
    assert_eq!(displays.len(), 8);

    let mut map = std::collections::HashMap::new();
    for (k, v) in displays {
        map.insert(k, v);
    }

    let any = map.get(ANY_KEY).unwrap();
    assert_eq!(any.type_.as_deref(), Some("*"));
    assert!(any.hide_title);

    let str_disp = map.get(STRING_KEY).unwrap();
    assert_eq!(str_disp.type_.as_deref(), Some("string"));
    assert_eq!(str_disp.title.as_deref(), Some("String Title"));
    let int_disp = map.get(INTEGER_KEY).unwrap();
    assert_eq!(int_disp.type_.as_deref(), Some("integer"));
    assert_eq!(int_disp.description.as_deref(), Some("Integer description"));

    assert_eq!(map.get(UNIT_KEY).unwrap().type_.as_deref(), Some("unit"));
    assert_eq!(
        map.get(BOOLEAN_KEY).unwrap().type_.as_deref(),
        Some("boolean")
    );
    assert_eq!(
        map.get(INTEGER_KEY).unwrap().type_.as_deref(),
        Some("integer")
    );
    assert_eq!(
        map.get(NUMBER_KEY).unwrap().type_.as_deref(),
        Some("number")
    );
    assert_eq!(
        map.get(STRING_KEY).unwrap().type_.as_deref(),
        Some("string")
    );
    assert_eq!(map.get(TEXT_KEY).unwrap().type_.as_deref(), Some("text"));
    assert_eq!(
        map.get(OBJECT_KEY).unwrap().type_.as_deref(),
        Some("object")
    );
}
