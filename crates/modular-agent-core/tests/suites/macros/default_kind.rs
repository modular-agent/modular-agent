use modular_agent_core::{
    AsModule, ModuleContext, ModuleData, ModuleSpec, Result, Value, async_trait, modular_agent,
};

#[modular_agent(title = "No Kind", category = "Tests")]
struct NoKindModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for NoKindModule {
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
fn default_kind_is_module() {
    let def = NoKindModule::module_definition();
    assert_eq!(def.kind, "Module");
    assert_eq!(def.title.as_deref(), Some("No Kind"));
    assert_eq!(def.category.as_deref(), Some("Tests"));
}
