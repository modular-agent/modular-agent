#![cfg(feature = "openai")]

use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use async_openai::types::responses::{FunctionArgs, ToolDefinition};
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCallChunk, ChatCompletionTool,
    ChatCompletionToolArgs, FunctionObjectArgs,
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

use crate::message::{self, Message, MessageHistory, ToolCall, ToolCallFunction};
use crate::tool::{self, list_tool_infos_patterns};

static CATEGORY: &str = "LLM/OpenAI";

static PIN_EMBEDDINGS: &str = "embeddings";
static PIN_HISTORY: &str = "history";
static PIN_INPUT: &str = "input";
static PIN_MESSAGE: &str = "message";
static PIN_RESET: &str = "reset";
static PIN_RESPONSE: &str = "response";

static CONFIG_MODEL: &str = "model";
static CONFIG_OPENAI_API_KEY: &str = "openai_api_key";
static CONFIG_OPTIONS: &str = "options";
static CONFIG_STREAM: &str = "stream";
static CONFIG_TOOLS: &str = "tools";

const DEFAULT_CONFIG_MODEL: &str = "gpt-5-nano";

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
#[askit_agent(
    title="OpenAI Completion",
    category=CATEGORY,
    inputs=[PIN_MESSAGE],
    outputs=[PIN_MESSAGE, PIN_RESPONSE],
    string_config(name=CONFIG_MODEL, default="gpt-3.5-turbo-instruct"),
    text_config(name=CONFIG_OPTIONS, default="{}"),
    string_global_config(name=CONFIG_OPENAI_API_KEY, title="OpenAI API Key")
)]
pub struct OpenAICompletionAgent {
    data: AgentData,
    manager: OpenAIManager,
}

#[async_trait]
impl AsAgent for OpenAICompletionAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OpenAIManager::new(),
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

        let mut messages;
        {
            if value.is_array() {
                let arr = value.as_array().unwrap();
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
                let message = value.as_str().unwrap_or("");
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
        self.try_output(ctx.clone(), PIN_MESSAGE, message.into())?;

        let out_response = AgentValue::from_serialize(&res)?;
        self.try_output(ctx, PIN_RESPONSE, out_response)?;

        Ok(())
    }
}

// OpenAI Chat Agent
#[askit_agent(
    title="OpenAI Chat",
    category=CATEGORY,
    inputs=[PIN_MESSAGE, PIN_RESET],
    outputs=[PIN_MESSAGE, PIN_HISTORY, PIN_RESPONSE],
    string_config(name=CONFIG_MODEL, default=DEFAULT_CONFIG_MODEL),
    boolean_config(name=CONFIG_STREAM, title="Stream"),
    string_config(name=CONFIG_TOOLS, default=""),
    text_config(name=CONFIG_OPTIONS, default="{}")
)]
pub struct OpenAIChatAgent {
    data: AgentData,
    manager: OpenAIManager,
    history: MessageHistory,
}

impl OpenAIChatAgent {
    async fn call_tools(
        &mut self,
        ctx: AgentContext,
        tool_calls: &Vec<ToolCall>,
    ) -> Result<(), AgentError> {
        let resp_messages = tool::call_tools(&ctx, tool_calls).await?;
        self.history.push_all(resp_messages);
        Ok(())
    }
}

#[async_trait]
impl AsAgent for OpenAIChatAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OpenAIManager::new(),
            history: MessageHistory::default(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PIN_RESET {
            self.history = MessageHistory::default();
            self.try_output(ctx, PIN_HISTORY, self.history.clone().into())?;
            return Ok(());
        }

        let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let messages = MessageHistory::from_value(value)?.messages();
        if messages.is_empty() {
            return Ok(());
        }

        for message in messages {
            self.try_output(ctx.clone(), PIN_MESSAGE, message.clone().into())?;
            self.history.push(message);
        }
        self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;

        if self.history.messages().last().unwrap().role != "user" {
            // If the last message isn’t a user message, just return
            return Ok(());
        }

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
        let options_json = if !config_options.is_empty() && config_options != "{}" {
            Some(
                serde_json::from_str::<serde_json::Value>(&config_options).map_err(|e| {
                    AgentError::InvalidValue(format!("Invalid JSON in options: {}", e))
                })?,
            )
        } else {
            None
        };
        let config_tools = self.configs()?.get_string_or_default(CONFIG_TOOLS);
        let tool_infos = if config_tools.is_empty() {
            vec![]
        } else {
            list_tool_infos_patterns(&config_tools)
                .map_err(|e| {
                    AgentError::InvalidConfig(format!(
                        "Invalid regex patterns in tools config: {}",
                        e
                    ))
                })?
                .into_iter()
                .map(|tool| tool.try_into())
                .collect::<Result<Vec<ChatCompletionTool>, AgentError>>()?
        };

        let use_stream = self.configs()?.get_bool_or_default(CONFIG_STREAM);

        let client = self.manager.get_client(self.askit())?;

        loop {
            let mut request = CreateChatCompletionRequestArgs::default()
                .model(config_model)
                .messages(
                    self.history
                        .messages_for_prompt()
                        .iter()
                        .map(|m| m.clone().into())
                        .collect::<Vec<ChatCompletionRequestMessage>>(),
                )
                .tools(tool_infos.clone())
                .stream(use_stream)
                .build()
                .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

            if let Some(options_json) = &options_json {
                // Merge options into request
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
                    .map_err(|e| {
                        AgentError::InvalidValue(format!("Deserialization error: {}", e))
                    })?;
            }

            let id = uuid::Uuid::new_v4().to_string();
            if use_stream {
                let mut stream = client
                    .chat()
                    .create_stream(request)
                    .await
                    .map_err(|e| AgentError::IoError(format!("OpenAI Stream Error: {}", e)))?;
                let mut message = Message::assistant("".to_string());
                message.id = Some(id.clone());
                let mut content = String::new();
                let mut thinking = String::new();
                let mut tool_calls: Vec<message::ToolCall> = Vec::new();
                while let Some(res) = stream.next().await {
                    let res =
                        res.map_err(|_| AgentError::IoError(format!("OpenAI Stream Error")))?;

                    for c in &res.choices {
                        if let Some(ref delta_content) = c.delta.content {
                            content.push_str(delta_content);
                        }
                        if let Some(tc) = &c.delta.tool_calls {
                            for call in tc {
                                tool_calls.push(call.try_into()?);
                            }
                        }
                        if let Some(refusal) = &c.delta.refusal {
                            thinking.push_str(&format!("Refusal: {}", refusal));
                        }
                    }

                    message.content = content.clone();
                    message.thinking = thinking.clone();
                    if !tool_calls.is_empty() {
                        message.tool_calls = Some(tool_calls.clone());
                    }

                    self.history.push(message.clone());

                    self.try_output(ctx.clone(), PIN_MESSAGE, message.clone().into())?;

                    let out_response = AgentValue::from_serialize(&res)?;
                    self.try_output(ctx.clone(), PIN_RESPONSE, out_response)?;

                    self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;
                }

                // Call tools if any
                if tool_calls.is_empty() {
                    return Ok(());
                }
                self.call_tools(ctx.clone(), &tool_calls).await?;
                self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;
            } else {
                let res = client
                    .chat()
                    .create(request)
                    .await
                    .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

                let mut tool_calls: Vec<ToolCall> = Vec::new();
                for c in &res.choices {
                    let mut message: Message = c.message.clone().into();
                    message.id = Some(id.clone());

                    self.history.push(message.clone());

                    self.try_output(ctx.clone(), PIN_MESSAGE, message.clone().into())?;

                    let out_response = AgentValue::from_serialize(&res)?;
                    self.try_output(ctx.clone(), PIN_RESPONSE, out_response)?;

                    self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;

                    if let Some(tc) = &c.message.tool_calls {
                        for call in tc {
                            tool_calls.push(call.try_into()?);
                        }
                    }
                }

                // Call tools if any
                if tool_calls.is_empty() {
                    return Ok(());
                }
                self.call_tools(ctx.clone(), &tool_calls).await?;
                self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;
            }
        }
    }
}

// OpenAI Embeddings Agent
#[askit_agent(
    title="OpenAI Embeddings",
    category=CATEGORY,
    inputs=[PIN_INPUT],
    outputs=[PIN_EMBEDDINGS],
    string_config(name=CONFIG_MODEL, default="text-embedding-3-small"),
    text_config(name=CONFIG_OPTIONS, default="{}")
)]
pub struct OpenAIEmbeddingsAgent {
    data: AgentData,
    manager: OpenAIManager,
}

#[async_trait]
impl AsAgent for OpenAIEmbeddingsAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OpenAIManager::new(),
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

        let input = value.as_str().unwrap_or(""); // TODO: other types
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

        let value = AgentValue::from_serialize(&res.data)?;
        self.try_output(ctx.clone(), PIN_EMBEDDINGS, value)?;

        Ok(())
    }
}

// OpenAI Responses Agent
// https://platform.openai.com/docs/api-reference/responses
#[askit_agent(
    title="OpenAI Responses",
    category=CATEGORY,
    inputs=[PIN_MESSAGE, PIN_RESET],
    outputs=[PIN_MESSAGE, PIN_HISTORY, PIN_RESPONSE],
    string_config(name=CONFIG_MODEL, default=DEFAULT_CONFIG_MODEL),
    boolean_config(name=CONFIG_STREAM, title="Stream"),
    string_config(name=CONFIG_TOOLS, default=""),
    text_config(name=CONFIG_OPTIONS, default="{}")
)]
pub struct OpenAIResponsesAgent {
    data: AgentData,
    manager: OpenAIManager,
    history: MessageHistory,
}

impl OpenAIResponsesAgent {
    async fn call_tools(
        &mut self,
        ctx: AgentContext,
        tool_calls: &Vec<ToolCall>,
    ) -> Result<(), AgentError> {
        let resp_messages = tool::call_tools(&ctx, tool_calls).await?;
        self.history.push_all(resp_messages);
        Ok(())
    }
}

#[async_trait]
impl AsAgent for OpenAIResponsesAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            manager: OpenAIManager::new(),
            history: MessageHistory::default(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PIN_RESET {
            self.history = MessageHistory::default();
            self.try_output(ctx, PIN_HISTORY, self.history.clone().into())?;
            return Ok(());
        }

        let config_model = &self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let messages = MessageHistory::from_value(value)?.messages();
        if messages.is_empty() {
            return Ok(());
        }

        for message in messages {
            self.try_output(ctx.clone(), PIN_MESSAGE, message.clone().into())?;
            self.history.push(message);
        }
        self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;

        if self.history.messages().last().unwrap().role != "user" {
            // If the last message isn’t a user message, just return
            return Ok(());
        }

        let config_options = self.configs()?.get_string_or_default(CONFIG_OPTIONS);
        let options_json = if !config_options.is_empty() && config_options != "{}" {
            Some(
                serde_json::from_str::<serde_json::Value>(&config_options).map_err(|e| {
                    AgentError::InvalidValue(format!("Invalid JSON in options: {}", e))
                })?,
            )
        } else {
            None
        };

        let config_tools = self.configs()?.get_string_or_default(CONFIG_TOOLS);
        let tool_infos = if config_tools.is_empty() {
            vec![]
        } else {
            list_tool_infos_patterns(&config_tools)
                .map_err(|e| {
                    AgentError::InvalidConfig(format!(
                        "Invalid regex patterns in tools config: {}",
                        e
                    ))
                })?
                .into_iter()
                .map(|tool| tool.try_into())
                .collect::<Result<Vec<ToolDefinition>, AgentError>>()?
        };

        let use_stream = self.configs()?.get_bool_or_default(CONFIG_STREAM);

        let client = self.manager.get_client(self.askit())?;

        loop {
            let mut request = CreateResponseArgs::default()
                .model(config_model)
                .input(responses::Input::Items(
                    self.history
                        .messages_for_prompt()
                        .iter()
                        .map(|m| m.into())
                        .collect::<Vec<responses::InputItem>>(),
                ))
                .tools(tool_infos.clone())
                .stream(use_stream)
                .build()
                .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

            if let Some(options_json) = &options_json {
                // Merge options into request
                let mut request_json = serde_json::to_value(&request)
                    .map_err(|e| AgentError::InvalidValue(format!("Serialization error: {}", e)))?;

                if let (Some(request_obj), Some(options_obj)) =
                    (request_json.as_object_mut(), options_json.as_object())
                {
                    for (key, value) in options_obj {
                        request_obj.insert(key.clone(), value.clone());
                    }
                }
                request = serde_json::from_value::<CreateResponse>(request_json).map_err(|e| {
                    AgentError::InvalidValue(format!("Deserialization error: {}", e))
                })?;
            }

            let id = uuid::Uuid::new_v4().to_string();
            if use_stream {
                let mut stream = client
                    .responses()
                    .create_stream(request)
                    .await
                    .map_err(|e| AgentError::IoError(format!("OpenAI Stream Error: {}", e)))?;

                let mut message = Message::assistant("".to_string());
                message.id = Some(id.clone());
                let mut content = String::new();
                let mut tool_calls: Vec<message::ToolCall> = Vec::new();
                while let Some(res) = stream.next().await {
                    let res_event = res
                        .map_err(|e| AgentError::IoError(format!("OpenAI Stream Error: {}", e)))?;

                    match &res_event {
                        responses::ResponseEvent::ResponseOutputTextDelta(delta) => {
                            content.push_str(&delta.delta);
                        }
                        responses::ResponseEvent::ResponseFunctionCallArgumentsDone(fc) => {
                            if let Ok(parameters) =
                                serde_json::from_str::<serde_json::Value>(&fc.arguments)
                            {
                                let call = ToolCall {
                                    function: ToolCallFunction {
                                        id: Some(fc.item_id.clone()),
                                        name: fc.name.clone(),
                                        parameters,
                                    },
                                };
                                tool_calls.push(call);
                            }
                        }
                        responses::ResponseEvent::ResponseCompleted(_) => {
                            let out_response = AgentValue::from_serialize(&res_event)?;
                            self.try_output(ctx.clone(), PIN_RESPONSE, out_response)?;
                            break;
                        }
                        _ => {}
                    }

                    message.content = content.clone();
                    if !tool_calls.is_empty() {
                        message.tool_calls = Some(tool_calls.clone());
                    }

                    self.history.push(message.clone());

                    self.try_output(ctx.clone(), PIN_MESSAGE, message.clone().into())?;

                    let out_response = AgentValue::from_serialize(&res_event)?;
                    self.try_output(ctx.clone(), PIN_RESPONSE, out_response)?;

                    self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;
                }

                // Call tools if any
                if tool_calls.is_empty() {
                    return Ok(());
                }
                self.call_tools(ctx.clone(), &tool_calls).await?;
                self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;
            } else {
                let res = client
                    .responses()
                    .create(request)
                    .await
                    .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

                // TODO: support tool calls
                let mut res_message: Message = Message::assistant(get_output_text(&res)); // TODO: better conversion
                res_message.id = Some(res.id.clone());

                self.history.push(res_message.clone());

                self.try_output(ctx.clone(), PIN_MESSAGE, res_message.clone().into())?;

                let out_response = AgentValue::from_serialize(&res)?;
                self.try_output(ctx.clone(), PIN_RESPONSE, out_response)?;

                self.try_output(ctx.clone(), PIN_HISTORY, self.history.clone().into())?;

                return Ok(());
            }
        }
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
        let content = msg.content.unwrap_or_default();
        let thinking = msg
            .refusal
            .map(|r| format!("Refusal: {}", r))
            .unwrap_or_default();

        let mut msg = Message::new(role.to_string(), content);
        msg.thinking = thinking;
        msg
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

impl TryFrom<tool::ToolInfo> for ChatCompletionTool {
    type Error = AgentError;

    fn try_from(info: tool::ToolInfo) -> Result<Self, Self::Error> {
        let mut function = FunctionObjectArgs::default();
        function.name(info.name);
        if !info.description.is_empty() {
            function.description(info.description);
        }
        if let Some(params) = info.parameters {
            function.parameters(serde_json::to_value(params).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to serialize tool parameters: {}", e))
            })?);
        }
        Ok(ChatCompletionToolArgs::default()
            .function(function.build().map_err(|e| {
                AgentError::InvalidValue(format!("Failed to build tool function: {}", e))
            })?)
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build tool: {}", e)))?)
    }
}

impl TryFrom<tool::ToolInfo> for ToolDefinition {
    type Error = AgentError;

    fn try_from(info: tool::ToolInfo) -> Result<Self, Self::Error> {
        let mut function = FunctionArgs::default();
        function.name(info.name);
        if !info.description.is_empty() {
            function.description(info.description);
        }
        if let Some(params) = info.parameters {
            function.parameters(serde_json::to_value(params).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to serialize tool parameters: {}", e))
            })?);
        }
        Ok(ToolDefinition::Function(function.build().map_err(|e| {
            AgentError::InvalidValue(format!("Failed to build tool function: {}", e))
        })?))
    }
}

impl TryFrom<&ChatCompletionMessageToolCallChunk> for message::ToolCall {
    type Error = AgentError;

    fn try_from(call: &ChatCompletionMessageToolCallChunk) -> Result<Self, AgentError> {
        let Some(function) = &call.function else {
            return Err(AgentError::InvalidValue(
                "ToolCallChunk missing function".to_string(),
            ));
        };
        let Some(name) = &function.name else {
            return Err(AgentError::InvalidValue(
                "ToolCallChunk function missing name".to_string(),
            ));
        };
        let parameters = if let Some(arguments) = &function.arguments {
            serde_json::from_str(arguments).map_err(|e| {
                AgentError::InvalidValue(format!("Failed to parse tool call arguments JSON: {}", e))
            })?
        } else {
            serde_json::json!({})
        };

        let function = message::ToolCallFunction {
            id: call.id.clone(),
            name: name.clone(),
            parameters,
        };
        Ok(message::ToolCall { function })
    }
}

impl TryFrom<&ChatCompletionMessageToolCall> for message::ToolCall {
    type Error = AgentError;

    fn try_from(call: &ChatCompletionMessageToolCall) -> Result<Self, AgentError> {
        let parameters = serde_json::from_str(&call.function.arguments).map_err(|e| {
            AgentError::InvalidValue(format!("Failed to parse tool call arguments JSON: {}", e))
        })?;

        let function = message::ToolCallFunction {
            id: Some(call.id.clone()),
            name: call.function.name.clone(),
            parameters,
        };
        Ok(message::ToolCall { function })
    }
}
