use modular_agent_core::{
    AgentContext, AgentData, AgentError, AgentSpec, AgentValue, AsAgent, async_trait, modular_agent,
};

// --- Single-line doc comment ---

/// Echoes input to output.
#[modular_agent(kind = "Test", title = "DocSingle", category = "Tests")]
struct DocSingleAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for DocSingleAgent {
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
fn single_line_doc_becomes_description() {
    let def = DocSingleAgent::agent_definition();
    assert_eq!(def.description.as_deref(), Some("Echoes input to output."));
}

// --- Multi-line doc comment ---

/// Adds a constant integer
/// to the input value.
#[modular_agent(kind = "Test", title = "DocMulti", category = "Tests")]
struct DocMultiAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for DocMultiAgent {
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
fn multi_line_doc_joined_with_newline() {
    let def = DocMultiAgent::agent_definition();
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
struct DocParagraphAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for DocParagraphAgent {
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
fn blank_line_doc_produces_paragraph_break() {
    let def = DocParagraphAgent::agent_definition();
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
struct DocExplicitAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for DocExplicitAgent {
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
fn explicit_description_overrides_doc_comment() {
    let def = DocExplicitAgent::agent_definition();
    assert_eq!(def.description.as_deref(), Some("Explicit wins"));
}

// --- No doc comment and no description ---

#[modular_agent(kind = "Test", title = "NoDoc", category = "Tests")]
struct NoDocAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for NoDocAgent {
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
fn no_doc_no_description_is_none() {
    let def = NoDocAgent::agent_definition();
    assert!(def.description.is_none());
}
