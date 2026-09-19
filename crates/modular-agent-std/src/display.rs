use std::vec;

use im::hashmap;
use modular_agent_core::{
    AsModule, Error, ModularAgent, Module, ModuleContext, ModuleData, ModuleOutput, ModuleSpec,
    Result, Value, async_trait, modular_agent,
};

const CATEGORY: &str = "Std/Display";

const PORT_VALUE: &str = "value";

const DISPLAY_VALUE: &str = "value";

const CONFIG_SAVE_VALUE: &str = "save_value";

// Display Value
#[modular_agent(
    kind = "Display",
    title = "Display Value",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    custom_config(
        name = DISPLAY_VALUE,
        readonly,
        type_="*",
        default=Value::unit(),
        hide_title,
    ),
    boolean_config(
        name = CONFIG_SAVE_VALUE,
        title = "Save Value",
        description = "Persist the displayed value in the patch file",
        detail,
    )
)]
struct DisplayValueModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for DisplayValueModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn start(&mut self) -> Result<()> {
        Ok(())
    }

    async fn process(&mut self, _ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        if self.configs()?.get_bool_or_default(CONFIG_SAVE_VALUE) {
            self.set_config(DISPLAY_VALUE.to_string(), value.clone())?;
        } else {
            self.set_config(DISPLAY_VALUE.to_string(), Value::unit())?;
        }
        self.emit_config_updated(DISPLAY_VALUE, value);
        Ok(())
    }
}

// Debug Value
#[modular_agent(
    kind = "Display",
    title = "Debug Value",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    object_config(
        name = DISPLAY_VALUE,
        readonly,
        hide_title,
    ),
    boolean_config(
        name = CONFIG_SAVE_VALUE,
        title = "Save Value",
        description = "Persist the displayed value in the patch file",
        detail,
    )
)]
struct DebugValueModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for DebugValueModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let ctx_json =
            serde_json::to_value(&ctx).map_err(|e| Error::InvalidValue(e.to_string()))?;
        let ctx = Value::from_json(ctx_json)?;
        let debug_value = Value::object(hashmap! { "ctx".into() => ctx, "value".into() => value });
        if self.configs()?.get_bool_or_default(CONFIG_SAVE_VALUE) {
            self.set_config(DISPLAY_VALUE.to_string(), debug_value.clone())?;
        } else {
            self.set_config(DISPLAY_VALUE.to_string(), Value::object(hashmap! {}))?;
        }
        self.emit_config_updated(DISPLAY_VALUE, debug_value);
        Ok(())
    }
}
