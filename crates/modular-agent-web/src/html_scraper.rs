use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use scraper::{Html, Selector};

static CATEGORY: &str = "Web";

static PORT_HTML: &str = "html";

/// Extract text content from HTML by CSS selector
#[askit_agent(
    title = "HTML Scraper",
    category = CATEGORY,
    inputs = [PORT_HTML],
    outputs = [PORT_HTML],
    string_config(name = "selector"),
)]
struct HtmlScraperAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for HtmlScraperAgent {
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
        let selector_str = self.configs()?.get_string_or_default("selector");
        if selector_str.is_empty() {
            return Ok(());
        }
        let selector = Selector::parse(&selector_str).map_err(|e| {
            AgentError::InvalidValue(format!("Invalid CSS selector '{}': {}", selector_str, e))
        })?;

        if value.is_array() {
            let mut arr = Vec::new();
            for item in value.as_array().unwrap() {
                let html = item.as_str().ok_or_else(|| {
                    AgentError::InvalidValue(
                        "Input array items for 'html' must be strings".to_string(),
                    )
                })?;
                let fragment = Html::parse_fragment(html);
                let selected: Vec<AgentValue> = fragment
                    .select(&selector)
                    .map(|elem| AgentValue::string(elem.html()))
                    .collect();
                arr.extend(selected);
            }
            return self.try_output(ctx, PORT_HTML, AgentValue::array(arr.into()));
        }

        let html = value.as_str().ok_or_else(|| {
            AgentError::InvalidValue("Input value for 'html' must be a string".to_string())
        })?;

        let document = Html::parse_document(html);
        let selected: Vec<AgentValue> = document
            .select(&selector)
            .map(|elem| AgentValue::string(elem.html()))
            .collect();
        self.try_output(ctx, PORT_HTML, AgentValue::array(selected.into()))
    }
}
