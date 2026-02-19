use modular_agent_core::{
    AgentContext, AgentData, AgentError, AgentSpec, AgentValue, AsAgent, async_trait, modular_agent,
};

// --- Agent with integer hints ---

#[modular_agent(
    kind = "Test",
    title = "Hinted Agent",
    category = "Tests",
    hint(color = 3, width = 2, height = 1),
)]
struct HintedAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for HintedAgent {
    fn new(
        ma: modular_agent_core::ModularAgent,
        id: String,
        spec: AgentSpec,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[test]
fn hint_integer_entries() {
    let def = HintedAgent::agent_definition();
    assert_eq!(def.hints.len(), 3);
    assert_eq!(def.hints["color"], serde_json::json!(3));
    assert_eq!(def.hints["width"], serde_json::json!(2));
    assert_eq!(def.hints["height"], serde_json::json!(1));
}

// --- Agent with no hints ---

#[modular_agent(
    kind = "Test",
    title = "No Hints Agent",
    category = "Tests",
)]
struct NoHintsAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for NoHintsAgent {
    fn new(
        ma: modular_agent_core::ModularAgent,
        id: String,
        spec: AgentSpec,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[test]
fn no_hints_yields_empty_map() {
    let def = NoHintsAgent::agent_definition();
    assert!(def.hints.is_empty());
}

// --- Agent with string hints ---

#[modular_agent(
    kind = "Test",
    title = "String Hint Agent",
    category = "Tests",
    hint(label = "red", shape = "circle"),
)]
struct StringHintAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for StringHintAgent {
    fn new(
        ma: modular_agent_core::ModularAgent,
        id: String,
        spec: AgentSpec,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[test]
fn hint_string_entries() {
    let def = StringHintAgent::agent_definition();
    assert_eq!(def.hints["label"], serde_json::json!("red"));
    assert_eq!(def.hints["shape"], serde_json::json!("circle"));
}

// --- Agent with mixed-type hints ---

#[modular_agent(
    kind = "Test",
    title = "Mixed Hint Agent",
    category = "Tests",
    hint(color = 3, resizable = true, label = "custom"),
)]
struct MixedHintAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for MixedHintAgent {
    fn new(
        ma: modular_agent_core::ModularAgent,
        id: String,
        spec: AgentSpec,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[test]
fn hint_mixed_type_entries() {
    let def = MixedHintAgent::agent_definition();
    assert_eq!(def.hints["color"], serde_json::json!(3));
    assert_eq!(def.hints["resizable"], serde_json::json!(true));
    assert_eq!(def.hints["label"], serde_json::json!("custom"));
}

// --- Agent with multiple hint() calls (merge) ---

#[modular_agent(
    kind = "Test",
    title = "Multi Hint Agent",
    category = "Tests",
    hint(color = 3),
    hint(width = 2, height = 1),
)]
struct MultiHintAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for MultiHintAgent {
    fn new(
        ma: modular_agent_core::ModularAgent,
        id: String,
        spec: AgentSpec,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[test]
fn multiple_hint_calls_merge() {
    let def = MultiHintAgent::agent_definition();
    assert_eq!(def.hints.len(), 3);
    assert_eq!(def.hints["color"], serde_json::json!(3));
    assert_eq!(def.hints["width"], serde_json::json!(2));
    assert_eq!(def.hints["height"], serde_json::json!(1));
}
