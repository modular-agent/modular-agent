#![cfg(feature = "sakura")]

use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};

use ollama_rs::{generation::chat::request::ChatMessageRequest, models::ModelOptions};
use sakura_ai_rs::SakuraAI;
use tokio_stream::StreamExt;

use crate::message_lib::Message;

static CATEGORY: &str = "LLM/Sakura";

static PIN_MESSAGE: &str = "message";
static PIN_RESPONSE: &str = "response";

static CONFIG_SAKURA_AI_API_KEY: &str = "sakura_ai_api_key";
static CONFIG_STREAM: &str = "stream";
static CONFIG_MODEL: &str = "model";
static CONFIG_OPTIONS: &str = "options";

const DEFAULT_CONFIG_MODEL: &str = "gpt-oss-120b";

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
#[askit_agent(
    title="SakuraAI Chat",
    category=CATEGORY,
    inputs=[PIN_MESSAGE],
    outputs=[PIN_MESSAGE, PIN_RESPONSE],
    string_config(name=CONFIG_MODEL, default=DEFAULT_CONFIG_MODEL),
    boolean_config(name=CONFIG_STREAM, title="Stream"),
    text_config(name=CONFIG_OPTIONS, default="{}"),
    string_global_config(name=CONFIG_SAKURA_AI_API_KEY, title="Sakura AI API Key"),
)]
pub struct SakuraAIChatAgent {
    data: AgentData,
    manager: SakuraAIManager,
}

#[async_trait]
impl AsAgent for SakuraAIChatAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: SakuraAIManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let mut messages: Vec<Message> = Vec::new();

        if value.is_string() {
            let message = value.as_str().unwrap_or("");
            if message.is_empty() {
                return Ok(());
            }
            messages.push(Message::user(message.to_string()));
        } else if value.is_object() {
            let obj = value.as_object().unwrap();
            if obj.contains_key("role") && obj.contains_key("content") {
                let msg: Message = value.clone().try_into()?;
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
                self.try_output(ctx.clone(), PIN_MESSAGE, message.into())?;

                let out_response = AgentValue::from_serialize(&res)?;
                self.try_output(ctx.clone(), PIN_RESPONSE, out_response)?;

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
            self.try_output(ctx.clone(), PIN_MESSAGE, message.into())?;

            let out_response = AgentValue::from_serialize(&res)?;
            self.try_output(ctx.clone(), PIN_RESPONSE, out_response)?;
        }

        Ok(())
    }
}
