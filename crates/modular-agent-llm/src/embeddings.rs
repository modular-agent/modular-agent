use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AgentValueMap,
    AsAgent, ModularAgent, async_trait, modular_agent,
};

use crate::provider::{ModelIdentifier, ProviderKind};

#[cfg(feature = "openai")]
use crate::openai_client;

#[cfg(feature = "ollama")]
use crate::ollama_client;

use im::{Vector, vector};

const CATEGORY: &str = "LLM";

const PORT_CHUNKS: &str = "chunks";
const PORT_DOC: &str = "doc";
const PORT_EMBEDDING: &str = "embedding";
const PORT_EMBEDDINGS: &str = "embeddings";
const PORT_STRING: &str = "string";

const CONFIG_MODEL: &str = "model";
const CONFIG_OPTIONS: &str = "options";

const DEFAULT_CONFIG_MODEL: &str = "openai/text-embedding-3-small";

/// Embeddings Agent that routes to different LLM providers based on model prefix.
///
/// # Model Format
/// - `openai/text-embedding-3-small` - Uses OpenAI API
/// - `ollama/nomic-embed-text` - Uses Ollama
/// - `openai/text-embedding-3-small` - Uses OpenAI
///
/// # Input Ports
/// - `string`: Single text input, outputs single embedding
/// - `chunks`: Array of (offset, text) pairs, outputs array of (offset, embedding) pairs
/// - `doc`: Document object(s) with `text` field, outputs with added `embedding` field
#[modular_agent(
    title = "Embeddings",
    category = CATEGORY,
    inputs = [PORT_STRING, PORT_CHUNKS, PORT_DOC],
    outputs = [PORT_EMBEDDING, PORT_EMBEDDINGS, PORT_DOC],
    string_config(name = CONFIG_MODEL, default = DEFAULT_CONFIG_MODEL),
    object_config(name = CONFIG_OPTIONS),
    hint(width = 2, height = 2),
)]
pub struct EmbeddingsAgent {
    data: AgentData,
    #[cfg(feature = "openai")]
    openai_manager: openai_client::OpenAIManager,
    #[cfg(feature = "ollama")]
    ollama_manager: ollama_client::OllamaManager,
}

#[async_trait]
impl AsAgent for EmbeddingsAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            #[cfg(feature = "openai")]
            openai_manager: openai_client::OpenAIManager::new(),
            #[cfg(feature = "ollama")]
            ollama_manager: ollama_client::OllamaManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config_model = self.configs()?.get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Err(AgentError::InvalidConfig("model is not set".to_string()));
        }

        // Parse model identifier to determine provider
        let model_id = ModelIdentifier::parse(&config_model)?;

        // Get common configs
        let config_options = self.configs()?.get_object_or_default(CONFIG_OPTIONS);

        // Route to appropriate provider
        match model_id.provider {
            #[cfg(feature = "claude")]
            ProviderKind::Claude => Err(AgentError::InvalidConfig(
                "Claude does not support embeddings. Use OpenAI or Ollama instead.".into(),
            )),
            #[cfg(feature = "openai")]
            ProviderKind::OpenAI => {
                self.process_openai(ctx, port, value, &model_id.model_name, config_options)
                    .await
            }
            #[cfg(feature = "ollama")]
            ProviderKind::Ollama => {
                self.process_ollama(ctx, port, value, &model_id.model_name, config_options)
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

impl EmbeddingsAgent {
    #[cfg(feature = "openai")]
    async fn process_openai(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
    ) -> Result<(), AgentError> {
        let client = self.openai_manager.get_client(self.ma())?;

        if port == PORT_STRING {
            let text = value.as_str().unwrap_or_default();
            if text.is_empty() {
                return Err(AgentError::InvalidValue(
                    "Input text is an empty string".to_string(),
                ));
            }
            let embeddings = openai_client::generate_embeddings(
                &client,
                vec![text.to_string()],
                model_name,
                &config_options,
            )
            .await?;
            if embeddings.len() != 1 {
                return Err(AgentError::Other(
                    "Expected exactly one embedding for single string input".to_string(),
                ));
            }
            return self
                .output(
                    ctx,
                    PORT_EMBEDDING,
                    AgentValue::tensor(embeddings.into_iter().next().unwrap()),
                )
                .await;
        }

        if port == PORT_CHUNKS {
            let (offsets, texts) = Self::parse_chunks(&value)?;
            if texts.is_empty() {
                return self
                    .output(ctx.clone(), PORT_EMBEDDINGS, AgentValue::array_default())
                    .await;
            }
            let embeddings =
                openai_client::generate_embeddings(&client, texts, model_name, &config_options)
                    .await?;
            let embedding_values_with_offsets =
                Self::zip_embeddings_with_offsets(offsets, embeddings);
            return self
                .output(
                    ctx,
                    PORT_EMBEDDINGS,
                    AgentValue::array(embedding_values_with_offsets),
                )
                .await;
        }

        if port == PORT_DOC {
            let (texts, indices, is_single) = Self::extract_texts_from_doc(&value)?;
            if texts.is_empty() {
                if is_single {
                    return Err(AgentError::InvalidValue(
                        "No text found in the document".to_string(),
                    ));
                }
                return self
                    .output(ctx.clone(), PORT_DOC, AgentValue::array_default())
                    .await;
            }

            let embeddings =
                openai_client::generate_embeddings(&client, texts, model_name, &config_options)
                    .await?;

            return self
                .output_doc_embeddings(ctx, value, embeddings, indices, is_single)
                .await;
        }

        Err(AgentError::InvalidPin(port))
    }

    #[cfg(feature = "ollama")]
    async fn process_ollama(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
        model_name: &str,
        config_options: AgentValueMap<String, AgentValue>,
    ) -> Result<(), AgentError> {
        let client = self.ollama_manager.get_client(self.ma())?;

        let options_json = if config_options.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::to_value(&config_options)
                .map_err(|e| AgentError::InvalidConfig(format!("Invalid JSON in options: {}", e)))?
        };

        if port == PORT_STRING {
            let text = value.as_str().unwrap_or_default();
            if text.is_empty() {
                return Err(AgentError::InvalidValue(
                    "Input text is an empty string".to_string(),
                ));
            }
            let embeddings = client
                .generate_embeddings(vec![text.to_string()], model_name, &options_json)
                .await?;
            if embeddings.len() != 1 {
                return Err(AgentError::Other(
                    "Expected exactly one embedding for single string input".to_string(),
                ));
            }
            return self
                .output(
                    ctx,
                    PORT_EMBEDDING,
                    AgentValue::tensor(embeddings.into_iter().next().unwrap()),
                )
                .await;
        }

        if port == PORT_CHUNKS {
            let (offsets, texts) = Self::parse_chunks(&value)?;
            if texts.is_empty() {
                return self
                    .output(ctx.clone(), PORT_EMBEDDINGS, AgentValue::array_default())
                    .await;
            }
            let embeddings = client
                .generate_embeddings(texts, model_name, &options_json)
                .await?;
            let embedding_values_with_offsets =
                Self::zip_embeddings_with_offsets(offsets, embeddings);
            return self
                .output(
                    ctx,
                    PORT_EMBEDDINGS,
                    AgentValue::array(embedding_values_with_offsets),
                )
                .await;
        }

        if port == PORT_DOC {
            let (texts, indices, is_single) = Self::extract_texts_from_doc(&value)?;
            if texts.is_empty() {
                if is_single {
                    return Err(AgentError::InvalidValue(
                        "No text found in the document".to_string(),
                    ));
                }
                return self
                    .output(ctx.clone(), PORT_DOC, AgentValue::array_default())
                    .await;
            }

            let embeddings = client
                .generate_embeddings(texts, model_name, &options_json)
                .await?;

            return self
                .output_doc_embeddings(ctx, value, embeddings, indices, is_single)
                .await;
        }

        Err(AgentError::InvalidPin(port))
    }

    // Helper functions

    fn parse_chunks(value: &AgentValue) -> Result<(Vec<i64>, Vec<String>), AgentError> {
        if !value.is_array() {
            return Err(AgentError::InvalidValue(
                "Input must be an array of strings".to_string(),
            ));
        }
        let mut offsets = vec![];
        let mut texts = vec![];
        for item in value.as_array().unwrap().iter() {
            let arr = item.as_array().ok_or_else(|| {
                AgentError::InvalidValue("Input chunks must be (offset, string) pairs".to_string())
            })?;
            if arr.len() != 2 {
                return Err(AgentError::InvalidValue(
                    "Input chunks must be (offset, string) pairs".to_string(),
                ));
            }
            let offset = arr[0].as_i64().ok_or_else(|| {
                AgentError::InvalidValue("Input chunks must be (offset, string) pairs".to_string())
            })?;
            let text = arr[1]
                .as_str()
                .ok_or_else(|| {
                    AgentError::InvalidValue(
                        "Input chunks must be (offset, string) pairs".to_string(),
                    )
                })?
                .to_string();
            if !text.is_empty() {
                offsets.push(offset);
                texts.push(text);
            }
        }
        Ok((offsets, texts))
    }

    fn zip_embeddings_with_offsets(
        offsets: Vec<i64>,
        embeddings: Vec<Vec<f32>>,
    ) -> Vector<AgentValue> {
        offsets
            .into_iter()
            .zip(embeddings)
            .map(|(offset, emb)| {
                AgentValue::array(vector![
                    AgentValue::integer(offset),
                    AgentValue::tensor(emb)
                ])
            })
            .collect()
    }

    fn extract_texts_from_doc(
        value: &AgentValue,
    ) -> Result<(Vec<String>, Vec<i64>, bool), AgentError> {
        let mut texts = vec![];
        let mut indices = vec![];
        let is_single = value.is_object();

        if is_single {
            let text = value.get_str("text").unwrap_or_default();
            if !text.is_empty() {
                texts.push(text.to_string());
                indices.push(0);
            }
        } else if value.is_array() {
            for (index, item) in value.as_array().unwrap().iter().enumerate() {
                let text = item.get_str("text").unwrap_or_default();
                if !text.is_empty() {
                    texts.push(text.to_string());
                    indices.push(index as i64);
                }
            }
        } else {
            return Err(AgentError::InvalidValue(
                "Input must be a document object or an array of document objects".to_string(),
            ));
        }

        Ok((texts, indices, is_single))
    }

    async fn output_doc_embeddings(
        &mut self,
        ctx: AgentContext,
        value: AgentValue,
        embeddings: Vec<Vec<f32>>,
        indices: Vec<i64>,
        is_single: bool,
    ) -> Result<(), AgentError> {
        if embeddings.len() != indices.len() {
            return Err(AgentError::Other(
                "Mismatch between number of embeddings and texts".to_string(),
            ));
        }

        if is_single {
            let embedding = embeddings.into_iter().next().unwrap();
            let mut output = value.clone();
            output.set("embedding".to_string(), AgentValue::tensor(embedding))?;
            return self.output(ctx.clone(), PORT_DOC, output).await;
        } else {
            let mut arr = value.clone().into_array().unwrap();
            for i in 0..embeddings.len() {
                let embedding = &embeddings[i];
                let index = indices[i];
                arr[index as usize].set(
                    "embedding".to_string(),
                    AgentValue::tensor(embedding.clone()),
                )?;
            }
            return self
                .output(ctx.clone(), PORT_DOC, AgentValue::array(arr))
                .await;
        }
    }
}
