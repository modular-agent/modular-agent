use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfig, AgentConfigEntry, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AgentValue, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
        ChatCompletionRequestUserMessageArgs, ChatCompletionResponseMessage,
        CreateChatCompletionRequest, CreateChatCompletionRequestArgs, CreateCompletionRequest,
        CreateCompletionRequestArgs, Role,
    },
};

use crate::message::{Message, MessageHistory};

// Shared client management for OpenAI agents
struct OpenAIManager {
    client: Arc<Mutex<Option<Client<OpenAIConfig>>>>,
}

impl OpenAIManager {
    fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
        }
    }

    fn get_client(&self, askit: &ASKit) -> Result<Client<OpenAIConfig>, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        let mut new_client = Client::new();

        if let Some(api_key) = askit
            .get_global_config("openai_chat")
            .and_then(|cfg| cfg.get_string(CONFIG_OPENAI_API_KEY).ok())
            .filter(|key| !key.is_empty())
        {
            let config = OpenAIConfig::new().with_api_key(&api_key);
            new_client = Client::with_config(config);
        }

        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

// OpenAI Completion Agent
pub struct OpenAICompletionAgent {
    data: AsAgentData,
    manager: OpenAIManager,
}

#[async_trait]
impl AsAgent for OpenAICompletionAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            manager: OpenAIManager::new(),
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

        let mut request = CreateCompletionRequestArgs::default()
            .model(config_model)
            .prompt(message)
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        let config_options = self.config()?.get_string_or_default(CONFIG_OPTIONS);
        if !config_options.is_empty() && config_options != "{}" {
            // Merge options into request
            let options_json = serde_json::from_str::<serde_json::Value>(&config_options)
                .map_err(|e| AgentError::InvalidValue(format!("Invalid JSON in options: {}", e)))?;

            let mut request_json = serde_json::to_value(&request)
                .map_err(|e| AgentError::InvalidValue(format!("Serialization error: {}", e)))?;

            if let (Some(request_obj), Some(options_obj)) =
                (request_json.as_object_mut(), options_json.as_object())
            {
                for (key, value) in options_obj {
                    request_obj.insert(key.clone(), value.clone());
                }
            }
            request = serde_json::from_value::<CreateCompletionRequest>(request_json)
                .map_err(|e| AgentError::InvalidValue(format!("Deserialization error: {}", e)))?;
        }

        let client = self.manager.get_client(self.askit())?;
        let res = client
            .completions()
            .create(request)
            .await
            .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

        let message = Message::assistant(res.choices[0].text.clone());
        self.try_output(ctx.clone(), PORT_MESSAGE, message.into())?;

        let out_response = AgentData::from_serialize(&res)?;
        self.try_output(ctx, PORT_RESPONSE, out_response)?;

        Ok(())
    }
}

// OpenAI Chat Agent
pub struct OpenAIChatAgent {
    data: AsAgentData,
    manager: OpenAIManager,
    history: MessageHistory,
}

#[async_trait]
impl AsAgent for OpenAIChatAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            manager: OpenAIManager::new(),
            history: MessageHistory(vec![]),
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

        let enable_history = self.config()?.get_bool_or_default(CONFIG_HISTORY);
        let messages = if enable_history {
            self.history.push(Message::user(message.to_string()));
            self.history.0.clone()
        } else {
            vec![Message::user(message.to_string())]
        }
        .into_iter()
        .map(|m| m.into())
        .collect::<Vec<ChatCompletionRequestMessage>>();

        let mut request = CreateChatCompletionRequestArgs::default()
            .model(config_model)
            .messages(messages)
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        let config_options = self.config()?.get_string_or_default(CONFIG_OPTIONS);
        if !config_options.is_empty() && config_options != "{}" {
            // Merge options into request
            let options_json = serde_json::from_str::<serde_json::Value>(&config_options)
                .map_err(|e| AgentError::InvalidValue(format!("Invalid JSON in options: {}", e)))?;

            let mut request_json = serde_json::to_value(&request)
                .map_err(|e| AgentError::InvalidValue(format!("Serialization error: {}", e)))?;

            if let (Some(request_obj), Some(options_obj)) =
                (request_json.as_object_mut(), options_json.as_object())
            {
                for (key, value) in options_obj {
                    request_obj.insert(key.clone(), value.clone());
                }
            }
            request = serde_json::from_value::<CreateChatCompletionRequest>(request_json)
                .map_err(|e| AgentError::InvalidValue(format!("Deserialization error: {}", e)))?;
        }

        let client = self.manager.get_client(self.askit())?;
        let res = client
            .chat()
            .create(request)
            .await
            .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

        let res_message: Message = res.choices[0].message.clone().into();
        self.try_output(ctx.clone(), PORT_MESSAGE, res_message.clone().into())?;

        let out_response = AgentData::from_serialize(&res)?;
        self.try_output(ctx.clone(), PORT_RESPONSE, out_response)?;

        let enable_history = self.config()?.get_bool_or_default(CONFIG_HISTORY);
        if enable_history {
            self.history.push(res_message.into());
            self.try_output(ctx, PORT_HISTORY, self.history.clone().into())?;
        }

        Ok(())
    }
}

// // OpenAI Embeddings Agent
// pub struct OpenAIEmbeddingsAgent {
//     data: AsAgentData,
//     manager: OpenAIManager,
// }

// #[async_trait]
// impl AsAgent for OpenAIEmbeddingsAgent {
//     fn new(
//         askit: ASKit,
//         id: String,
//         def_name: String,
//         config: Option<AgentConfig>,
//     ) -> Result<Self, AgentError> {
//         Ok(Self {
//             data: AsAgentData::new(askit, id, def_name, config),
//             manager: OpenAIManager::new(),
//         })
//     }

//     fn data(&self) -> &AsAgentData {
//         &self.data
//     }

//     fn mut_data(&mut self) -> &mut AsAgentData {
//         &mut self.data
//     }

//     async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
//         let config_model = &self.config()?.get_string_or_default(CONFIG_MODEL);
//         if config_model.is_empty() {
//             return Ok(());
//         }

//         let input = data.as_str().unwrap_or(""); // TODO: other types
//         if input.is_empty() {
//             return Ok(());
//         }

//         let client = self.manager.get_client(self.askit())?;
//         let mut request = GenerateEmbeddingsRequest::new(config_model.to_string(), input.into());

//         let config_options = self.config()?.get_string_or_default(CONFIG_OPTIONS);
//         if !config_options.is_empty() && config_options != "{}" {
//             if let Ok(options_json) = serde_json::from_str::<ModelOptions>(&config_options) {
//                 request = request.options(options_json);
//             } else {
//                 return Err(AgentError::InvalidValue(
//                     "Invalid JSON in options".to_string(),
//                 ));
//             }
//         }

//         let res = client
//             .generate_embeddings(request)
//             .await
//             .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

//         let embeddings = AgentData::from_serialize(&res.embeddings)?;
//         self.try_output(ctx.clone(), PORT_EMBEDDINGS, embeddings)?;

//         Ok(())
//     }
// }

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "LLM";

static PORT_EMBEDDINGS: &str = "embeddings";
static PORT_HISTORY: &str = "history";
static PORT_INPUT: &str = "input";
static PORT_MESSAGE: &str = "message";
static PORT_RESPONSE: &str = "response";

static CONFIG_MODEL: &str = "model";
static CONFIG_OPENAI_API_KEY: &str = "openai_api_key";
static CONFIG_OPTIONS: &str = "options";
static CONFIG_HISTORY: &str = "history";

const DEFAULT_CONFIG_MODEL: &str = "gpt-5-nano";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "openai_completion",
            Some(new_agent_boxed::<OpenAICompletionAgent>),
        )
        // .use_native_thread()
        .with_title("OpenAI Completion")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGE])
        .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE])
        .with_default_config(vec![
            (
                CONFIG_MODEL.into(),
                AgentConfigEntry::new(AgentValue::string("gpt-3.5-turbo-instruct"), "string")
                    .with_title("Model"),
            ),
            (
                CONFIG_OPTIONS.into(),
                AgentConfigEntry::new(AgentValue::string("{}"), "text").with_title("Options"),
            ),
        ]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "openai_chat",
            Some(new_agent_boxed::<OpenAIChatAgent>),
        )
        // .use_native_thread()
        .with_title("OpenAI Chat")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGE])
        .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE, PORT_HISTORY])
        .with_global_config(vec![(
            CONFIG_OPENAI_API_KEY.into(),
            AgentConfigEntry::new(AgentValue::string(""), "string").with_title("OpenAI API Key"),
        )])
        .with_default_config(vec![
            (
                CONFIG_MODEL.into(),
                AgentConfigEntry::new(AgentValue::string(DEFAULT_CONFIG_MODEL), "string")
                    .with_title("Model"),
            ),
            (
                CONFIG_HISTORY.into(),
                AgentConfigEntry::new(AgentValue::boolean(false), "boolean")
                    .with_title("Enable History"),
            ),
            (
                CONFIG_OPTIONS.into(),
                AgentConfigEntry::new(AgentValue::string("{}"), "text").with_title("Options"),
            ),
        ]),
    );

    // askit.register_agent(
    //     AgentDefinition::new(
    //         AGENT_KIND,
    //         "ollama_embeddings",
    //         Some(new_agent_boxed::<OpenAIEmbeddingsAgent>),
    //     )
    //     // .use_native_thread()
    //     .with_title("OpenAI Embeddings")
    //     .with_category(CATEGORY)
    //     .with_inputs(vec![PORT_INPUT])
    //     .with_outputs(vec![PORT_EMBEDDINGS])
    //     .with_default_config(vec![
    //         (
    //             CONFIG_MODEL.into(),
    //             AgentConfigEntry::new(AgentValue::string(DEFAULT_CONFIG_MODEL), "string")
    //                 .with_title("Model"),
    //         ),
    //         (
    //             CONFIG_OPTIONS.into(),
    //             AgentConfigEntry::new(AgentValue::string("{}"), "text").with_title("Options"),
    //         ),
    //     ]),
    // );
}

impl From<ChatCompletionResponseMessage> for Message {
    fn from(msg: ChatCompletionResponseMessage) -> Self {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
            Role::Function => "function",
        };
        Self {
            role: role.to_string(),
            content: msg.content.unwrap_or_default(),
        }
    }
}

impl From<Message> for ChatCompletionRequestMessage {
    fn from(msg: Message) -> Self {
        match msg.role.as_str() {
            "system" => ChatCompletionRequestSystemMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .unwrap()
                .into(),
            "user" => ChatCompletionRequestUserMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .unwrap()
                .into(),
            "assistant" => ChatCompletionRequestAssistantMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .unwrap()
                .into(),
            "tool" => ChatCompletionRequestToolMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .unwrap()
                .into(),
            _ => ChatCompletionRequestUserMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .unwrap()
                .into(),
        }
    }
}
