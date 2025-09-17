use std::sync::{Arc, Mutex};
use std::vec;

use agent_stream_kit::prelude::*;
use rig::client::CompletionClient;
use rig::completion::CompletionRequestBuilder;
use rig::providers::ollama::{self, ClientBuilder};

use crate::prompt::data_to_prompts;

// Rig Ollama Agent
pub struct RigOllamaAgent {
    data: AsAgentData,
    client: Arc<Mutex<Option<ollama::Client>>>,
}

impl RigOllamaAgent {
    // fn get_ollama_url(&self) -> Result<String, AgentError> {
    //     let mut ollama_url = self
    //         .get_global_config()
    //         .ok_or(AgentError::NoGlobalConfig)?
    //         .get_string_or_default(CONFIG_OLLAMA_URL);
    //     if ollama_url.is_empty() {
    //         if let Ok(ollama_host) = std::env::var("OLLAMA_HOST") {
    //             ollama_url = format!("http://{}", ollama_host);
    //         } else {
    //             ollama_url = DEFAULT_OLLAMA_URL.to_string();
    //         }
    //     }
    //     Ok(ollama_url)
    // }

    fn get_client(&mut self) -> Result<ollama::Client, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        // let ollama_url = self.get_ollama_url()?;
        // let new_client = ollama::Client::from_url(&api_base);
        let api_base = std::env::var("OLLAMA_API_BASE_URL").expect("OLLAMA_API_BASE_URL not set");
        let new_client = ClientBuilder::new()
            .base_url(&api_base)
            .build()
            .map_err(|e| AgentError::IoError(format!("Failed to create Ollama client: {}", e)))?;

        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

#[async_trait]
impl AsAgent for RigOllamaAgent {
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
            let comp_model = comp_model.clone();
            let user_message = prompt.message;

            let mut builder = CompletionRequestBuilder::new(comp_model, user_message);
            if let Some(preamble) = prompt.preamble {
                builder = builder.preamble(preamble);
            }
            if prompt.history.len() > 0 {
                builder = builder.messages(prompt.history);
            }
            let response = builder
                .send()
                .await
                .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

            let msg_json = serde_json::to_value(response.raw_response.message.clone())
                .map_err(|e| AgentError::InvalidValue(format!("serde_json error: {}", e)))?;
            let msg_value = AgentValue::from_json_value(msg_json)?;
            out_messages.push(msg_value);

            let resp_json = serde_json::to_value(response.raw_response)
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
// static CONFIG_OLLAMA_URL: &str = "ollama_url";

const DEFAULT_CONFIG_MODEL: &str = "gemma3:4b";
// const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "rig_ollama", Some(new_boxed::<RigOllamaAgent>))
            // .use_native_thread()
            .with_title("Rig Ollama")
            .with_category(CATEGORY)
            .with_inputs(vec![PORT_MESSAGE])
            .with_outputs(vec![PORT_MESSAGE, PORT_RESPONSE])
            // .with_global_config(vec![(
            //     CONFIG_OLLAMA_URL.into(),
            //     AgentConfigEntry::new(AgentValue::new_string(DEFAULT_OLLAMA_URL), "string")
            //         .with_title("Ollama URL"),
            // )])
            .with_default_config(vec![(
                CONFIG_MODEL.into(),
                AgentConfigEntry::new(AgentValue::new_string(DEFAULT_CONFIG_MODEL), "string")
                    .with_title("Chat Model"),
            )]),
    );
}
