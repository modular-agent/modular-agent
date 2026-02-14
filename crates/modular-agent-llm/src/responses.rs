#![cfg(feature = "openai")]

use im::vector;
use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AgentValueMap,
    AsAgent, Message, ModularAgent, ToolCall, ToolCallFunction, async_trait, modular_agent,
};

use crate::openai_client;

const CATEGORY: &str = "LLM";

const PORT_MESSAGE: &str = "message";
const PORT_RESPONSE: &str = "response";
const PORT_RESET: &str = "reset";

const CONFIG_MODEL: &str = "model";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_STREAM: &str = "stream";
const CONFIG_TOOLS: &str = "tools";
const CONFIG_USE_CONVERSATION_STATE: &str = "use_conversation_state";

const DEFAULT_MODEL: &str = "gpt-5-mini";

/// Responses Agent using OpenAI Responses API.
///
/// The Responses API is OpenAI's new API primitive that provides:
/// - Server-side conversation state via `previous_response_id`
/// - Built-in tools (web_search, file_search, code_interpreter) - future support
/// - Semantic streaming events
/// - Better performance with reasoning models
///
/// # Configuration
/// - `model`: Model name (default: "gpt-5-mini")
/// - `stream`: Enable streaming mode
/// - `use_conversation_state`: Use server-side conversation state
/// - `tools`: Tool patterns to enable (regex, newline-separated)
/// - `options`: Additional request options as JSON
///
/// # Ports
/// - Input `message`: Message or array of messages to send
/// - Input `reset`: Any value to reset conversation state
/// - Output `message`: Assistant's response message
/// - Output `response`: Raw API response
#[modular_agent(
    title = "Responses",
    category = CATEGORY,
    inputs = [PORT_MESSAGE, PORT_RESET],
    outputs = [PORT_MESSAGE, PORT_RESPONSE],
    boolean_config(name = CONFIG_STREAM, title = "Stream"),
    boolean_config(name = CONFIG_USE_CONVERSATION_STATE, title = "Use Conversation State"),
    string_config(name = CONFIG_MODEL, default = DEFAULT_MODEL),
    text_config(name = CONFIG_TOOLS),
    object_config(name = CONFIG_OPTIONS),
)]
pub struct ResponsesAgent {
    data: AgentData,
    openai_manager: openai_client::OpenAIManager,
    last_response_id: Option<String>,
}

#[async_trait]
impl AsAgent for ResponsesAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            openai_manager: openai_client::OpenAIManager::new(),
            last_response_id: None,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        self.last_response_id = None;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.last_response_id = None;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        // Handle reset port
        if port == PORT_RESET {
            self.last_response_id = None;
            return Ok(());
        }

        let config = self.configs()?;
        let config_model = config.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        // Convert value to messages
        let Some(value) = value.to_message_value() else {
            return Err(AgentError::InvalidValue(
                "Input value is not a valid message".to_string(),
            ));
        };
        let messages = if value.is_array() {
            value.into_array().unwrap()
        } else {
            vector![value]
        };
        if messages.is_empty() {
            return Ok(());
        }

        // If the last message isn't a user/tool message, just return
        let role = &messages.last().unwrap().as_message().unwrap().role;
        if role != "user" && role != "tool" {
            return Ok(());
        }

        // Get configs
        let config_options = config.get_object_or_default(CONFIG_OPTIONS);
        let config_tools = config.get_string_or_default(CONFIG_TOOLS);
        let use_stream = config.get_bool_or_default(CONFIG_STREAM);
        let use_conversation_state = config.get_bool_or_default(CONFIG_USE_CONVERSATION_STATE);

        self.process_response(
            ctx,
            messages,
            &config_model,
            config_options,
            config_tools,
            use_stream,
            use_conversation_state,
        )
        .await
    }
}

impl ResponsesAgent {
    async fn process_response(
        &mut self,
        ctx: AgentContext,
        messages: im::Vector<AgentValue>,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_tools: String,
        use_stream: bool,
        use_conversation_state: bool,
    ) -> Result<(), AgentError> {
        use async_openai::types::responses::{
            CreateResponse, CreateResponseArgs, FunctionTool, Tool,
        };
        use modular_agent_core::tool::list_tool_infos_patterns;

        let client = self.openai_manager.get_client(self.ma())?;

        // Build input from messages
        let input = openai_client::messages_to_response_input(&messages)?;

        // Build tools array
        let tools: Vec<Tool> = if config_tools.is_empty() {
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
                .map(|info| {
                    let function = FunctionTool {
                        name: info.name,
                        description: if info.description.is_empty() {
                            None
                        } else {
                            Some(info.description)
                        },
                        parameters: info.parameters,
                        strict: None,
                    };
                    Tool::Function(function)
                })
                .collect()
        };

        // Build request
        let mut request = CreateResponseArgs::default()
            .model(model_name)
            .input(input)
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

        // Add previous_response_id for conversation continuity
        if use_conversation_state {
            if let Some(prev_id) = &self.last_response_id {
                request.previous_response_id = Some(prev_id.clone());
            }
        }

        // Add tools if configured
        if !tools.is_empty() {
            request.tools = Some(tools);
        }

        // Merge options
        if !config_options.is_empty() {
            let options_json = serde_json::to_value(&config_options)
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

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            self.process_streaming(ctx, &client, request, &id, use_conversation_state)
                .await
        } else {
            self.process_non_streaming(ctx, &client, request, &id, use_conversation_state)
                .await
        }
    }

    async fn process_non_streaming(
        &mut self,
        ctx: AgentContext,
        client: &async_openai::Client<async_openai::config::OpenAIConfig>,
        request: async_openai::types::responses::CreateResponse,
        id: &str,
        use_conversation_state: bool,
    ) -> Result<(), AgentError> {
        use async_openai::types::responses::Response;

        let response: Response = client
            .responses()
            .create(request)
            .await
            .map_err(|e| AgentError::IoError(format!("OpenAI Responses Error: {}", e)))?;

        // Store response ID for conversation continuity
        if use_conversation_state {
            self.last_response_id = Some(response.id.clone());
        }

        // Convert response to message
        let mut message = openai_client::response_output_to_message(&response.output)?;
        message.id = Some(id.to_string());

        // Output message
        self.output(ctx.clone(), PORT_MESSAGE.to_string(), message.into())
            .await?;

        // Output raw response
        let out_response = AgentValue::from_serialize(&response)?;
        self.output(ctx, PORT_RESPONSE.to_string(), out_response)
            .await?;

        Ok(())
    }

    async fn process_streaming(
        &mut self,
        ctx: AgentContext,
        client: &async_openai::Client<async_openai::config::OpenAIConfig>,
        request: async_openai::types::responses::CreateResponse,
        id: &str,
        use_conversation_state: bool,
    ) -> Result<(), AgentError> {
        use async_openai::types::responses::ResponseStreamEvent;
        use futures::StreamExt;

        let mut stream = client
            .responses()
            .create_stream(request)
            .await
            .map_err(|e| AgentError::IoError(format!("OpenAI Responses Stream Error: {}", e)))?;

        let mut message = Message::assistant(String::new());
        message.id = Some(id.to_string());

        let mut content = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut current_tool_name: Option<String> = None;
        let mut current_tool_call_id: Option<String> = None;
        let mut current_tool_arguments = String::new();

        while let Some(event) = stream.next().await {
            let event = event.map_err(|e| AgentError::IoError(format!("Stream error: {}", e)))?;

            match event {
                ResponseStreamEvent::ResponseOutputTextDelta(delta_event) => {
                    content.push_str(&delta_event.delta);
                    message.content = content.clone();
                    self.output(
                        ctx.clone(),
                        PORT_MESSAGE.to_string(),
                        message.clone().into(),
                    )
                    .await?;
                }
                ResponseStreamEvent::ResponseFunctionCallArgumentsDelta(args_delta) => {
                    current_tool_arguments.push_str(&args_delta.delta);
                }
                ResponseStreamEvent::ResponseOutputItemAdded(item_added) => {
                    // Check if this is a function call item using JSON serialization
                    let item_json = serde_json::to_value(&item_added.item).unwrap_or_default();
                    if let Some(name) = item_json.get("name").and_then(|v| v.as_str()) {
                        current_tool_name = Some(name.to_string());
                        current_tool_call_id = item_json
                            .get("call_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        current_tool_arguments.clear();
                    }
                }
                ResponseStreamEvent::ResponseOutputItemDone(_item_done) => {
                    // Handle completed function call
                    if let Some(name) = current_tool_name.take() {
                        let parameters: serde_json::Value =
                            serde_json::from_str(&current_tool_arguments).unwrap_or_default();
                        let tool_call = ToolCall {
                            function: ToolCallFunction {
                                id: current_tool_call_id.take(),
                                name,
                                parameters,
                            },
                        };
                        tool_calls.push(tool_call);
                        message.tool_calls = Some(tool_calls.clone().into());
                        current_tool_arguments.clear();

                        self.output(
                            ctx.clone(),
                            PORT_MESSAGE.to_string(),
                            message.clone().into(),
                        )
                        .await?;
                    }
                }
                ResponseStreamEvent::ResponseCompleted(completed) => {
                    // Store response ID for conversation continuity
                    if use_conversation_state {
                        self.last_response_id = Some(completed.response.id.clone());
                    }

                    let out_response = AgentValue::from_serialize(&completed.response)?;
                    self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                        .await?;
                }
                _ => {
                    // Handle other events as needed
                }
            }
        }

        Ok(())
    }
}

// TODO: Future support for built-in tools
// The Responses API supports these built-in tools:
// - web_search: Search the web for information
// - file_search: Search files in vector stores
// - code_interpreter: Execute code in a sandbox
//
// These can be enabled via the options config as JSON:
// {
//   "tools": [
//     { "type": "web_search" },
//     { "type": "file_search", "vector_store_ids": ["vs_abc123"] },
//     { "type": "code_interpreter" }
//   ]
// }
