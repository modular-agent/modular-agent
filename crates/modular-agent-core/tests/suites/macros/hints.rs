use modular_agent_core::{
    AsModule, ModuleContext, ModuleData, ModuleSpec, Result, Value, async_trait, modular_agent,
};

// --- Module with integer hints ---

#[modular_agent(
    kind = "Test",
    title = "Hinted Module",
    category = "Tests",
    hint(color = 3, width = 2, height = 1)
)]
struct HintedModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for HintedModule {
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
fn hint_integer_entries() {
    let def = HintedModule::module_definition();
    assert_eq!(def.hints.len(), 3);
    assert_eq!(def.hints["color"], serde_json::json!(3));
    assert_eq!(def.hints["width"], serde_json::json!(2));
    assert_eq!(def.hints["height"], serde_json::json!(1));
}

// --- Module with no hints ---

#[modular_agent(kind = "Test", title = "No Hints Module", category = "Tests")]
struct NoHintsModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for NoHintsModule {
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
fn no_hints_yields_empty_map() {
    let def = NoHintsModule::module_definition();
    assert!(def.hints.is_empty());
}

// --- Module with string hints ---

#[modular_agent(
    kind = "Test",
    title = "String Hint Module",
    category = "Tests",
    hint(label = "red", shape = "circle")
)]
struct StringHintModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for StringHintModule {
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
fn hint_string_entries() {
    let def = StringHintModule::module_definition();
    assert_eq!(def.hints["label"], serde_json::json!("red"));
    assert_eq!(def.hints["shape"], serde_json::json!("circle"));
}

// --- Module with mixed-type hints ---

#[modular_agent(
    kind = "Test",
    title = "Mixed Hint Module",
    category = "Tests",
    hint(color = 3, resizable = true, label = "custom")
)]
struct MixedHintModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for MixedHintModule {
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
fn hint_mixed_type_entries() {
    let def = MixedHintModule::module_definition();
    assert_eq!(def.hints["color"], serde_json::json!(3));
    assert_eq!(def.hints["resizable"], serde_json::json!(true));
    assert_eq!(def.hints["label"], serde_json::json!("custom"));
}

// --- Module with multiple hint() calls (merge) ---

#[modular_agent(
    kind = "Test",
    title = "Multi Hint Module",
    category = "Tests",
    hint(color = 3),
    hint(width = 2, height = 1)
)]
struct MultiHintModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for MultiHintModule {
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
fn multiple_hint_calls_merge() {
    let def = MultiHintModule::module_definition();
    assert_eq!(def.hints.len(), 3);
    assert_eq!(def.hints["color"], serde_json::json!(3));
    assert_eq!(def.hints["width"], serde_json::json!(2));
    assert_eq!(def.hints["height"], serde_json::json!(1));
}
