#![cfg(feature = "yaml")]

use std::vec;

use modular_agent_core::{
    AsModule, Error, ModularAgent, ModuleContext, ModuleData, ModuleOutput, ModuleSpec, Result,
    Value, async_trait, modular_agent,
};

const CATEGORY: &str = "Std/YAML";

const PORT_VALUE: &str = "value";
const PORT_YAML: &str = "yaml";

// To YAML
#[modular_agent(
    title = "To YAML",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    outputs = [PORT_YAML],
    hint(color=5),
)]
struct ToYamlModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for ToYamlModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let yaml =
            serde_yaml_ng::to_string(&value).map_err(|e| Error::InvalidValue(e.to_string()))?;
        self.output(ctx, PORT_YAML, Value::string(yaml)).await?;
        Ok(())
    }
}

// From YAML
#[modular_agent(
    title = "From YAML",
    category = CATEGORY,
    inputs = [PORT_YAML],
    outputs = [PORT_VALUE],
    hint(color=5),
)]
struct FromYamlModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for FromYamlModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let s = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("not a string".to_string()))?;
        let v: serde_json::Value =
            serde_yaml_ng::from_str(s).map_err(|e| Error::InvalidValue(e.to_string()))?;
        let value = Value::from_json(v)?;
        self.output(ctx, PORT_VALUE, value).await?;
        Ok(())
    }
}
