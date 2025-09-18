use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfig, AgentConfigEntry, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AgentValue, AgentValueMap, AsAgent, AsAgentData, async_trait,
    new_agent_boxed,
};
use rig::client::CompletionClient;
use rig::completion::CompletionRequestBuilder;
use rig::providers::openai::{self, ClientBuilder};

use crate::prompt::data_to_prompts;

// Rig OpenAI Agent
pub struct RigOpenAIAgent {
    data: AsAgentData,
    client: Arc<Mutex<Option<openai::Client>>>,
}

impl RigOpenAIAgent {
    fn get_client(&mut self) -> Result<openai::Client, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            AgentError::IoError("OPENAI_API_KEY environment variable not set".to_string())
        })?;
        let new_client = ClientBuilder::new(&api_key)
            .build()
            .map_err(|e| AgentError::IoError(format!("Failed to create OpenAI client: {}", e)))?;

        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

#[async_trait]
impl AsAgent for RigOpenAIAgent {
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

        let client = self.get_client()?;
        let comp_model = client.completion_model(config_model);

        let prompts = data_to_prompts(data)?;

        let mut out_messages = Vec::new();
        let mut out_responses = Vec::new();

        for prompt in prompts {
            let user_message = prompt.message;

            let mut builder = CompletionRequestBuilder::new(comp_model.clone(), user_message);
            if let Some(preamble) = prompt.preamble {
                builder = builder.preamble(preamble);
            }
            if prompt.history.len() > 0 {
                builder = builder.messages(prompt.history);
            }

            let response = builder
                .send()
                .await
                .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

            for output in &response.raw_response.output {
                let msg_json = serde_json::to_value(output)
                    .map_err(|e| AgentError::InvalidValue(format!("serde_json error: {}", e)))?;
                let Some(ty) = msg_json.get("type") else {
                    continue;
                };
                if ty != "message" {
                    continue;
                }
                let Some(role) = msg_json.get("role").and_then(|v| v.as_str()) else {
                    return Err(AgentError::InvalidValue(
                        "missing role in a response".to_string(),
                    ));
                };
                let Some(content) = msg_json
                    .get("content")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.get(0))
                    .and_then(|v| v.get("text"))
                    .and_then(|v| v.as_str())
                else {
                    return Err(AgentError::InvalidValue(
                        "missing content text in a response".to_string(),
                    ));
                };
                let msg_value = AgentValue::new_object(AgentValueMap::from([
                    ("role".to_string(), AgentValue::new_string(role.to_string())),
                    (
                        "content".to_string(),
                        AgentValue::new_string(content.to_string()),
                    ),
                ]));
                out_messages.push(msg_value);
            }

            let resp_json = serde_json::to_value(&response.raw_response)
                .map_err(|e| AgentError::InvalidValue(format!("serde_json error: {}", e)))?;
            let resp_value = AgentValue::from_json_value(resp_json)?;
            out_responses.push(resp_value);
        }

        if out_messages.len() == 1 {
            let out_message = AgentData::new_custom_object(
                "message",
                out_messages[0]
                    .as_object()
                    .ok_or_else(|| AgentError::InvalidValue("wrong object".to_string()))?
                    .to_owned(),
            );
            self.try_output(ctx.clone(), PORT_MESSAGE, out_message)?;
        } else if out_messages.len() > 1 {
            let out_message = AgentData::new_array("message", out_messages);
            self.try_output(ctx.clone(), PORT_MESSAGE, out_message)?;
        }

        if out_responses.len() == 1 {
            let out_response = AgentData::new_custom_object(
                "response",
                out_responses[0]
                    .as_object()
                    .ok_or_else(|| AgentError::InvalidValue("wrong object".to_string()))?
                    .to_owned(),
            );
            self.try_output(ctx, PORT_RESPONSE, out_response)?;
        } else if out_responses.len() > 1 {
            let out_response = AgentData::new_array("response", out_responses);
            self.try_output(ctx, PORT_RESPONSE, out_response)?;
        }

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/Rig";

static PORT_MESSAGE: &str = "message";
static PORT_RESPONSE: &str = "response";

static CONFIG_MODEL: &str = "model";

const DEFAULT_CONFIG_MODEL: &str = "gpt-5-nano";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "rig_openai",
            Some(new_agent_boxed::<RigOpenAIAgent>),
        )
        .with_title("Rig OpenAI")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_MESSAGE])
        .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE])
        .with_default_config(vec![(
            CONFIG_MODEL.into(),
            AgentConfigEntry::new(AgentValue::new_string(DEFAULT_CONFIG_MODEL), "string")
                .with_title("Chat Model"),
        )]),
    );
}
