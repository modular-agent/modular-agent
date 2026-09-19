use modular_agent_core::{
    AsModule, ModuleContext, ModuleData, ModuleSpec, Result, Value, async_trait, modular_agent,
};

#[modular_agent(
    title = "Literal Name Module",
    category = "Tests",
    string_config(name = "literal_config", default = "val"),
    string_global_config(name = "literal_global", default = "global_val")
)]
struct LiteralNameModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for LiteralNameModule {
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
fn string_literal_names_are_kept() {
    let def = LiteralNameModule::module_definition();

    let cfgs = def.configs.expect("default configs exist");
    let (cfg_key, cfg_entry) = cfgs.first().expect("config entry exists");
    assert_eq!(cfg_key, "literal_config");
    assert_eq!(cfg_entry.value, Value::string("val"));

    let global_cfgs = def.global_configs.expect("global configs exist");
    let (g_key, g_entry) = global_cfgs.first().expect("global entry exists");
    assert_eq!(g_key, "literal_global");
    assert_eq!(g_entry.value, Value::string("global_val"));
}
