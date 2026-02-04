#![cfg(feature = "ollama")]

use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    ModularAgent, async_trait, modular_agent,
};

use crate::ollama_client::OllamaManager;

const CATEGORY: &str = "LLM/Ollama";

const PORT_MODEL_INFO: &str = "model_info";
const PORT_MODEL_LIST: &str = "model_list";
const PORT_MODEL_NAME: &str = "model_name";
const PORT_UNIT: &str = "unit";

// Ollama List Local Models
#[modular_agent(
    title="List Local Models",
    category=CATEGORY,
    inputs=[PORT_UNIT],
    outputs=[PORT_MODEL_LIST],
)]
pub struct OllamaListLocalModelsAgent {
    data: AgentData,
    manager: OllamaManager,
}

#[async_trait]
impl AsAgent for OllamaListLocalModelsAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            manager: OllamaManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let client = self.manager.get_client(self.ma())?;
        let model_list = client
            .list_local_models()
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;
        let model_list = AgentValue::from_serialize(&model_list)?;

        self.output(ctx.clone(), PORT_MODEL_LIST, model_list)
            .await?;
        Ok(())
    }
}

// Ollama Show Model Info
#[modular_agent(
    title="Show Model Info",
    category=CATEGORY,
    inputs=[PORT_MODEL_NAME],
    outputs=[PORT_MODEL_INFO],
)]
pub struct OllamaShowModelInfoAgent {
    data: AgentData,
    manager: OllamaManager,
}

#[async_trait]
impl AsAgent for OllamaShowModelInfoAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            manager: OllamaManager::new(),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let model_name = value.as_str().unwrap_or("");
        if model_name.is_empty() {
            return Ok(());
        }

        let client = self.manager.get_client(self.ma())?;
        let model_info = client
            .show_model_info(model_name.to_string())
            .await
            .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;
        let model_info = AgentValue::from_serialize(&model_info)?;

        self.output(ctx.clone(), PORT_MODEL_INFO, model_info)
            .await?;
        Ok(())
    }
}
