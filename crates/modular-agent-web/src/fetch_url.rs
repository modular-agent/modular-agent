use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use reqwest::Client;

static CATEGORY: &str = "Web";

static PORT_URL: &str = "url";
static PORT_TEXT: &str = "text";

/// Fetch text content from a given URL
#[askit_agent(
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

        self.try_output(ctx, PORT_TEXT, AgentValue::string(text))
    }
}
