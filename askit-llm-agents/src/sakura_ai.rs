#![cfg(feature = "sakura")]

use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfig, AgentConfigEntry, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AgentValue, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};

use ollama_rs::{
    generation::chat::{ChatMessage, request::ChatMessageRequest},
    models::ModelOptions,
};
use sakura_ai_rs::SakuraAI;
use tokio_stream::StreamExt;

use crate::message::{Message, MessageHistory};

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
            .get_global_config("sakura_ai_chat")
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
    history: Arc<Mutex<MessageHistory>>,
}

#[async_trait]
impl AsAgent for SakuraAIChatAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            manager: SakuraAIManager::new(),
            history: Arc::new(Mutex::new(MessageHistory::default())),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        let history_size = config.get_integer_or_default(CONFIG_HISTORY);
        if history_size != self.config()?.get_integer_or_default(CONFIG_HISTORY) {
            let mut history_guard = self.history.lock().unwrap();
            *history_guard = MessageHistory::new(history_guard.messages.clone(), history_size);
        }
        Ok(())
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

        let client = self.manager.get_client(self.askit())?;
        let mut request = ChatMessageRequest::new(
            config_model.to_string(),
            vec![ChatMessage::user(message.to_string())],
        );

        let config_options = self.config()?.get_string_or_default(CONFIG_OPTIONS);
        if !config_options.is_empty() && config_options != "{}" {
            if let Ok(options_json) = serde_json::from_str::<ModelOptions>(&config_options) {
                request = request.options(options_json);
            } else {
                return Err(AgentError::InvalidValue(
                    "Invalid JSON in options".to_string(),
                ));
            }
        }

        let history_size = self.config()?.get_integer_or_default(CONFIG_HISTORY);
        let use_stream = self.config()?.get_bool_or_default(CONFIG_STREAM);
        if use_stream {
            let mut stream = if history_size > 0 {
                client
                    .send_chat_messages_with_history_stream(self.history.clone(), request)
                    .await
            } else {
                client.send_chat_messages_stream(request).await
            }
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

            let mut content = String::new();
            while let Some(res) = stream.next().await {
                let res = res.map_err(|_| AgentError::IoError(format!("Ollama Stream Error")))?;

                content.push_str(&res.message.content);

                let message = Message::assistant(content.clone());
                self.try_output(ctx.clone(), PORT_MESSAGE, message.into())?;

                let out_response = AgentData::from_serialize(&res)?;
                self.try_output(ctx.clone(), PORT_RESPONSE, out_response)?;

                if res.done {
                    break;
                }
            }
            if history_size > 0 {
                let messages = self.history.lock().unwrap();
                self.try_output(ctx.clone(), PORT_HISTORY, messages.clone().into())?;
            }
        } else {
            let res = if history_size > 0 {
                let mut history = self.history.lock().unwrap().clone();
                let res = client
                    .send_chat_messages_with_history(&mut history, request)
                    .await;
                *self.history.lock().unwrap() = history;
                res
            } else {
                client.send_chat_messages(request).await
            }
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

            let message: Message = res.message.clone().into();
            self.try_output(ctx.clone(), PORT_MESSAGE, message.into())?;

            let out_response = AgentData::from_serialize(&res)?;
            self.try_output(ctx.clone(), PORT_RESPONSE, out_response)?;

            if history_size > 0 {
                let messages = self.history.lock().unwrap();
                self.try_output(ctx, PORT_HISTORY, messages.clone().into())?;
            }
        }

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "LLM";

static PORT_HISTORY: &str = "history";
static PORT_MESSAGE: &str = "message";
static PORT_RESPONSE: &str = "response";

static CONFIG_HISTORY: &str = "history";
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
        .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE, PORT_HISTORY])
        .with_global_config(vec![(
            CONFIG_SAKURA_AI_API_KEY.into(),
            AgentConfigEntry::new(AgentValue::string(""), "string").with_title("Sakura AI API Key"),
        )])
        .with_default_config(vec![
            (
                CONFIG_MODEL.into(),
                AgentConfigEntry::new(AgentValue::string(DEFAULT_CONFIG_MODEL), "string")
                    .with_title("Model"),
            ),
            (
                CONFIG_HISTORY.into(),
                AgentConfigEntry::new(AgentValue::integer(0), "integer").with_title("History Size"),
            ),
            (
                CONFIG_STREAM.into(),
                AgentConfigEntry::new(AgentValue::boolean(false), "boolean").with_title("Stream"),
            ),
            (
                CONFIG_OPTIONS.into(),
                AgentConfigEntry::new(AgentValue::string("{}"), "text").with_title("Options"),
            ),
        ]),
    );
}
