use modular_agent_core::{
    AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent, ModularAgent,
    async_trait, modular_agent,
};
use reqwest::Client;

static CATEGORY: &str = "Web";

static PORT_URL: &str = "url";
static PORT_TEXT: &str = "text";

/// Fetch text content from a given URL
#[modular_agent(
    title = "Fetch URL",
    category = CATEGORY,
    inputs = [PORT_URL],
    outputs = [PORT_TEXT],
)]
struct FetchUrlAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for FetchUrlAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let url = value.as_str().ok_or_else(|| {
            AgentError::InvalidValue("Input value for 'url' must be a string".to_string())
        })?;
        // TODO: validate URL

        let client = Client::new();
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| AgentError::IoError(format!("HTTP Request Error: {}", e)))?;
        let text = response
            .text()
            .await
            .map_err(|e| AgentError::IoError(format!("HTTP Response Error: {}", e)))?;

        self.output(ctx, PORT_TEXT, AgentValue::string(text)).await
    }
}
