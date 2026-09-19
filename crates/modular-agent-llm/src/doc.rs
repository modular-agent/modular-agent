use std::vec;

use icu_normalizer::{ComposingNormalizer, ComposingNormalizerBorrowed};
use im::vector;
use modular_agent_core::{
    AsModule, Error, ModularAgent, Module, ModuleContext, ModuleData, ModuleOutput, ModuleSpec,
    Result, Value, async_trait, modular_agent,
};
use text_splitter::{ChunkConfig, TextSplitter};
use tokenizers::Tokenizer;

const CATEGORY: &str = "LLM/Doc";

const PORT_CHUNKS: &str = "chunks";
const PORT_DOC: &str = "doc";
const PORT_STRING: &str = "string";

const CONFIG_MAX_CHARACTERS: &str = "max_characters";
const CONFIG_MAX_TOKENS: &str = "max_tokens";
const CONFIG_TOKENIZER: &str = "tokenizer";

#[modular_agent(
    title="NFKC",
    category=CATEGORY,
    inputs=[PORT_STRING, PORT_DOC],
    outputs=[PORT_STRING, PORT_DOC],
)]
pub struct NFKCModule {
    data: ModuleData,
    normalizer: Option<ComposingNormalizerBorrowed<'static>>,
}

#[async_trait]
impl AsModule for NFKCModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            normalizer: None,
        })
    }

    async fn start(&mut self) -> Result<()> {
        let normalizer = ComposingNormalizer::new_nfkc();
        self.normalizer = Some(normalizer);
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.normalizer = None;
        Ok(())
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        if port == PORT_STRING {
            let s = value.as_str().unwrap_or("");
            if s.is_empty() {
                return self.output(ctx.clone(), PORT_STRING, value).await;
            }
            let nfkc_text = self
                .normalizer
                .as_ref()
                .map(|n| n.normalize(s))
                .unwrap_or_default();
            return self
                .output(ctx.clone(), PORT_STRING, Value::string(nfkc_text))
                .await;
        }

        if port == PORT_DOC {
            if value.is_object() {
                let text = value.get_str("text").unwrap_or_default();
                if text.is_empty() {
                    return self
                        .output(ctx.clone(), PORT_DOC, Value::string_default())
                        .await;
                }
                let nfkc_text = self
                    .normalizer
                    .as_ref()
                    .map(|n| n.normalize(text))
                    .unwrap_or_default();
                let mut output = value.clone();
                output.set("text".to_string(), Value::string(nfkc_text))?;
                return self.output(ctx.clone(), PORT_DOC, output).await;
            } else {
                return Err(Error::InvalidValue(
                    "Input must be an object with a text field".to_string(),
                ));
            }
        }

        Err(Error::InvalidPin(port))
    }
}

#[modular_agent(
    title="Split Text",
    category=CATEGORY,
    inputs=[PORT_STRING, PORT_DOC],
    outputs=[PORT_CHUNKS, PORT_DOC],
    integer_config(name=CONFIG_MAX_CHARACTERS, default=512),
)]
pub struct SplitTextModule {
    data: ModuleData,
}

impl SplitTextModule {
    fn split_into_chunks(&self, text: &str, max_characters: usize) -> Vec<(usize, String)> {
        TextSplitter::new(max_characters)
            .chunk_indices(text)
            .map(|(offset, chunk)| (offset, chunk.to_string()))
            .collect()
    }
}

#[async_trait]
impl AsModule for SplitTextModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        let max_characters = self
            .configs()?
            .get_integer_or_default(CONFIG_MAX_CHARACTERS) as usize;
        if max_characters == 0 {
            return Err(Error::InvalidConfig(
                "max_characters must be greater than 0".to_string(),
            ));
        }

        if port == PORT_STRING {
            let text = value.as_str().unwrap_or("");
            if text.is_empty() {
                return self
                    .output(ctx.clone(), PORT_CHUNKS, Value::array_default())
                    .await;
            }
            let chunks = self.split_into_chunks(text, max_characters);
            self.output(
                ctx.clone(),
                PORT_CHUNKS,
                Value::array(
                    chunks
                        .into_iter()
                        .map(|(offset, chunk)| {
                            Value::array(vector![
                                Value::integer(offset as i64),
                                Value::string(chunk)
                            ])
                        })
                        .collect::<Vec<_>>()
                        .into(),
                ),
            )
            .await?;
            return Ok(());
        }

        if port == PORT_DOC {
            if value.is_object() {
                let text = value.get_str("text").unwrap_or("");
                if text.is_empty() {
                    return self
                        .output(ctx.clone(), PORT_DOC, Value::array_default())
                        .await;
                }
                let chunks = self.split_into_chunks(text, max_characters);
                self.output(
                    ctx,
                    PORT_DOC,
                    Value::array(
                        chunks
                            .into_iter()
                            .map(|(offset, chunk)| {
                                let mut output = value.clone();
                                output.set("offset".to_string(), Value::integer(offset as i64))?;
                                output.set("text".to_string(), Value::string(chunk))?;
                                Ok(output)
                            })
                            .collect::<Result<Vec<_>>>()?
                            .into(),
                    ),
                )
                .await?;
            }
            return Ok(());
        }

        Err(Error::InvalidPin(port))
    }
}

#[modular_agent(
    title="Split Text by Tokens",
    category=CATEGORY,
    inputs=[PORT_STRING, PORT_DOC],
    outputs=[PORT_CHUNKS, PORT_DOC],
    integer_config(name=CONFIG_MAX_TOKENS, default=500),
    string_config(name=CONFIG_TOKENIZER, default="nomic-ai/nomic-embed-text-v2-moe"),
    hint(width = 2, height = 2),
)]
pub struct SplitTextByTokensModule {
    data: ModuleData,
    splitter: Option<TextSplitter<Tokenizer>>,
}

impl SplitTextByTokensModule {
    fn split_into_chunks(
        &mut self,
        text: &str,
        max_tokens: usize,
        tokenizer_model: &str,
    ) -> Result<Vec<(usize, String)>> {
        if self.splitter.is_none() {
            let tokenizer = Tokenizer::from_pretrained(tokenizer_model, None)
                .map_err(|e| Error::InvalidConfig(format!("Failed to load tokenizer: {}", e)))?;
            let splitter = TextSplitter::new(ChunkConfig::new(max_tokens).with_sizer(tokenizer));
            self.splitter = Some(splitter);
        }
        Ok(self
            .splitter
            .as_ref()
            .unwrap()
            .chunk_indices(text)
            .map(|(offset, chunk)| (offset, chunk.to_string()))
            .collect())
    }
}

#[async_trait]
impl AsModule for SplitTextByTokensModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            splitter: None,
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        self.splitter = None;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.splitter = None;
        Ok(())
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        let max_tokens = self.configs()?.get_integer_or_default(CONFIG_MAX_TOKENS) as usize;
        if max_tokens == 0 {
            return Err(Error::InvalidConfig(
                "max_tokens must be greater than 0".to_string(),
            ));
        }

        let tokenizer_model = self.configs()?.get_string_or_default(CONFIG_TOKENIZER);
        if tokenizer_model.is_empty() {
            return Err(Error::InvalidConfig(
                "tokenizer must be a non-empty string".to_string(),
            ));
        }

        if port == PORT_STRING {
            let text = value.as_str().unwrap_or("");
            if text.is_empty() {
                return self
                    .output(ctx.clone(), PORT_CHUNKS, Value::array_default())
                    .await;
            }

            let chunks = self.split_into_chunks(text, max_tokens, &tokenizer_model)?;
            self.output(
                ctx.clone(),
                PORT_CHUNKS,
                Value::array(
                    chunks
                        .into_iter()
                        .map(|(offset, chunk)| {
                            Value::array(vector![
                                Value::integer(offset as i64),
                                Value::string(chunk)
                            ])
                        })
                        .collect::<Vec<_>>()
                        .into(),
                ),
            )
            .await?;
            return Ok(());
        }

        if port == PORT_DOC {
            let text = value.get_str("text").unwrap_or("");
            if text.is_empty() {
                return self
                    .output(ctx.clone(), PORT_DOC, Value::array_default())
                    .await;
            }

            let chunks = self.split_into_chunks(text, max_tokens, &tokenizer_model)?;
            self.output(
                ctx,
                PORT_DOC,
                Value::array(
                    chunks
                        .into_iter()
                        .map(|(offset, chunk)| {
                            let mut output = value.clone();
                            output.set("offset".to_string(), Value::integer(offset as i64))?;
                            output.set("text".to_string(), Value::string(chunk))?;
                            Ok(output)
                        })
                        .collect::<Result<Vec<_>>>()?
                        .into(),
                ),
            )
            .await?;
            return Ok(());
        }

        Err(Error::InvalidPin(port))
    }
}
