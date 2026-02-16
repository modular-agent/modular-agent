#![cfg(feature = "openai")]

use im::vector;
use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AgentValueMap,
    AsAgent, Message, ModularAgent, ToolCall, ToolCallFunction, async_trait, modular_agent,
};

use crate::openai_client;
use crate::provider::{ModelIdentifier, ProviderKind};

const CATEGORY: &str = "LLM";

const PORT_MESSAGE: &str = "message";
const PORT_RESPONSE: &str = "response";
const PORT_RESET: &str = "reset";

const CONFIG_MODEL: &str = "model";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_STREAM: &str = "stream";
const CONFIG_TOOLS: &str = "tools";
const CONFIG_USE_CONVERSATION_STATE: &str = "use_conversation_state";

const DEFAULT_MODEL: &str = "openai/gpt-5-mini";

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

        let model_id = ModelIdentifier::parse(&config_model)?;
        if model_id.provider != ProviderKind::OpenAI {
            return Err(AgentError::InvalidConfig(
                "ResponsesAgent only supports OpenAI models".into(),
            ));
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
            &model_id.model_name,
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
        use modular_agent_core::tool::list_tool_infos_patterns;

        let client = self.openai_manager.get_client(self.ma())?;

        // Build input from messages
        let input = openai_client::messages_to_response_input(&messages)?;

        // Build tools array
        let tools_json: Vec<serde_json::Value> = if config_tools.is_empty() {
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
                    serde_json::json!({
                        "type": "function",
                        "name": info.name,
                        "description": if info.description.is_empty() {
                            serde_json::Value::Null
                        } else {
                            serde_json::Value::String(info.description)
                        },
                        "parameters": info.parameters.unwrap_or(serde_json::json!({})),
                    })
                })
                .collect()
        };

        // Build request
        let mut request = serde_json::json!({
            "model": model_name,
            "input": input,
            "stream": use_stream,
        });

        // Add previous_response_id for conversation continuity
        if use_conversation_state && let Some(prev_id) = &self.last_response_id {
            request["previous_response_id"] = serde_json::Value::String(prev_id.clone());
        }

        // Add tools if configured
        if !tools_json.is_empty() {
            request["tools"] = serde_json::Value::Array(tools_json);
        }

        // Merge options
        openai_client::merge_options(&mut request, &config_options)?;

        let id = uuid::Uuid::new_v4().to_string();
        if use_stream {
            self.process_streaming(ctx, &client, &request, &id, use_conversation_state)
                .await
        } else {
            self.process_non_streaming(ctx, &client, &request, &id, use_conversation_state)
                .await
        }
    }

    async fn process_non_streaming(
        &mut self,
        ctx: AgentContext,
        client: &openai_client::OpenAIClient,
        request: &serde_json::Value,
        id: &str,
        use_conversation_state: bool,
    ) -> Result<(), AgentError> {
        let response: serde_json::Value =
            client.post_json(&client.responses_url(), request).await?;

        // Store response ID for conversation continuity
        if use_conversation_state && let Some(resp_id) = response.get("id").and_then(|v| v.as_str())
        {
            self.last_response_id = Some(resp_id.to_string());
        }

        // Convert response to message
        let output = response
            .get("output")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut message = openai_client::response_output_to_message(&output)?;
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
        client: &openai_client::OpenAIClient,
        request: &serde_json::Value,
        id: &str,
        use_conversation_state: bool,
    ) -> Result<(), AgentError> {
        use futures::StreamExt;

        let url = client.responses_url();
        let mut stream = client.post_stream(&url, request).await?;

        let mut message = Message::assistant(String::new());
        message.id = Some(id.to_string());

        let mut content = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut current_tool_name: Option<String> = None;
        let mut current_tool_call_id: Option<String> = None;
        let mut current_tool_arguments = String::new();

        while let Some(res) = stream.next().await {
            let Some(data) = res? else {
                continue; // [DONE] sentinel
            };
            let event: openai_client::ResponseStreamEvent =
                serde_json::from_str(&data).unwrap_or(openai_client::ResponseStreamEvent::Other);

            match event {
                openai_client::ResponseStreamEvent::OutputTextDelta { delta } => {
                    content.push_str(&delta);
                    message.content = content.clone();
                    self.output(
                        ctx.clone(),
                        PORT_MESSAGE.to_string(),
                        message.clone().into(),
                    )
                    .await?;
                }
                openai_client::ResponseStreamEvent::FunctionCallArgumentsDelta { delta } => {
                    current_tool_arguments.push_str(&delta);
                }
                openai_client::ResponseStreamEvent::OutputItemAdded { item } => {
                    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                        current_tool_name = Some(name.to_string());
                        current_tool_call_id = item
                            .get("call_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        current_tool_arguments.clear();
                    }
                }
                openai_client::ResponseStreamEvent::OutputItemDone { .. } => {
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
                openai_client::ResponseStreamEvent::Completed { response } => {
                    // Store response ID for conversation continuity
                    if use_conversation_state
                        && let Some(resp_id) = response.get("id").and_then(|v| v.as_str())
                    {
                        self.last_response_id = Some(resp_id.to_string());
                    }

                    let out_response = AgentValue::from_serialize(&response)?;
                    self.output(ctx.clone(), PORT_RESPONSE.to_string(), out_response)
                        .await?;
                }
                openai_client::ResponseStreamEvent::Other => {}
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
