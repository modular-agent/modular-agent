use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfig, AgentConfigEntry, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AgentValue, AgentValueMap, AsAgent, AsAgentData, async_trait,
    new_agent_boxed,
};
use ollama_rs::Ollama;
use ollama_rs::generation::completion::request::GenerationRequest;

// Ollama Agent
pub struct OllamaAgent {
    data: AsAgentData,
    client: Arc<Mutex<Option<Ollama>>>,
}

impl OllamaAgent {
    fn get_ollama_url(&self) -> String {
        if let Some(ollama_url) = self
            .get_global_config()
            .and_then(|cfg| cfg.get_string(CONFIG_OLLAMA_URL).ok())
        {
            if !ollama_url.is_empty() {
                return ollama_url;
            }
        }
        if let Ok(ollama_api_base_url) = std::env::var("OLLAMA_API_BASE_URL") {
            return ollama_api_base_url;
        } else if let Ok(ollama_host) = std::env::var("OLLAMA_HOST") {
            return format!("http://{}:11434", ollama_host);
        }
        DEFAULT_OLLAMA_URL.to_string()
    }

    fn get_client(&mut self) -> Result<Ollama, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        let api_base_url = self.get_ollama_url();
        let (api_base, port) = api_base_url
            .rsplit_once(':')
            .unwrap_or(("http://localhost", "11434"));
        let port = port.parse::<u16>().unwrap_or(11434);
        let new_client = Ollama::new(api_base, port);
        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

#[async_trait]
impl AsAgent for OllamaAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            client: Arc::new(Mutex::new(None)),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let config_model = &self.config()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let message = data.as_str().unwrap_or("");
        if message.is_empty() {
            return Ok(());
        }

        let client = self.get_client()?;
        let res = client
            .generate(GenerationRequest::new(config_model.to_string(), message))
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

        let out_message = AgentData::new_custom_object(
            "message",
            AgentValueMap::from([
                ("role".to_string(), AgentValue::new_string("assistant")),
                (
                    "content".to_string(),
                    AgentValue::new_string(res.response.clone()),
                ),
            ]),
        );
        self.try_output(ctx.clone(), PORT_MESSAGE, out_message)?;

        let res_json = serde_json::to_value(&res)
            .map_err(|e| AgentError::InvalidValue(format!("serde_json error: {}", e)))?;
        let out_response = AgentData::from_json_value(res_json)?;
        self.try_output(ctx, PORT_RESPONSE, out_response)?;

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "LLM";

static PORT_MESSAGE: &str = "message";
static PORT_RESPONSE: &str = "response";

static CONFIG_MODEL: &str = "model";
static CONFIG_OLLAMA_URL: &str = "ollama_url";

const DEFAULT_CONFIG_MODEL: &str = "gemma3:4b";
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "ollama", Some(new_agent_boxed::<OllamaAgent>))
            // .use_native_thread()
            .with_title("Ollama")
            .with_category(CATEGORY)
            .with_inputs(vec![PORT_MESSAGE])
            .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE])
            .with_global_config(vec![(
                CONFIG_OLLAMA_URL.into(),
                AgentConfigEntry::new(AgentValue::new_string(DEFAULT_OLLAMA_URL), "string")
                    .with_title("Ollama URL"),
            )])
            .with_default_config(vec![(
                CONFIG_MODEL.into(),
                AgentConfigEntry::new(AgentValue::new_string(DEFAULT_CONFIG_MODEL), "string")
                    .with_title("Chat Model"),
            )]),
    );
}
