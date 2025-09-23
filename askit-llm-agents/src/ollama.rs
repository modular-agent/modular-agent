use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfig, AgentConfigEntry, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AgentValue, AgentValueMap, AsAgent, AsAgentData, async_trait,
    new_agent_boxed,
};
use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::completion::request::GenerationRequest;

// Shared client management for Ollama agents
struct OllamaManager {
    client: Arc<Mutex<Option<Ollama>>>,
}

impl OllamaManager {
    fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    fn get_ollama_url(global_config: Option<AgentConfig>) -> String {
        if let Some(ollama_url) =
            global_config.and_then(|cfg| cfg.get_string(CONFIG_OLLAMA_URL).ok())
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

    fn get_client(&self, askit: &ASKit) -> Result<Ollama, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        let global_config = askit.get_global_config("ollama_completion");
        let api_base_url = Self::get_ollama_url(global_config);
        let new_client = Ollama::try_new(api_base_url)
            .map_err(|e| AgentError::IoError(format!("Ollama Client Error: {}", e)))?;
        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

// Ollama Completion Agent
pub struct OllamaCompletionAgent {
    data: AsAgentData,
    manager: OllamaManager,
}

#[async_trait]
impl AsAgent for OllamaCompletionAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            manager: OllamaManager::new(),
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

        let mut request = GenerationRequest::new(config_model.to_string(), message);

        let config_system = self.config()?.get_string_or_default(CONFIG_SYSTEM);
        if !config_system.is_empty() {
            request = request.system(config_system);
        }

        let client = self.manager.get_client(self.askit())?;
        let res = client
            .generate(request)
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

// Ollama Chat Agent
pub struct OllamaChatAgent {
    data: AsAgentData,
    manager: OllamaManager,
    history: Vec<ChatMessage>,
}

#[async_trait]
impl AsAgent for OllamaChatAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            manager: OllamaManager::new(),
            history: vec![],
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

        let mut client = self.manager.get_client(self.askit())?;
        let res = client
            .send_chat_messages_with_history(
                &mut self.history,
                ChatMessageRequest::new(
                    config_model.to_string(),
                    vec![ChatMessage::user(message.to_string())],
                ),
            )
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

        let out_message = AgentData::new_custom_object(
            "message",
            AgentValueMap::from([
                ("role".to_string(), AgentValue::new_string("assistant")),
                (
                    "content".to_string(),
                    AgentValue::new_string(res.message.content.clone()),
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
static CONFIG_SYSTEM: &str = "system";

const DEFAULT_CONFIG_MODEL: &str = "gemma3:4b";
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "ollama_completion",
            Some(new_agent_boxed::<OllamaCompletionAgent>),
        )
        // .use_native_thread()
        .with_title("Ollama Completion")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGE])
        .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE])
        .with_global_config(vec![(
            CONFIG_OLLAMA_URL.into(),
            AgentConfigEntry::new(AgentValue::new_string(DEFAULT_OLLAMA_URL), "string")
                .with_title("Ollama URL"),
        )])
        .with_default_config(vec![
            (
                CONFIG_MODEL.into(),
                AgentConfigEntry::new(AgentValue::new_string(DEFAULT_CONFIG_MODEL), "string")
                    .with_title("Model"),
            ),
            (
                CONFIG_SYSTEM.into(),
                AgentConfigEntry::new(AgentValue::new_string(""), "text").with_title("System"),
            ),
        ]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "ollama_chat",
            Some(new_agent_boxed::<OllamaChatAgent>),
        )
        // .use_native_thread()
        .with_title("Ollama Chat")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGE])
        .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE])
        .with_default_config(vec![(
            CONFIG_MODEL.into(),
            AgentConfigEntry::new(AgentValue::new_string(DEFAULT_CONFIG_MODEL), "string")
                .with_title("Model"),
        )]),
    );
}
