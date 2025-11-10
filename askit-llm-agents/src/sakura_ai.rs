#![cfg(feature = "sakura")]

use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigEntry, AgentConfigs, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};

use ollama_rs::{generation::chat::request::ChatMessageRequest, models::ModelOptions};
use sakura_ai_rs::SakuraAI;
use tokio_stream::StreamExt;

use crate::message::Message;

// Shared client management for SakuraAI agents
struct SakuraAIManager {
    client: Arc<Mutex<Option<SakuraAI>>>,
}

impl SakuraAIManager {
    fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    fn get_client(&self, askit: &ASKit) -> Result<SakuraAI, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        let mut new_client = SakuraAI::default();

        if let Some(api_key) = askit
            .get_global_configs("sakura_ai_chat")
            .and_then(|cfg| cfg.get_string(CONFIG_SAKURA_AI_API_KEY).ok())
            .filter(|key| !key.is_empty())
        {
            new_client = new_client.with_api_key(&api_key);
        }

        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

// SakuraAI Chat Agent
pub struct SakuraAIChatAgent {
    data: AsAgentData,
    manager: SakuraAIManager,
}

#[async_trait]
impl AsAgent for SakuraAIChatAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            manager: SakuraAIManager::new(),
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
        data: AgentData,
    ) -> Result<(), AgentError> {
        let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let mut messages: Vec<Message> = Vec::new();

        if data.is_string() {
            let message = data.as_str().unwrap_or("");
            if message.is_empty() {
                return Ok(());
            }
            messages.push(Message::user(message.to_string()));
        } else if data.is_object() {
            let obj = data.as_object().unwrap();
            if obj.contains_key("role") && obj.contains_key("content") {
                let msg: Message = data.clone().try_into()?;
                messages.push(msg);
            } else {
                if obj.contains_key("history") {
                    let history_data = obj.get("history").unwrap();
                    if history_data.is_array() {
                        let arr = history_data.as_array().unwrap();
                        for item in arr {
                            let msg: Message = item.clone().try_into()?;
                            messages.push(msg);
                        }
                    }
                }
                if obj.contains_key("message") {
                    let msg_data = obj.get("message").unwrap();
                    let msg: Message = msg_data.clone().try_into()?;
                    messages.push(msg);
                }
            }
        }

        if messages.is_empty() {
            return Ok(());
        }

        let client = self.manager.get_client(self.askit())?;
        let mut request = ChatMessageRequest::new(
            config_model.to_string(),
            messages.into_iter().map(|m| m.into()).collect(),
        );

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
        if !config_options.is_empty() && config_options != "{}" {
            if let Ok(options_json) = serde_json::from_str::<ModelOptions>(&config_options) {
                request = request.options(options_json);
            } else {
                return Err(AgentError::InvalidValue(
                    "Invalid JSON in options".to_string(),
                ));
            }
        }

        let id = uuid::Uuid::new_v4().to_string();
        let use_stream = self.configs()?.get_bool_or_default(CONFIG_STREAM);
        if use_stream {
            let mut stream = client
                .send_chat_messages_stream(request)
                .await
                .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

            let mut content = String::new();
            while let Some(res) = stream.next().await {
                let res = res.map_err(|_| AgentError::IoError(format!("Ollama Stream Error")))?;

                content.push_str(&res.message.content);

                let mut message = Message::assistant(content.clone());
                message.id = Some(id.clone());
                self.try_output(ctx.clone(), PORT_MESSAGE, message.into())?;

                let out_response = AgentData::from_serialize(&res)?;
                self.try_output(ctx.clone(), PORT_RESPONSE, out_response)?;

                if res.done {
                    break;
                }
            }
        } else {
            let res = client
                .send_chat_messages(request)
                .await
                .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

            let mut message = Message::assistant(res.message.content.clone());
            message.id = Some(id.clone());
            self.try_output(ctx.clone(), PORT_MESSAGE, message.into())?;

            let out_response = AgentData::from_serialize(&res)?;
            self.try_output(ctx.clone(), PORT_RESPONSE, out_response)?;
        }

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "LLM";

static PORT_MESSAGE: &str = "message";
static PORT_RESPONSE: &str = "response";

static CONFIG_SAKURA_AI_API_KEY: &str = "sakura_ai_api_key";
static CONFIG_STREAM: &str = "stream";
static CONFIG_MODEL: &str = "model";
static CONFIG_OPTIONS: &str = "options";

const DEFAULT_CONFIG_MODEL: &str = "gpt-oss-120b";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "sakura_ai_chat",
            Some(new_agent_boxed::<SakuraAIChatAgent>),
        )
        // .use_native_thread()
        .with_title("SakuraAI Chat")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGE])
        .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE])
        .with_global_configs(vec![(
            CONFIG_SAKURA_AI_API_KEY,
            AgentConfigEntry::new("", "password").with_title("Sakura AI API Key"),
        )])
        .with_default_configs(vec![
            (
                CONFIG_MODEL,
                AgentConfigEntry::new(DEFAULT_CONFIG_MODEL, "string").with_title("Model"),
            ),
            (
                CONFIG_STREAM,
                AgentConfigEntry::new(false, "boolean").with_title("Stream"),
            ),
            (
                CONFIG_OPTIONS,
                AgentConfigEntry::new("{}", "text").with_title("Options"),
            ),
        ]),
    );
}
