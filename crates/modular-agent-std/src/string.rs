use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentError, AgentOutput, AgentValue, AsAgent,
    AsAgentData, async_trait,
};
use askit_macros::askit_agent;
use handlebars::Handlebars;

static CATEGORY: &str = "Std/String";

static PIN_DATA: &str = "data";
static PIN_STRING: &str = "string";
static PIN_STRINGS: &str = "strings";

static CONFIG_SEP: &str = "sep";
static CONFIG_TEMPLATE: &str = "template";

/// The `StringJoinAgent` is responsible for joining an array of strings into a single string
/// using a specified separator. It processes input value, applies transformations to handle
/// escape sequences (e.g., `\n`, `\t`), and outputs the resulting string.
///
/// # Configuration
/// - `CONFIG_SEP`: Specifies the separator to use when joining strings. Defaults to an empty string.
///
/// # Input
/// - Expects an array of strings as input value.
///
/// # Output
/// - Produces a single joined string as output.
///
/// # Example
/// Given the input `["Hello", "World"]` and `CONFIG_SEP` set to `" "`, the output will be `"Hello World"`.
#[askit_agent(
    title = "String Join",
    category = CATEGORY,
    inputs = [PIN_STRINGS],
    outputs = [PIN_STRING],
    string_config(name = CONFIG_SEP, default = "\\n")
)]
struct StringJoinAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for StringJoinAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
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

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;

        let sep = config.get_string_or_default(CONFIG_SEP);

        if value.is_array() {
            let mut out = Vec::new();
            for v in value
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
            let out_value = AgentValue::string(out);
            self.try_output(ctx, PIN_STRING, out_value)
        } else {
            self.try_output(ctx, PIN_STRING, value)
        }
    }
}

// Template String Agent
#[askit_agent(
    title = "Template String",
    category = CATEGORY,
    inputs = [PIN_DATA],
    outputs = [PIN_STRING],
    string_config(name = CONFIG_TEMPLATE, default = "{{value}}")
)]
struct TemplateStringAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for TemplateStringAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
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

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;

        let template = config.get_string_or_default(CONFIG_TEMPLATE);
        if template.is_empty() {
            return Err(AgentError::InvalidConfig("template is not set".into()));
        }

        let reg = handlebars_new();

        if value.is_array() {
            let mut out_arr = Vec::new();
            for v in value
                .as_array()
                .ok_or_else(|| AgentError::InvalidArrayValue("Expected array".into()))?
            {
                let rendered_string = reg.render_template(&template, v).map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to render template: {}", e))
                })?;
                out_arr.push(rendered_string.into());
            }
            self.try_output(ctx, PIN_STRING, AgentValue::array(out_arr))
        } else {
            let rendered_string = reg.render_template(&template, &value).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            let out_value = AgentValue::string(rendered_string);
            self.try_output(ctx, PIN_STRING, out_value)
        }
    }
}

// Template Text Agent
#[askit_agent(
    title = "Template Text",
    category = CATEGORY,
    inputs = [PIN_DATA],
    outputs = [PIN_STRING],
    text_config(name = CONFIG_TEMPLATE, default = "{{value}}")
)]
struct TemplateTextAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for TemplateTextAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
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

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;

        let template = config.get_string_or_default(CONFIG_TEMPLATE);
        if template.is_empty() {
            return Err(AgentError::InvalidConfig("template is not set".into()));
        }

        let reg = handlebars_new();

        if value.is_array() {
            let mut out_arr = Vec::new();
            for v in value
                .as_array()
                .ok_or_else(|| AgentError::InvalidArrayValue("Expected array".into()))?
            {
                let rendered_string = reg.render_template(&template, v).map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to render template: {}", e))
                })?;
                out_arr.push(rendered_string.into());
            }
            self.try_output(ctx, PIN_STRING, AgentValue::array(out_arr))
        } else {
            let rendered_string = reg.render_template(&template, &value).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            let out_value = AgentValue::string(rendered_string);
            self.try_output(ctx, PIN_STRING, out_value)
        }
    }
}

// Template Array Agent
#[askit_agent(
    title = "Template Array",
    category = CATEGORY,
    inputs = [PIN_DATA],
    outputs = [PIN_STRING],
    text_config(name = CONFIG_TEMPLATE, default = "{{value}}")
)]
struct TemplateArrayAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for TemplateArrayAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
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

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;

        let template = config.get_string_or_default(CONFIG_TEMPLATE);
        if template.is_empty() {
            return Err(AgentError::InvalidConfig("template is not set".into()));
        }

        let reg = handlebars_new();

        if value.is_array() {
            let rendered_string = reg.render_template(&template, &value).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            self.try_output(ctx, PIN_STRING, AgentValue::string(rendered_string))
        } else {
            let d = AgentValue::array(vec![value.clone()]);
            let rendered_string = reg.render_template(&template, &d).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            let out_value = AgentValue::string(rendered_string);
            self.try_output(ctx, PIN_STRING, out_value)
        }
    }
}

fn handlebars_new<'a>() -> Handlebars<'a> {
    let mut reg = Handlebars::new();
    reg.register_escape_fn(handlebars::no_escape);
    reg.register_helper("to_json", Box::new(to_json_helper));

    #[cfg(feature = "yaml")]
    reg.register_helper("to_yaml", Box::new(to_yaml_helper));

    reg
}

fn to_json_helper(
    h: &handlebars::Helper<'_>,
    _: &handlebars::Handlebars<'_>,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext<'_, '_>,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    if let Some(value) = h.param(0) {
        let json_str = serde_json::to_string_pretty(&value.value()).map_err(|e| {
            handlebars::RenderErrorReason::Other(format!("Failed to serialize to JSON: {}", e))
        })?;
        out.write(&json_str)?;
    }
    Ok(())
}

#[cfg(feature = "yaml")]
fn to_yaml_helper(
    h: &handlebars::Helper<'_>,
    _: &handlebars::Handlebars<'_>,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext<'_, '_>,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    if let Some(value) = h.param(0) {
        let yaml_str = serde_yaml_ng::to_string(&value.value()).map_err(|e| {
            handlebars::RenderErrorReason::Other(format!("Failed to serialize to YAML: {}", e))
        })?;
        out.write(&yaml_str)?;
    }
    Ok(())
}
