use modular_agent_core::{
    AsModule, ModularAgent, Module, ModuleContext, ModuleData, ModuleOutput, ModuleSpec, Result,
    Value, async_trait, modular_agent,
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
    hint(width = 2, height = 1),
)]
pub struct OllamaListLocalModelsModule {
    data: ModuleData,
    manager: OllamaManager,
}

#[async_trait]
impl AsModule for OllamaListLocalModelsModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            manager: OllamaManager::new(),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, _value: Value) -> Result<()> {
        let client = self.manager.get_client(self.ma())?;
        let model_list = client.list_local_models().await?;
        let model_list = Value::from_serialize(&model_list)?;

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
    hint(width = 2, height = 1),
)]
pub struct OllamaShowModelInfoModule {
    data: ModuleData,
    manager: OllamaManager,
}

#[async_trait]
impl AsModule for OllamaShowModelInfoModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            manager: OllamaManager::new(),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let model_name = value.as_str().unwrap_or("");
        if model_name.is_empty() {
            return Ok(());
        }

        let client = self.manager.get_client(self.ma())?;
        let model_info = client.show_model_info(model_name).await?;
        let model_info = Value::from_serialize(&model_info)?;

        self.output(ctx.clone(), PORT_MODEL_INFO, model_info)
            .await?;
        Ok(())
    }
}
