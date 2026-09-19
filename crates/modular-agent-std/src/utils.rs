use std::vec;

use modular_agent_core::{
    AsModule, ModularAgent, Module, ModuleContext, ModuleData, ModuleOutput, ModuleSpec, Result,
    Value, async_trait, modular_agent,
};

const CATEGORY: &str = "Std/Utils";

const PORT_VALUE: &str = "value";
const PORT_RESET: &str = "reset";
const PORT_COUNT: &str = "count";

const DISPLAY_COUNT: &str = "count";

/// Counter
#[modular_agent(
    title = "Counter",
    category = CATEGORY,
    inputs = [PORT_VALUE, PORT_RESET],
    outputs = [PORT_COUNT],
    integer_config(
        name = DISPLAY_COUNT,
        readonly,
        hide_title,
    ),
    hint(color=6),
)]
struct CounterModule {
    data: ModuleData,
    count: i64,
}

#[async_trait]
impl AsModule for CounterModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            count: 0,
        })
    }

    async fn start(&mut self) -> Result<()> {
        self.count = 0;
        // The running count is only emitted, never persisted; this write
        // clears a count an older version saved into the patch.
        self.set_config(DISPLAY_COUNT.to_string(), Value::integer(0))?;
        self.emit_config_updated(DISPLAY_COUNT, Value::integer(0));
        Ok(())
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, _value: Value) -> Result<()> {
        if port == PORT_RESET {
            self.count = 0;
        } else if port == PORT_VALUE {
            self.count += 1;
        }
        self.output(ctx, PORT_COUNT, Value::integer(self.count))
            .await?;
        self.emit_config_updated(DISPLAY_COUNT, Value::integer(self.count));

        Ok(())
    }
}
