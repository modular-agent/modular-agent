use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use html_to_markdown_rs::{ConversionOptions, PreprocessingPreset, convert};

static CATEGORY: &str = "Web";

static PORT_HTML: &str = "html";
static PORT_MARKDOWN: &str = "markdown";

/// Convert HTML to Markdown
#[askit_agent(
    title = "HTML to Markdown",
    category = CATEGORY,
    inputs = [PORT_HTML],
    outputs = [PORT_MARKDOWN],
)]
struct HtmlToMarkdownAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for HtmlToMarkdownAgent {
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
        if value.is_array() {
            let mut arr = vec![];
            for item in value.as_array().unwrap() {
                let html = item.as_str().ok_or_else(|| {
                    AgentError::InvalidValue(
                        "Input array items for 'html' must be strings".to_string(),
                    )
                })?;
                let markdown = html2markdown(html)?;
                arr.push(AgentValue::string(markdown));
            }
            return self.try_output(ctx, PORT_MARKDOWN, AgentValue::array(arr));
        }

        let html = value.as_str().ok_or_else(|| {
            AgentError::InvalidValue("Input value for 'html' must be a string".to_string())
        })?;
        let markdown = html2markdown(html)?;
        self.try_output(ctx, PORT_MARKDOWN, AgentValue::string(markdown))
    }
}

fn html2markdown(html: &str) -> Result<String, AgentError> {
    let mut options = ConversionOptions::default();
    options.preprocessing.enabled = true;
    options.preprocessing.preset = PreprocessingPreset::Aggressive;
    options.preprocessing.remove_navigation = true;
    options.preprocessing.remove_forms = true;

    let markdown = convert(html, Some(options)).map_err(|e| {
        AgentError::InvalidValue(format!("Failed to convert HTML to Markdown: {}", e))
    })?;
    Ok(markdown)
}
