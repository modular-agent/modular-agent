use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AgentValueMap,
    AsAgent, Message, ModularAgent, async_trait, modular_agent,
};

use crate::provider::{ModelIdentifier, ProviderKind};

#[cfg(feature = "openai")]
use crate::openai_client;

#[cfg(feature = "ollama")]
use crate::ollama_client;

const CATEGORY: &str = "LLM";

const PORT_MESSAGE: &str = "message";
const PORT_PROMPT: &str = "prompt";
const PORT_RESET: &str = "reset";
const PORT_RESPONSE: &str = "response";

const CONFIG_MODEL: &str = "model";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_SYSTEM: &str = "system";
const CONFIG_USE_CONTEXT: &str = "use_context";

const DEFAULT_CONFIG_MODEL: &str = "gpt-3.5-turbo-instruct";

/// Completion Agent that routes to different LLM providers based on model prefix.
///
/// # Model Format
/// - `openai/gpt-3.5-turbo-instruct` - Uses OpenAI API
/// - `ollama/codellama` - Uses Ollama
/// - `gpt-3.5-turbo-instruct` - No prefix defaults to OpenAI (backward compatible)
///
/// # Ollama-specific Features
/// - `use_context`: When enabled, maintains conversation context across requests
#[modular_agent(
    title = "Completion",
    category = CATEGORY,
    inputs = [PORT_PROMPT, PORT_RESET],
    outputs = [PORT_MESSAGE, PORT_RESPONSE],
    string_config(name = CONFIG_MODEL, default = DEFAULT_CONFIG_MODEL),
    text_config(name = CONFIG_SYSTEM, default = ""),
    boolean_config(name = CONFIG_USE_CONTEXT, title = "Use Context (Ollama only)"),
    object_config(name = CONFIG_OPTIONS),
)]
pub struct CompletionAgent {
    data: AgentData,
    #[cfg(feature = "openai")]
    openai_manager: openai_client::OpenAIManager,
    #[cfg(feature = "ollama")]
    ollama_manager: ollama_client::OllamaManager,
    #[cfg(feature = "ollama")]
    context: Option<ollama_client::GenerationContext>,
}

#[async_trait]
impl AsAgent for CompletionAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            #[cfg(feature = "openai")]
            openai_manager: openai_client::OpenAIManager::new(),
            #[cfg(feature = "ollama")]
            ollama_manager: ollama_client::OllamaManager::new(),
            #[cfg(feature = "ollama")]
            context: None,
        })
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        #[cfg(feature = "ollama")]
        {
            self.context = None;
        }
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
            #[cfg(feature = "ollama")]
            {
                self.context = None;
            }
            return Ok(());
        }

        let config_model = self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        // Parse model identifier to determine provider
        let model_id = ModelIdentifier::parse(&config_model)?;

        // Get prompt from value
        let prompt = value.as_str().unwrap_or("");
        if prompt.is_empty() {
            return Ok(());
        }

        // Get common configs
        let config_options = self.configs()?.get_object_or_default(CONFIG_OPTIONS);
        let config_system = self.configs()?.get_string_or_default(CONFIG_SYSTEM);

        // Route to appropriate provider
        match model_id.provider {
            #[cfg(feature = "claude")]
            ProviderKind::Claude => Err(AgentError::InvalidConfig(
                "Claude does not support text completions. Use ChatAgent instead.".into(),
            )),
            #[cfg(feature = "openai")]
            ProviderKind::OpenAI => {
                self.process_openai(
                    ctx,
                    prompt,
                    &model_id.model_name,
                    config_options,
                    config_system,
                )
                .await
            }
            #[cfg(feature = "ollama")]
            ProviderKind::Ollama => {
                self.process_ollama(
                    ctx,
                    prompt,
                    &model_id.model_name,
                    config_options,
                    config_system,
                )
                .await
            }
            #[allow(unreachable_patterns)]
            _ => Err(AgentError::InvalidConfig(format!(
                "Provider {:?} not enabled. Enable the corresponding feature.",
                model_id.provider
            ))),
        }
    }
}

impl CompletionAgent {
    #[cfg(feature = "openai")]
    async fn process_openai(
        &mut self,
        ctx: AgentContext,
        prompt: &str,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_system: String,
    ) -> Result<(), AgentError> {
        use async_openai::types::completions::{
            CreateCompletionRequest, CreateCompletionRequestArgs,
        };

        let client = self.openai_manager.get_client(self.ma())?;

        // Build the prompt with system message if provided
        let full_prompt = if !config_system.is_empty() {
            format!("{}\n\n{}", config_system, prompt)
        } else {
            prompt.to_string()
        };

        let mut request = CreateCompletionRequestArgs::default()
            .model(model_name)
            .prompt(full_prompt)
            .build()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to build request: {}", e)))?;

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
            request = serde_json::from_value::<CreateCompletionRequest>(request_json)
                .map_err(|e| AgentError::InvalidValue(format!("Deserialization error: {}", e)))?;
        }

        let res = client
            .completions()
            .create(request)
            .await
            .map_err(|e| AgentError::IoError(format!("OpenAI Error: {}", e)))?;

        let message = Message::assistant(res.choices[0].text.clone());
        self.output(ctx.clone(), PORT_MESSAGE.to_string(), message.into())
            .await?;

        let out_response = AgentValue::from_serialize(&res)?;
        self.output(ctx, PORT_RESPONSE.to_string(), out_response)
            .await?;

        Ok(())
    }

    #[cfg(feature = "ollama")]
    async fn process_ollama(
        &mut self,
        ctx: AgentContext,
        prompt: &str,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_system: String,
    ) -> Result<(), AgentError> {
        use ollama_rs::generation::completion::request::GenerationRequest;
        use ollama_rs::models::ModelOptions;

        let client = self.ollama_manager.get_client(self.ma())?;

        let use_context = self.configs()?.get_bool_or_default(CONFIG_USE_CONTEXT);

        let mut request = GenerationRequest::new(model_name.to_string(), prompt);

        if !config_system.is_empty() {
            request = request.system(config_system);
        }

        if !config_options.is_empty() {
            let config_options = serde_json::to_value(&config_options)
                .map_err(|e| AgentError::InvalidValue(format!("Invalid JSON in options: {}", e)))?;
            if let Ok(options_json) = serde_json::from_value::<ModelOptions>(config_options) {
                request = request.options(options_json);
            } else {
                return Err(AgentError::InvalidValue(
                    "Invalid JSON in options".to_string(),
                ));
            }
        }

        if use_context {
            if let Some(context) = &self.context {
                request = request.context(context.clone());
            }
        }

        let res = client
            .generate(request)
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

        if use_context {
            self.context = res.context.clone().or(self.context.clone());
        }

        let message = Message::assistant(res.response.clone());
        self.output(ctx.clone(), PORT_MESSAGE.to_string(), message.into())
            .await?;

        let out_response = AgentValue::from_serialize(&res)?;
        self.output(ctx, PORT_RESPONSE.to_string(), out_response)
            .await?;

        Ok(())
    }
}
