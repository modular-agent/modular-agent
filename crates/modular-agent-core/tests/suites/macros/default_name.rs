use modular_agent_core::{
    AsModule, ModuleContext, ModuleData, ModuleSpec, Result, Value, async_trait, modular_agent,
};

static CONFIG_KEY: &str = "config_key";

#[modular_agent(kind = "Test", title = "DefaultName", category = "Tests")]
struct MyModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for MyModule {
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
fn default_name_uses_module_path_and_ident() {
    let def = MyModule::module_definition();
    assert_eq!(
        def.name,
        concat!(module_path!(), "::", stringify!(MyModule))
    );
}

#[modular_agent(
    kind = "CustomModule",
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
struct MyModuleExplicit {
    data: ModuleData,
}

#[async_trait]
impl AsModule for MyModuleExplicit {
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
fn explicit_fields_and_configs_are_set() {
    let def = MyModuleExplicit::module_definition();
    assert_eq!(def.kind, "CustomModule");
    assert_eq!(def.name, "custom_name");
    assert_eq!(def.title.as_deref(), Some("Custom Title"));
    assert_eq!(def.category.as_deref(), Some("Custom Category"));
    assert_eq!(
        def.inputs.as_deref(),
        Some(&["in_a".into(), "in_b".into()][..])
    );
    assert_eq!(def.outputs.as_deref(), Some(&["out_x".into()][..]));

    let cfgs = def.configs.expect("default configs exist");
    let (key, entry) = cfgs.first().expect("one config entry");
    assert_eq!(key, CONFIG_KEY);
    assert_eq!(entry.value, Value::string("default_value"));
    assert_eq!(entry.title.as_deref(), Some("Config Title"));
    assert_eq!(entry.description.as_deref(), Some("Config Description"));
}
