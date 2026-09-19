use modular_agent_core::{
    AsModule, ModuleContext, ModuleData, ModuleSpec, Result, Value, async_trait, modular_agent,
};

// --- Single-line doc comment ---

/// Echoes input to output.
#[modular_agent(kind = "Test", title = "DocSingle", category = "Tests")]
struct DocSingleModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for DocSingleModule {
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
fn single_line_doc_becomes_description() {
    let def = DocSingleModule::module_definition();
    assert_eq!(def.description.as_deref(), Some("Echoes input to output."));
}

// --- Multi-line doc comment ---

/// Adds a constant integer
/// to the input value.
#[modular_agent(kind = "Test", title = "DocMulti", category = "Tests")]
struct DocMultiModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for DocMultiModule {
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
fn multi_line_doc_joined_with_newline() {
    let def = DocMultiModule::module_definition();
    assert_eq!(
        def.description.as_deref(),
        Some("Adds a constant integer\nto the input value.")
    );
}

// --- Doc comment with blank line (paragraph break) ---

/// First paragraph.
///
/// Second paragraph.
#[modular_agent(kind = "Test", title = "DocParagraph", category = "Tests")]
struct DocParagraphModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for DocParagraphModule {
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
fn blank_line_doc_produces_paragraph_break() {
    let def = DocParagraphModule::module_definition();
    assert_eq!(
        def.description.as_deref(),
        Some("First paragraph.\n\nSecond paragraph.")
    );
}

// --- Explicit description overrides doc comment ---

/// This doc comment should be ignored.
#[modular_agent(
    kind = "Test",
    title = "DocExplicit",
    category = "Tests",
    description = "Explicit wins"
)]
struct DocExplicitModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for DocExplicitModule {
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
fn explicit_description_overrides_doc_comment() {
    let def = DocExplicitModule::module_definition();
    assert_eq!(def.description.as_deref(), Some("Explicit wins"));
}

// --- No doc comment and no description ---

#[modular_agent(kind = "Test", title = "NoDoc", category = "Tests")]
struct NoDocModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for NoDocModule {
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
fn no_doc_no_description_is_none() {
    let def = NoDocModule::module_definition();
    assert!(def.description.is_none());
}
