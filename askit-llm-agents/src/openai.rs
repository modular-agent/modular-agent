#![cfg(feature = "openai")]

use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentError, AgentOutput,
    AsAgent, AsAgentData, async_trait, new_agent_boxed,
};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
        ChatCompletionRequestUserMessageArgs, ChatCompletionResponseMessage,
        CreateChatCompletionRequest, CreateChatCompletionRequestArgs, CreateCompletionRequest,
        CreateCompletionRequestArgs, CreateEmbeddingRequest, CreateEmbeddingRequestArgs, Role,
        responses::{self, CreateResponse, CreateResponseArgs, OutputContent, OutputMessage},
    },
};
use futures::StreamExt;

use crate::message::Message;

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
            .get_global_configs("openai_chat")
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
        config: Option<AgentConfigs>,
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

        let mut messages;
        {
            if data.is_array() {
                let arr = data.as_array().unwrap();
                messages = Vec::new();
                for item in arr {
                    let msg: Message = item.clone().try_into()?;
                    messages.push(msg);
                }
                // Check if the last message is user
                if let Some(last_msg) = messages.last() {
                    if last_msg.role != "user" {
                        return Ok(());
                    }
                }
            } else {
                let message = data.as_str().unwrap_or("");
                if message.is_empty() {
                    return Ok(());
                }
                messages = vec![Message::user(message.to_string())];
            }
        }

        let mut request = CreateCompletionRequestArgs::default()
            .model(config_model)
            .prompt(
                messages
                    .iter()
                    .map(|m| m.content.clone())
                    .collect::<Vec<String>>(),
            )
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
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
}

#[async_trait]
impl AsAgent for OpenAIChatAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
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

        let messages = messages
            .into_iter()
            .map(|m| m.into())
            .collect::<Vec<ChatCompletionRequestMessage>>();

        let use_stream = self.configs()?.get_bool_or_default(CONFIG_STREAM);

        let mut request = CreateChatCompletionRequestArgs::default()
            .model(config_model)
            .messages(messages)
            .stream(use_stream)
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
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

        if use_stream {
            let mut stream = client
                .chat()
                .create_stream(request)
                .await
                .map_err(|e| AgentError::IoError(format!("OpenAI Stream Error: {}", e)))?;
            let mut content = String::new();
            while let Some(res) = stream.next().await {
                let res = res.map_err(|_| AgentError::IoError(format!("OpenAI Stream Error")))?;
                res.choices.iter().for_each(|c| {
                    if let Some(ref delta_content) = c.delta.content {
                        content.push_str(delta_content);
                    }
                });

                let mut message = Message::assistant(content.clone());
                message.id = Some(res.id.clone());
                self.try_output(ctx.clone(), PORT_MESSAGE, message.into())?;

                let out_response = AgentData::from_serialize(&res)?;
                self.try_output(ctx.clone(), PORT_RESPONSE, out_response)?;
            }
        } else {
            let res = client
                .chat()
                .create(request)
                .await
                .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

            let mut content = String::new();
            res.choices.iter().for_each(|c| {
                if let Some(ref c) = c.message.content {
                    content.push_str(c);
                }
            });

            let mut res_message = Message::assistant(content);
            res_message.id = Some(res.id.clone());
            self.try_output(ctx.clone(), PORT_MESSAGE, res_message.clone().into())?;

            let out_response = AgentData::from_serialize(&res)?;
            self.try_output(ctx.clone(), PORT_RESPONSE, out_response)?;
        }

        Ok(())
    }
}

// OpenAI Embeddings Agent
pub struct OpenAIEmbeddingsAgent {
    data: AsAgentData,
    manager: OpenAIManager,
}

#[async_trait]
impl AsAgent for OpenAIEmbeddingsAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
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

        let input = data.as_str().unwrap_or(""); // TODO: other types
        if input.is_empty() {
            return Ok(());
        }

        let client = self.manager.get_client(self.askit())?;
        let mut request = CreateEmbeddingRequestArgs::default()
            .model(config_model.to_string())
            .input(vec![input])
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
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
            request = serde_json::from_value::<CreateEmbeddingRequest>(request_json)
                .map_err(|e| AgentError::InvalidValue(format!("Deserialization error: {}", e)))?;
        }

        let res = client
            .embeddings()
            .create(request)
            .await
            .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

        let data = AgentData::from_serialize(&res.data)?;
        self.try_output(ctx.clone(), PORT_EMBEDDINGS, data)?;

        Ok(())
    }
}

// OpenAI Responses Agent
// https://platform.openai.com/docs/api-reference/responses
pub struct OpenAIResponsesAgent {
    data: AsAgentData,
    manager: OpenAIManager,
}

#[async_trait]
impl AsAgent for OpenAIResponsesAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
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

        let use_stream = self.configs()?.get_bool_or_default(CONFIG_STREAM);

        let mut request = CreateResponseArgs::default()
            .model(config_model)
            .input(responses::Input::Items(
                messages
                    .iter()
                    .map(|m| m.into())
                    .collect::<Vec<responses::InputItem>>(),
            ))
            .stream(use_stream)
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
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
            request = serde_json::from_value::<CreateResponse>(request_json)
                .map_err(|e| AgentError::InvalidValue(format!("Deserialization error: {}", e)))?;
        }

        let client = self.manager.get_client(self.askit())?;

        if use_stream {
            let mut stream = client
                .responses()
                .create_stream(request)
                .await
                .map_err(|e| AgentError::IoError(format!("OpenAI Stream Error: {}", e)))?;
            let mut content = String::new();
            let mut id = None;
            while let Some(res) = stream.next().await {
                let res_event =
                    res.map_err(|e| AgentError::IoError(format!("OpenAI Stream Error: {}", e)))?;
                match &res_event {
                    responses::ResponseEvent::ResponseOutputTextDelta(delta) => {
                        id = Some(delta.item_id.clone());
                        content.push_str(&delta.delta);
                    }
                    responses::ResponseEvent::ResponseCompleted(_) => {
                        let out_response = AgentData::from_serialize(&res_event)?;
                        self.try_output(ctx.clone(), PORT_RESPONSE, out_response)?;
                        break;
                    }
                    _ => {}
                }

                let mut message = Message::assistant(content.clone());
                message.id = id.clone();
                self.try_output(ctx.clone(), PORT_MESSAGE, message.into())?;

                let out_response = AgentData::from_serialize(&res_event)?;
                self.try_output(ctx.clone(), PORT_RESPONSE, out_response)?;
            }
        } else {
            let res = client
                .responses()
                .create(request)
                .await
                .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

            let mut res_message: Message = Message::assistant(get_output_text(&res)); // TODO: better conversion
            res_message.id = Some(res.id.clone());
            self.try_output(ctx.clone(), PORT_MESSAGE, res_message.clone().into())?;

            let out_response = AgentData::from_serialize(&res)?;
            self.try_output(ctx.clone(), PORT_RESPONSE, out_response)?;
        }

        Ok(())
    }
}

fn get_output_text(response: &responses::Response) -> String {
    let mut output_text = String::new();
    response.output.iter().for_each(|msg| {
        if let responses::OutputContent::Message(m) = msg {
            m.content.iter().for_each(|c| {
                if let responses::Content::OutputText(t) = c {
                    output_text.push_str(&t.text);
                }
            });
        }
    });
    output_text
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
        Message::new(role.to_string(), msg.content.unwrap_or_default())
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

impl From<&Message> for responses::InputItem {
    fn from(msg: &Message) -> Self {
        responses::InputItem::Message(responses::InputMessage {
            kind: responses::InputMessageType::Message,
            role: match msg.role.as_str() {
                "system" => responses::Role::System,
                "user" => responses::Role::User,
                "assistant" => responses::Role::Assistant,
                "developer" => responses::Role::Developer,
                _ => responses::Role::Developer,
            },
            content: responses::InputContent::TextInput(msg.content.clone()),
        })
    }
}

impl From<OutputContent> for Message {
    fn from(content: OutputContent) -> Self {
        match content {
            OutputContent::Message(msg) => msg.into(),
            _ => Message::new("unknown".to_string(), "".to_string()),
        }
    }
}

impl From<OutputMessage> for Message {
    fn from(msg: OutputMessage) -> Self {
        let role = match msg.role {
            responses::Role::System => "system",
            responses::Role::User => "user",
            responses::Role::Assistant => "assistant",
            responses::Role::Developer => "developer",
        };
        let content = msg
            .content
            .into_iter()
            .map(|c| match c {
                responses::Content::OutputText(t) => t.text,
                responses::Content::Refusal(r) => format!("Refusal: {}", r.refusal),
            })
            .collect::<Vec<String>>()
            .join(" ");
        let mut message = Message::new(role.to_string(), content);
        message.id = Some(msg.id);
        message
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "LLM";

static PORT_EMBEDDINGS: &str = "embeddings";
static PORT_INPUT: &str = "input";
static PORT_MESSAGE: &str = "message";
static PORT_RESPONSE: &str = "response";

static CONFIG_MODEL: &str = "model";
static CONFIG_OPENAI_API_KEY: &str = "openai_api_key";
static CONFIG_OPTIONS: &str = "options";
static CONFIG_STREAM: &str = "stream";

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
        .with_string_config_with(CONFIG_MODEL, "gpt-3.5-turbo-instruct", |entry| {
            entry.with_title("Model")
        })
        .with_text_config_with(CONFIG_OPTIONS, "{}", |entry| entry.with_title("Options")),
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
        .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE])
        .with_custom_global_config_with(
            CONFIG_OPENAI_API_KEY,
            "",
            "password",
            |entry| entry.with_title("OpenAI API Key"),
        )
        .with_string_config_with(CONFIG_MODEL, DEFAULT_CONFIG_MODEL, |entry| {
            entry.with_title("Model")
        })
        .with_boolean_config_with(CONFIG_STREAM, false, |entry| entry.with_title("Stream"))
        .with_text_config_with(CONFIG_OPTIONS, "{}", |entry| entry.with_title("Options")),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "openai_embeddings",
            Some(new_agent_boxed::<OpenAIEmbeddingsAgent>),
        )
        // .use_native_thread()
        .with_title("OpenAI Embeddings")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_INPUT])
        .with_outputs(vec![PORT_EMBEDDINGS])
        .with_string_config_with(CONFIG_MODEL, "text-embedding-3-small", |entry| {
            entry.with_title("Model")
        })
        .with_text_config_with(CONFIG_OPTIONS, "{}", |entry| entry.with_title("Options")),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "openai_responses",
            Some(new_agent_boxed::<OpenAIResponsesAgent>),
        )
        // .use_native_thread()
        .with_title("OpenAI Responses")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGE])
        .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE])
        .with_string_config_with(CONFIG_MODEL, DEFAULT_CONFIG_MODEL, |entry| {
            entry.with_title("Model")
        })
        .with_boolean_config_with(CONFIG_STREAM, false, |entry| entry.with_title("Stream"))
        .with_text_config_with(CONFIG_OPTIONS, "{}", |entry| entry.with_title("Options")),
    );
}
