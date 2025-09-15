use agent_stream_kit::{
    ASKit, Agent, AgentConfig, AgentConfigEntry, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AgentValue, AsAgent, AsAgentData, new_boxed,
};
use async_trait::async_trait;
use handlebars::Handlebars;

/// The `StringJoinAgent` is responsible for joining an array of strings into a single string
/// using a specified separator. It processes input data, applies transformations to handle
/// escape sequences (e.g., `\n`, `\t`), and outputs the resulting string.
///
/// # Configuration
/// - `CONFIG_SEP`: Specifies the separator to use when joining strings. Defaults to an empty string.
///
/// # Input
/// - Expects an array of strings as input data.
///
/// # Output
/// - Produces a single joined string as output.
///
/// # Example
/// Given the input `["Hello", "World"]` and `CONFIG_SEP` set to `" "`, the output will be `"Hello World"`.
struct StringJoinAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for StringJoinAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let config = self.config().ok_or(AgentError::NoConfig)?;

        let sep = config.get_string_or_default(CONFIG_SEP);

        if data.is_array() {
            let mut out = Vec::new();
            for v in data
                .as_array()
                .ok_or_else(|| AgentError::InvalidArrayValue("Expected array".into()))?
            {
                out.push(v.as_str().unwrap_or_default());
            }
            let mut out = out.join(&sep);
            out = out.replace("\\n", "\n");
            out = out.replace("\\t", "\t");
            out = out.replace("\\r", "\r");
            out = out.replace("\\\\", "\\");
            let out_data = AgentData::new_string(out);
            self.try_output(ctx, PORT_STRING, out_data)
        } else {
            self.try_output(ctx, PORT_STRING, data)
        }
    }
}

/// The `TextJoinAgent` is responsible for joining an array of texts into a single text
/// using a specified separator. It processes input data, applies transformations to handle
/// escape sequences (e.g., `\n`, `\t`), and outputs the resulting text.
///
/// # Configuration
/// - `CONFIG_SEP`: Specifies the separator to use when joining texts. Defaults to an empty string.
///
/// # Input
/// - Expects an array of texts as input data.
///
/// # Output
/// - Produces a single joined text as output.
///
/// # Example
/// Given the input `["Hello", "World"]` and `CONFIG_SEP` set to `"\\n"`, the output will be `"Hello\nWorld"`.
struct TextJoinAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for TextJoinAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let config = self.config().ok_or(AgentError::NoConfig)?;

        let sep = config.get_string_or_default(CONFIG_SEP);

        if data.is_array() {
            let mut out = Vec::new();
            for v in data
                .as_array()
                .ok_or_else(|| AgentError::InvalidArrayValue("Expected array".into()))?
            {
                out.push(v.as_str().unwrap_or_default());
            }
            let mut out = out.join(&sep);
            out = out.replace("\\n", "\n");
            out = out.replace("\\t", "\t");
            out = out.replace("\\r", "\r");
            out = out.replace("\\\\", "\\");
            let out_data = AgentData::new_text(out);
            self.try_output(ctx, PORT_TEXT, out_data)
        } else {
            self.try_output(ctx, PORT_TEXT, data)
        }
    }
}

// Template String Agent
struct TemplateStringAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for TemplateStringAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let config = self.config().ok_or(AgentError::NoConfig)?;

        let template = config.get_string_or_default(CONFIG_TEMPLATE);
        if template.is_empty() {
            return Err(AgentError::InvalidConfig("template is not set".into()));
        }

        let reg = Handlebars::new();

        if data.is_array() {
            let kind = &data.kind;
            let mut out_arr = Vec::new();
            for v in data
                .as_array()
                .ok_or_else(|| AgentError::InvalidArrayValue("Expected array".into()))?
            {
                let d = AgentData {
                    kind: kind.clone(),
                    value: v.clone(),
                };
                let rendered_string = reg.render_template(&template, &d).map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to render template: {}", e))
                })?;
                out_arr.push(AgentValue::new_string(rendered_string));
            }
            self.try_output(ctx, PORT_STRING, AgentData::new_array("string", out_arr))
        } else {
            let rendered_string = reg.render_template(&template, &data).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            let out_data = AgentData::new_string(rendered_string);
            self.try_output(ctx, PORT_STRING, out_data)
        }
    }
}

// Template Text Agent
struct TemplateTextAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for TemplateTextAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let config = self.config().ok_or(AgentError::NoConfig)?;

        let template = config.get_string_or_default(CONFIG_TEMPLATE);
        if template.is_empty() {
            return Err(AgentError::InvalidConfig("template is not set".into()));
        }

        let reg = Handlebars::new();

        if data.is_array() {
            let kind = &data.kind;
            let mut out_arr = Vec::new();
            for v in data
                .as_array()
                .ok_or_else(|| AgentError::InvalidArrayValue("Expected array".into()))?
            {
                let d = AgentData {
                    kind: kind.clone(),
                    value: v.clone(),
                };
                let rendered_string = reg.render_template(&template, &d).map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to render template: {}", e))
                })?;
                out_arr.push(AgentValue::new_string(rendered_string));
            }
            self.try_output(ctx, PORT_TEXT, AgentData::new_array("text", out_arr))
        } else {
            let rendered_string = reg.render_template(&template, &data).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            let out_data = AgentData::new_text(rendered_string);
            self.try_output(ctx, PORT_TEXT, out_data)
        }
    }
}

// Template Array Agent
struct TemplateArrayAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for TemplateArrayAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let config = self.config().ok_or(AgentError::NoConfig)?;

        let template = config.get_string_or_default(CONFIG_TEMPLATE);
        if template.is_empty() {
            return Err(AgentError::InvalidConfig("template is not set".into()));
        }

        let reg = Handlebars::new();

        if data.is_array() {
            let rendered_string = reg.render_template(&template, &data).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            self.try_output(ctx, PORT_TEXT, AgentData::new_text(rendered_string))
        } else {
            let kind = &data.kind;
            let d = AgentData::new_array(kind, vec![data.value.clone()]);
            let rendered_string = reg.render_template(&template, &d).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            let out_data = AgentData::new_text(rendered_string);
            self.try_output(ctx, PORT_TEXT, out_data)
        }
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/String";

static PORT_DATA: &str = "data";
static PORT_STRING: &str = "string";
static PORT_STRINGS: &str = "strings";
static PORT_TEXT: &str = "text";
static PORT_TEXTS: &str = "texts";

static CONFIG_SEP: &str = "sep";
static CONFIG_TEMPLATE: &str = "template";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_text_join",
            Some(new_boxed::<TextJoinAgent>),
        )
        .with_title("Text Join")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_TEXTS])
        .with_outputs(vec![PORT_TEXT])
        .with_default_config(vec![(
            CONFIG_SEP.into(),
            AgentConfigEntry::new(AgentValue::new_string("\\n"), "string"),
        )]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_string_join",
            Some(new_boxed::<StringJoinAgent>),
        )
        .with_title("String Join")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_STRINGS])
        .with_outputs(vec![PORT_STRING])
        .with_default_config(vec![(
            CONFIG_SEP.into(),
            AgentConfigEntry::new(AgentValue::new_string("\\n"), "string"),
        )]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_template_array",
            Some(new_boxed::<TemplateArrayAgent>),
        )
        .with_title("Template Array")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_DATA])
        .with_outputs(vec![PORT_TEXT])
        .with_default_config(vec![(
            CONFIG_TEMPLATE.into(),
            AgentConfigEntry::new(AgentValue::new_string("{{value}}"), "text"),
        )]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_template_string",
            Some(new_boxed::<TemplateStringAgent>),
        )
        .with_title("Template String")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_DATA])
        .with_outputs(vec![PORT_STRING])
        .with_default_config(vec![(
            CONFIG_TEMPLATE.into(),
            AgentConfigEntry::new(AgentValue::new_string("{{value}}"), "string"),
        )]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_template_text",
            Some(new_boxed::<TemplateTextAgent>),
        )
        .with_title("Template Text")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_DATA])
        .with_outputs(vec![PORT_TEXT])
        .with_default_config(vec![(
            CONFIG_TEMPLATE.into(),
            AgentConfigEntry::new(AgentValue::new_string("{{value}}"), "text"),
        )]),
    );
}
