use modular_agent_kit::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent, MAK,
    async_trait, modular_agent,
};
use scraper::{Html, Selector};

static CATEGORY: &str = "Web";

static PORT_HTML: &str = "html";

/// Extract text content from HTML by CSS selector
#[modular_agent(
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
    fn new(mak: MAK, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
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
            return self
                .output(ctx, PORT_HTML, AgentValue::array(arr.into()))
                .await;
        }

        let html = value.as_str().ok_or_else(|| {
            AgentError::InvalidValue("Input value for 'html' must be a string".to_string())
        })?;

        let selected = {
            let document = Html::parse_document(html);
            let selected: Vec<AgentValue> = document
                .select(&selector)
                .map(|elem| AgentValue::string(elem.html()))
                .collect();
            selected.clone()
        };
        self.output(ctx, PORT_HTML, AgentValue::array(selected.into()))
            .await
    }
}
