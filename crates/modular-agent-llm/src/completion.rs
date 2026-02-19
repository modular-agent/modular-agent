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

const CONFIG_MAX_TOKENS: &str = "max_tokens";
const CONFIG_MODEL: &str = "model";
const CONFIG_OPTIONS: &str = "options";
const CONFIG_SYSTEM: &str = "system";
const CONFIG_TEMPERATURE: &str = "temperature";
const CONFIG_TOP_P: &str = "top_p";
const CONFIG_USE_CONTEXT: &str = "use_context";

const DEFAULT_CONFIG_MODEL: &str = "openai/gpt-3.5-turbo-instruct";

/// Completion Agent that routes to different LLM providers based on model prefix.
///
/// # Model Format
/// - `openai/gpt-3.5-turbo-instruct` - Uses OpenAI API
/// - `ollama/codellama` - Uses Ollama
/// - `openai/gpt-3.5-turbo-instruct` - Uses OpenAI
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
    integer_config(name = CONFIG_MAX_TOKENS, title = "Max Tokens", default = 0, description = "0: use API default", detail),
    number_config(name = CONFIG_TEMPERATURE, title = "Temperature", default = -1.0, description = "-1: use API default (0.0-2.0)", detail),
    number_config(name = CONFIG_TOP_P, title = "Top P", default = -1.0, description = "-1: use API default (0.0-1.0)", detail),
    object_config(name = CONFIG_OPTIONS, title = "Options", description = "Additional request options as JSON", detail),
    hint(width = 2, height = 2),
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
        let config = self.configs()?;
        let config_options = config.get_object_or_default(CONFIG_OPTIONS);
        let config_system = config.get_string_or_default(CONFIG_SYSTEM);
        let max_tokens = config.get_integer_or_default(CONFIG_MAX_TOKENS);
        let temperature = config.get_number_or_default(CONFIG_TEMPERATURE);
        let top_p = config.get_number_or_default(CONFIG_TOP_P);

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
                    max_tokens,
                    temperature,
                    top_p,
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
                    max_tokens,
                    temperature,
                    top_p,
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
    #[allow(clippy::too_many_arguments)]
    async fn process_openai(
        &mut self,
        ctx: AgentContext,
        prompt: &str,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_system: String,
        max_tokens: i64,
        temperature: f64,
        top_p: f64,
    ) -> Result<(), AgentError> {
        let client = self.openai_manager.get_client(self.ma())?;

        // Build the prompt with system message if provided
        let full_prompt = if !config_system.is_empty() {
            format!("{}\n\n{}", config_system, prompt)
        } else {
            prompt.to_string()
        };

        let mut request = serde_json::json!({
            "model": model_name,
            "prompt": full_prompt,
        });

        openai_client::merge_options(&mut request, &config_options)?;
        if max_tokens > 0 {
            request["max_tokens"] = max_tokens.into();
        }
        if temperature >= 0.0 {
            request["temperature"] = temperature.into();
        }
        if top_p >= 0.0 {
            request["top_p"] = top_p.into();
        }

        let res: openai_client::CompletionResponse = client
            .post_json(&client.completions_url(), &request)
            .await?;

        let message = Message::assistant(res.choices[0].text.clone());
        self.output(ctx.clone(), PORT_MESSAGE.to_string(), message.into())
            .await?;

        let out_response = AgentValue::from_serialize(&res)?;
        self.output(ctx, PORT_RESPONSE.to_string(), out_response)
            .await?;

        Ok(())
    }

    #[cfg(feature = "ollama")]
    #[allow(clippy::too_many_arguments)]
    async fn process_ollama(
        &mut self,
        ctx: AgentContext,
        prompt: &str,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
        config_system: String,
        max_tokens: i64,
        temperature: f64,
        top_p: f64,
    ) -> Result<(), AgentError> {
        let client = self.ollama_manager.get_client(self.ma())?;

        let use_context = self.configs()?.get_bool_or_default(CONFIG_USE_CONTEXT);

        let mut request = serde_json::json!({
            "model": model_name,
            "prompt": prompt,
            "stream": false,
        });

        if !config_system.is_empty() {
            request["system"] = serde_json::Value::String(config_system);
        }

        ollama_client::merge_options(&mut request, &config_options)?;
        if max_tokens > 0 || temperature >= 0.0 || top_p >= 0.0 {
            if !request.get("options").is_some_and(|v| v.is_object()) {
                request["options"] = serde_json::json!({});
            }
            let opts = request["options"].as_object_mut().unwrap();
            if max_tokens > 0 {
                opts.insert("num_predict".into(), max_tokens.into());
            }
            if temperature >= 0.0 {
                opts.insert("temperature".into(), temperature.into());
            }
            if top_p >= 0.0 {
                opts.insert("top_p".into(), top_p.into());
            }
        }

        if use_context && let Some(context) = &self.context {
            request["context"] = serde_json::to_value(context).unwrap_or(serde_json::Value::Null);
        }

        let res: ollama_client::GenerateResponse =
            client.post_json(&client.generate_url(), &request).await?;

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
