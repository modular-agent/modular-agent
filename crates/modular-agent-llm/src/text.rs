use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use text_splitter::{ChunkConfig, TextSplitter};
use tokenizers::Tokenizer;
use unicode_normalization::UnicodeNormalization;

static CATEGORY: &str = "LLM/Text";

static PIN_ARRAY: &str = "array";
static PIN_TEXT: &str = "text";

static CONFIG_MAX_CHARACTERS: &str = "max_characters";
static CONFIG_MAX_TOKENS: &str = "max_tokens";
static CONFIG_TOKENIZER: &str = "tokenizer";

#[askit_agent(
    title="Split Text",
    category=CATEGORY,
    inputs=[PIN_TEXT],
    outputs=[PIN_ARRAY],
    integer_config(name=CONFIG_MAX_CHARACTERS, default=512),
)]
pub struct SplitTextAgent {
    data: AgentData,
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
        _pin: String,
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

        let text = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("Input must be a string".to_string()))?;

        let splitter = TextSplitter::new(max_characters);
        let chunks = splitter
            .chunk_indices(text)
            .map(|(start, t)| {
                AgentValue::array(vec![
                    AgentValue::integer(start as i64),
                    AgentValue::string(t),
                ])
            })
            .collect::<Vec<_>>();

        self.try_output(ctx.clone(), PIN_ARRAY, AgentValue::array(chunks))?;

        Ok(())
    }
}

#[askit_agent(
    title="Split Text by Tokens",
    category=CATEGORY,
    inputs=[PIN_TEXT],
    outputs=[PIN_ARRAY],
    integer_config(name=CONFIG_MAX_TOKENS, default=4000),
    string_config(name=CONFIG_TOKENIZER, default="BAAI/bge-m3")
)]
pub struct SplitTextByTokensAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for SplitTextByTokensAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
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

        let text = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("Input must be a string".to_string()))?;

        let tokenizer = Tokenizer::from_pretrained(&tokenizer_model, None)
            .map_err(|e| AgentError::InvalidConfig(format!("Failed to load tokenizer: {}", e)))?;

        let splitter = TextSplitter::new(ChunkConfig::new(max_tokens).with_sizer(tokenizer));
        let chunks = splitter
            .chunk_indices(text)
            .map(|(start, t)| {
                AgentValue::array(vec![
                    AgentValue::integer(start as i64),
                    AgentValue::string(t),
                ])
            })
            .collect::<Vec<_>>();

        self.try_output(ctx.clone(), PIN_ARRAY, AgentValue::array(chunks))?;

        Ok(())
    }
}

#[askit_agent(
    title="NFKC",
    category=CATEGORY,
    inputs=[PIN_TEXT],
    outputs=[PIN_TEXT],
)]
pub struct NFKCAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for NFKCAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let text = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("Input must be a string".to_string()))?;

        let nfkc_text: String = text.nfkc().collect();

        self.try_output(ctx.clone(), PIN_TEXT, AgentValue::string(nfkc_text))?;

        Ok(())
    }
}
