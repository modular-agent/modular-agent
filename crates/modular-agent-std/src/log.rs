use modular_agent_core::{
    AsModule, Error, ModularAgent, Module, ModuleContext, ModuleData, ModuleLogLevel, ModuleOutput,
    ModuleSpec, Result, Value, async_trait, modular_agent,
};

use crate::string::{handlebars_new, template_data};

const CATEGORY: &str = "Std/Log";

const PORT_VALUE: &str = "value";

const CONFIG_LEVEL: &str = "level";
const CONFIG_MESSAGE: &str = "message";

/// Writes a record to the host's log: stderr for `ma`, the log view for the desktop app.
///
/// `message` is a handlebars template like Template String, so `{{value.id}}` logs one
/// field instead of the whole value. `level` is one of error, warn, info, debug.
/// An error value from an `err` port renders as its message.
#[modular_agent(
    title = "Log",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    string_config(name = CONFIG_LEVEL, default = "info"),
    string_config(name = CONFIG_MESSAGE, default = "{{value}}"),
)]
struct LogModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for LogModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, _ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;
        let level: ModuleLogLevel = config.get_string_or_default(CONFIG_LEVEL).parse()?;
        let template = config.get_string_or_default(CONFIG_MESSAGE);

        // Error values serialize to null, so hand the template the error's message instead.
        let value = match &value {
            Value::Error(e) => Value::string(e.to_string()),
            _ => value,
        };
        let message = handlebars_new()
            .render_template(&template, &template_data(&value))
            .map_err(|e| Error::InvalidValue(format!("Failed to render template: {}", e)))?;

        self.emit_log(level, message);
        Ok(())
    }
}
