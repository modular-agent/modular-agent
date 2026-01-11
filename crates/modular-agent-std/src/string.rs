use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use handlebars::Handlebars;
use im::vector;
use serde_json::json;

static CATEGORY: &str = "Std/String";

static PIN_STRING: &str = "string";
static PIN_STRINGS: &str = "strings";
static PIN_VALUE: &str = "value";
static PIN_T: &str = "t";
static PIN_F: &str = "f";

static CONFIG_LEN: &str = "len";
static CONFIG_OVERLAP: &str = "overlap";
static CONFIG_SEP: &str = "sep";
static CONFIG_TEMPLATE: &str = "template";

/// Check if the input is a string.
#[askit_agent(
    title = "IsString",
    category = CATEGORY,
    inputs = [PIN_VALUE],
    outputs = [PIN_T, PIN_F],
)]
struct IsStringAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for IsStringAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if value.is_string() {
            self.output(ctx, PIN_T, value).await
        } else {
            self.output(ctx, PIN_F, value).await
        }
    }
}

/// Check if the input string is empty.
#[askit_agent(
    title = "IsEmptyString",
    category = CATEGORY,
    inputs = [PIN_STRING],
    outputs = [PIN_T, PIN_F],
)]
struct IsEmptyStringAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for IsEmptyStringAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let is_empty = if let Some(s) = value.as_str() {
            s.is_empty()
        } else {
            false
        };
        if is_empty {
            self.output(ctx, PIN_T, value).await
        } else {
            self.output(ctx, PIN_F, value).await
        }
    }
}

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
    data: AgentData,
}

#[async_trait]
impl AsAgent for StringJoinAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
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
            self.output(ctx, PIN_STRING, out_value).await
        } else {
            self.output(ctx, PIN_STRING, value).await
        }
    }
}

#[askit_agent(
    title = "String Length Split",
    category = CATEGORY,
    inputs = [PIN_STRING],
    outputs = [PIN_STRINGS],
    integer_config(name = CONFIG_LEN, default = 65536),
    integer_config(name = CONFIG_OVERLAP, default = 1024),
)]
struct StringLengthSplitAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for StringLengthSplitAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;

        let n = config.get_integer_or_default(CONFIG_LEN) as usize;
        if n <= 0 {
            return Err(AgentError::InvalidConfig("n must be greater than 0".into()));
        }

        let overlap = config.get_integer_or_default(CONFIG_OVERLAP) as usize;
        if overlap >= n {
            return Err(AgentError::InvalidConfig(
                "overlap must be less than n".into(),
            ));
        }

        let s = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("Input value must be a string".into()))?;

        let mut out = Vec::new();
        let mut start = 0;
        let len = s.len();
        while start < len {
            let mut end = usize::min(start + n, len);
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            if end <= start {
                end = start + s[start..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            }

            out.push(AgentValue::string(s[start..end].to_string()));

            if end == len {
                break;
            }

            let mut next_start = end.saturating_sub(overlap);
            while next_start < len && !s.is_char_boundary(next_start) {
                next_start += 1;
            }
            start = next_start;
        }
        self.output(ctx, PIN_STRINGS, AgentValue::array(out.into()))
            .await
    }
}

// Template String Agent
#[askit_agent(
    title = "Template String",
    category = CATEGORY,
    inputs = [PIN_VALUE],
    outputs = [PIN_STRING],
    string_config(name = CONFIG_TEMPLATE, default = "{{value}}")
)]
struct TemplateStringAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for TemplateStringAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
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
                let data = json!({"value": v});
                let rendered_string = reg.render_template(&template, &data).map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to render template: {}", e))
                })?;
                out_arr.push(rendered_string.into());
            }
            self.output(ctx, PIN_STRING, AgentValue::array(out_arr.into()))
                .await
        } else {
            let data = json!({"value": value});
            let rendered_string = reg.render_template(&template, &data).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            let out_value = AgentValue::string(rendered_string);
            self.output(ctx, PIN_STRING, out_value).await
        }
    }
}

// Template Text Agent
#[askit_agent(
    title = "Template Text",
    category = CATEGORY,
    inputs = [PIN_VALUE],
    outputs = [PIN_STRING],
    text_config(name = CONFIG_TEMPLATE, default = "{{value}}")
)]
struct TemplateTextAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for TemplateTextAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
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
                let data = json!({"value": v});
                let rendered_string = reg.render_template(&template, &data).map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to render template: {}", e))
                })?;
                out_arr.push(rendered_string.into());
            }
            self.output(ctx, PIN_STRING, AgentValue::array(out_arr.into()))
                .await
        } else {
            let data = json!({"value": value});
            let rendered_string = reg.render_template(&template, &data).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            let out_value = AgentValue::string(rendered_string);
            self.output(ctx, PIN_STRING, out_value).await
        }
    }
}

// Template Array Agent
#[askit_agent(
    title = "Template Array",
    category = CATEGORY,
    inputs = [PIN_VALUE],
    outputs = [PIN_STRING],
    text_config(name = CONFIG_TEMPLATE, default = "{{value}}")
)]
struct TemplateArrayAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for TemplateArrayAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
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
            self.output(ctx, PIN_STRING, AgentValue::string(rendered_string))
                .await
        } else {
            let d = AgentValue::array(vector![value.clone()]);
            let rendered_string = reg.render_template(&template, &d).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to render template: {}", e))
            })?;
            let out_value = AgentValue::string(rendered_string);
            self.output(ctx, PIN_STRING, out_value).await
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
