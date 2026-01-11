use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use icu_normalizer::{ComposingNormalizer, ComposingNormalizerBorrowed};
use im::{Vector, vector};
use text_splitter::{ChunkConfig, TextSplitter};
use tokenizers::Tokenizer;

const CATEGORY: &str = "LLM/Doc";

const PIN_CHUNKS: &str = "chunks";
const PIN_DOC: &str = "doc";
const PIN_STRING: &str = "string";

const CONFIG_MAX_CHARACTERS: &str = "max_characters";
const CONFIG_MAX_TOKENS: &str = "max_tokens";
const CONFIG_TOKENIZER: &str = "tokenizer";

#[askit_agent(
    title="NFKC",
    category=CATEGORY,
    inputs=[PIN_STRING, PIN_DOC],
    outputs=[PIN_STRING, PIN_DOC],
)]
pub struct NFKCAgent {
    data: AgentData,
    normalizer: Option<ComposingNormalizerBorrowed<'static>>,
}

#[async_trait]
impl AsAgent for NFKCAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            normalizer: None,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        let normalizer = ComposingNormalizer::new_nfkc();
        self.normalizer = Some(normalizer);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.normalizer = None;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if pin == PIN_STRING {
            let s = value.as_str().unwrap_or("");
            if s.is_empty() {
                return self.output(ctx.clone(), PIN_STRING, value).await;
            }
            let nfkc_text = self
                .normalizer
                .as_ref()
                .map(|n| n.normalize(s))
                .unwrap_or_default();
            return self.output(ctx.clone(), PIN_STRING, AgentValue::string(nfkc_text)).await;
        }

        if pin == PIN_DOC {
            if value.is_object() {
                let text = value.get_str("text").unwrap_or_default();
                if text.is_empty() {
                    return self.output(ctx.clone(), PIN_DOC, value).await;
                }
                let nfkc_text = self
                    .normalizer
                    .as_ref()
                    .map(|n| n.normalize(text))
                    .unwrap_or_default();
                let mut output = value.clone();
                output.set("text".to_string(), AgentValue::string(nfkc_text))?;
                return self.output(ctx.clone(), PIN_DOC, output).await;
            } else {
                return Err(AgentError::InvalidValue(
                    "Input must be an object with a text field".to_string(),
                ));
            }
        }

        Err(AgentError::InvalidPin(pin))
    }
}

#[askit_agent(
    title="Split Text",
    category=CATEGORY,
    inputs=[PIN_STRING, PIN_DOC],
    outputs=[PIN_CHUNKS, PIN_DOC],
    integer_config(name=CONFIG_MAX_CHARACTERS, default=512),
)]
pub struct SplitTextAgent {
    data: AgentData,
}

impl SplitTextAgent {
    fn split_into_chunks(&self, text: &str, max_characters: usize) -> Vector<AgentValue> {
        TextSplitter::new(max_characters)
            .chunk_indices(text)
            .map(|(start, t)| {
                AgentValue::array(vector![
                    AgentValue::integer(start as i64),
                    AgentValue::string(t),
                ])
            })
            .collect::<Vector<_>>()
    }
}

#[async_trait]
impl AsAgent for SplitTextAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let max_characters = self
            .configs()?
            .get_integer_or_default(CONFIG_MAX_CHARACTERS) as usize;
        if max_characters == 0 {
            return Err(AgentError::InvalidConfig(
                "max_characters must be greater than 0".to_string(),
            ));
        }

        if pin == PIN_STRING {
            let text = value.as_str().unwrap_or("");
            if text.is_empty() {
                return self.output(ctx.clone(), PIN_CHUNKS, AgentValue::array_default()).await;
            }
            let chunks = self.split_into_chunks(text, max_characters);
            return self.output(ctx.clone(), PIN_CHUNKS, AgentValue::array(chunks)).await;
        }

        if pin == PIN_DOC {
            if value.is_object() {
                let text = value.get_str("text").unwrap_or("");
                let chunks = if text.is_empty() {
                    Vector::new()
                } else {
                    self.split_into_chunks(text, max_characters)
                };
                let mut output = value.clone();
                output.set("chunks".to_string(), AgentValue::array(chunks))?;
                return self.output(ctx.clone(), PIN_DOC, output).await;
            }
        }

        Err(AgentError::InvalidPin(pin))
    }
}

#[askit_agent(
    title="Split Text by Tokens",
    category=CATEGORY,
    inputs=[PIN_STRING, PIN_DOC],
    outputs=[PIN_CHUNKS, PIN_DOC],
    integer_config(name=CONFIG_MAX_TOKENS, default=500),
    string_config(name=CONFIG_TOKENIZER, default="nomic-ai/nomic-embed-text-v2-moe")
)]
pub struct SplitTextByTokensAgent {
    data: AgentData,
    splitter: Option<TextSplitter<Tokenizer>>,
}

impl SplitTextByTokensAgent {
    fn split_into_chunks(
        &mut self,
        text: &str,
        max_tokens: usize,
        tokenizer_model: &str,
    ) -> Result<Vector<AgentValue>, AgentError> {
        if self.splitter.is_none() {
            let tokenizer = Tokenizer::from_pretrained(tokenizer_model, None).map_err(|e| {
                AgentError::InvalidConfig(format!("Failed to load tokenizer: {}", e))
            })?;
            let splitter = TextSplitter::new(ChunkConfig::new(max_tokens).with_sizer(tokenizer));
            self.splitter = Some(splitter);
        }
        Ok(self
            .splitter
            .as_ref()
            .unwrap()
            .chunk_indices(text)
            .map(|(start, t)| {
                AgentValue::array(vector![
                    AgentValue::integer(start as i64),
                    AgentValue::string(t),
                ])
            })
            .collect::<Vector<_>>())
    }
}

#[async_trait]
impl AsAgent for SplitTextByTokensAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            splitter: None,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        self.splitter = None;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.splitter = None;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let max_tokens = self.configs()?.get_integer_or_default(CONFIG_MAX_TOKENS) as usize;
        if max_tokens == 0 {
            return Err(AgentError::InvalidConfig(
                "max_tokens must be greater than 0".to_string(),
            ));
        }

        let tokenizer_model = self.configs()?.get_string_or_default(CONFIG_TOKENIZER);
        if tokenizer_model.is_empty() {
            return Err(AgentError::InvalidConfig(
                "tokenizer must be a non-empty string".to_string(),
            ));
        }

        if pin == PIN_STRING {
            let text = value.as_str().unwrap_or("");
            if text.is_empty() {
                return self.output(ctx.clone(), PIN_CHUNKS, AgentValue::array_default()).await;
            }

            let chunks = self.split_into_chunks(text, max_tokens, &tokenizer_model)?;
            return self.output(ctx.clone(), PIN_CHUNKS, AgentValue::array(chunks)).await;
        }

        if pin == PIN_DOC {
            let text = value.get_str("text").unwrap_or("");
            let chunks = if text.is_empty() {
                Vector::new()
            } else {
                self.split_into_chunks(text, max_tokens, &tokenizer_model)?
            };
            let mut output = value.clone();
            output.set("chunks".to_string(), AgentValue::array(chunks))?;
            return self.output(ctx.clone(), PIN_DOC, output).await;
        }

        Err(AgentError::InvalidPin(pin))
    }
}
