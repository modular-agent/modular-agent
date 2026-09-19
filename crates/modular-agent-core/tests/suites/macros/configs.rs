use std::collections::HashMap;

use im::vector;
use modular_agent_core::{
    AsModule, ModuleContext, ModuleData, ModuleSpec, Result, Value, async_trait, modular_agent,
};

const UNIT_KEY: &str = "unit";
const BOOLEAN_KEY: &str = "boolean";
const INTEGER_KEY: &str = "integer";
const NUMBER_KEY: &str = "number";
const STRING_KEY: &str = "string";
const TEXT_KEY: &str = "text";
const ARRAY_KEY: &str = "array";
const OBJECT_KEY: &str = "object";
const CUSTOM_KEY: &str = "custom";
const GLOBAL_BOOLEAN_KEY: &str = "global_boolean";
const GLOBAL_INTEGER_KEY: &str = "global_integer";
const GLOBAL_NUMBER_KEY: &str = "global_number";
const GLOBAL_STRING_KEY: &str = "global_string";
const GLOBAL_TEXT_KEY: &str = "global_text";
const GLOBAL_ARRAY_KEY: &str = "global_array";
const GLOBAL_OBJECT_KEY: &str = "global_object";
const GLOBAL_CUSTOM_KEY: &str = "global_custom";

#[modular_agent(
    kind = "Test",
    title = "Config Module",
    category = "Tests",
    unit_config(name = UNIT_KEY),
    boolean_config(name = BOOLEAN_KEY, default = true, title = "Bool Title"),
    integer_config(name = INTEGER_KEY, default = 7, hidden),
    number_config(name = NUMBER_KEY, default = 2.5, description = "num"),
    string_config(name = STRING_KEY, default = "hello", detail),
    text_config(name = TEXT_KEY, default = "long"),
    array_config(
        name = ARRAY_KEY,
        default = Value::from(vector![Value::integer(1), Value::string("two")]),
        title = "Arr"
    ),
    object_config(
        name = OBJECT_KEY,
        default = Value::object_default(),
        title = "Obj",
        description = "Obj desc"
    ),
    custom_config(
        name = CUSTOM_KEY,
        type_ = "custom",
        default = Value::string("c"),
        title = "Custom",
        description = "Custom desc",
        detail
    ),
    boolean_global_config(name = GLOBAL_BOOLEAN_KEY, title = "Global Bool"),
    integer_global_config(name = GLOBAL_INTEGER_KEY, default = -1),
    number_global_config(name = GLOBAL_NUMBER_KEY, default = 2.71, description = "e", hidden),
    string_global_config(name = GLOBAL_STRING_KEY, default = "gs", detail),
    text_global_config(name = GLOBAL_TEXT_KEY, default = "gt"),
    array_global_config(name = GLOBAL_ARRAY_KEY),
    object_global_config(
        name = GLOBAL_OBJECT_KEY,
        default = Value::object_default(),
        title = "GObj",
        description = "Global obj"
    ),
    custom_global_config(
        name = GLOBAL_CUSTOM_KEY,
        type_ = "gcustom",
        default = Value::string("gc"),
        title = "GCustom",
        description = "Global custom desc"
    )
)]
struct ConfigModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for ConfigModule {
    fn new(ma: modular_agent_core::ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, _ctx: ModuleContext, _port: String, _value: Value) -> Result<()> {
        Ok(())
    }
}

#[test]
fn def_name_is_generated() {
    let expected = concat!(module_path!(), "::", stringify!(ConfigModule));
    assert_eq!(ConfigModule::DEF_NAME, expected);
    assert_eq!(ConfigModule::def_name(), expected);
}

#[test]
fn config_entries_are_generated() {
    let def = ConfigModule::module_definition();
    let configs: HashMap<_, _> = def.configs.expect("default configs").into_iter().collect();

    let array_entry = &configs[ARRAY_KEY];
    assert_eq!(array_entry.type_.as_deref(), Some("array"));
    assert_eq!(
        array_entry.value,
        Value::array(vector![Value::integer(1), Value::string("two")])
    );
    assert_eq!(array_entry.title.as_deref(), Some("Arr"));

    assert_eq!(configs[UNIT_KEY].type_.as_deref(), Some("unit"));
    assert_eq!(configs[UNIT_KEY].value, Value::unit());

    let bool_entry = &configs[BOOLEAN_KEY];
    assert_eq!(bool_entry.type_.as_deref(), Some("boolean"));
    assert_eq!(bool_entry.value, Value::boolean(true));
    assert_eq!(bool_entry.title.as_deref(), Some("Bool Title"));

    assert_eq!(configs[INTEGER_KEY].value, Value::integer(7));
    assert!(configs[INTEGER_KEY].hidden);
    assert_eq!(configs[NUMBER_KEY].description.as_deref(), Some("num"));
    assert_eq!(configs[STRING_KEY].value, Value::string("hello"));
    assert!(configs[STRING_KEY].detail);
    assert_eq!(configs[TEXT_KEY].value, Value::string("long"));

    let obj_entry = &configs[OBJECT_KEY];
    assert_eq!(obj_entry.type_.as_deref(), Some("object"));
    assert_eq!(obj_entry.title.as_deref(), Some("Obj"));
    assert_eq!(obj_entry.description.as_deref(), Some("Obj desc"));

    let custom_entry = &configs[CUSTOM_KEY];
    assert_eq!(custom_entry.type_.as_deref(), Some("custom"));
    assert_eq!(custom_entry.value, Value::string("c"));
    assert_eq!(custom_entry.title.as_deref(), Some("Custom"));
    assert_eq!(custom_entry.description.as_deref(), Some("Custom desc"));
    assert!(custom_entry.detail);
}

#[test]
fn global_config_entries_are_generated() {
    let def = ConfigModule::module_definition();
    let configs: HashMap<_, _> = def
        .global_configs
        .expect("global configs")
        .into_iter()
        .collect();

    let array_entry = &configs[GLOBAL_ARRAY_KEY];
    assert_eq!(array_entry.type_.as_deref(), Some("array"));
    assert_eq!(array_entry.value, Value::array_default());

    let bool_entry = &configs[GLOBAL_BOOLEAN_KEY];
    assert_eq!(bool_entry.type_.as_deref(), Some("boolean"));
    assert_eq!(bool_entry.value, Value::boolean(false));
    assert_eq!(bool_entry.title.as_deref(), Some("Global Bool"));

    assert_eq!(configs[GLOBAL_INTEGER_KEY].value, Value::integer(-1));
    assert_eq!(configs[GLOBAL_NUMBER_KEY].description.as_deref(), Some("e"));
    assert!(configs[GLOBAL_NUMBER_KEY].hidden);
    assert_eq!(configs[GLOBAL_STRING_KEY].value, Value::string("gs"));
    assert!(configs[GLOBAL_STRING_KEY].detail);
    assert_eq!(configs[GLOBAL_TEXT_KEY].value, Value::string("gt"));

    let obj_entry = &configs[GLOBAL_OBJECT_KEY];
    assert_eq!(obj_entry.type_.as_deref(), Some("object"));
    assert_eq!(obj_entry.title.as_deref(), Some("GObj"));
    assert_eq!(obj_entry.description.as_deref(), Some("Global obj"));

    let custom_entry = &configs[GLOBAL_CUSTOM_KEY];
    assert_eq!(custom_entry.type_.as_deref(), Some("gcustom"));
    assert_eq!(custom_entry.value, Value::string("gc"));
    assert_eq!(custom_entry.title.as_deref(), Some("GCustom"));
    assert_eq!(
        custom_entry.description.as_deref(),
        Some("Global custom desc")
    );
}
