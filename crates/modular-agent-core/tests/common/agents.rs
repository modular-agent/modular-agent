use modular_agent_core::{
    AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent, ModularAgent,
    async_trait, modular_agent,
};

const CATEGORY: &str = "Core/Utils";

const PORT_IN: &str = "in";
const PORT_OUT: &str = "out";
const PORT_RESET: &str = "reset";
const PORT_COUNT: &str = "count";
const CONFIG_INITIAL_COUNT: &str = "initial_count";
const GLOBAL_STRING: &str = "global_string";
pub const CONFIG_DYN: &str = "dyn";
pub const PORT_DYN_OUT: &str = "dyn_out";

/// Counter
#[modular_agent(
    title = "Counter",
    category = CATEGORY,
    inputs = [PORT_IN, PORT_RESET],
    outputs = [PORT_COUNT],
    integer_config(name = CONFIG_INITIAL_COUNT, default = 1),
    string_global_config(name = GLOBAL_STRING, default = "gs"),
)]
pub struct CounterAgent {
    data: AgentData,
    pub count: i64,
}

/// Emits "started", then sleeps for a long time without observing
/// cancellation. Used to verify that stop_agent does not block behind a
/// long-running process().
#[modular_agent(
    title = "Stuck Sleep",
    category = CATEGORY,
    inputs = [PORT_IN],
    outputs = [PORT_OUT],
)]
pub struct StuckSleepAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for StuckSleepAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        self.output(ctx.clone(), PORT_OUT, AgentValue::string("started"))
            .await?;
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        self.output(ctx, PORT_OUT, AgentValue::string("done")).await
    }
}

/// Emits "started", then waits on the context cancel token. Emits "aborted"
/// when the flow is cancelled, "done" if it times out after a long sleep.
#[modular_agent(
    title = "Cancel Wait",
    category = CATEGORY,
    inputs = [PORT_IN],
    outputs = [PORT_OUT],
)]
pub struct CancelWaitAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for CancelWaitAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        self.output(ctx.clone(), PORT_OUT, AgentValue::string("started"))
            .await?;
        let Some(token) = ctx.cancel_token().cloned() else {
            return Err(AgentError::Other("no cancel token in context".into()));
        };
        tokio::select! {
            _ = token.cancelled() => {
                self.output(ctx, PORT_OUT, AgentValue::string("aborted")).await
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                self.output(ctx, PORT_OUT, AgentValue::string("done")).await
            }
        }
    }
}

/// Mutates its spec in new(): adds a dynamic config and output port.
/// Models agents like ZipToObject that generate configs/ports at
/// construction time; the mutation must survive into the preset spec.
#[modular_agent(
    title = "Dyn Spec",
    category = CATEGORY,
    inputs = [PORT_IN],
    outputs = [PORT_OUT],
)]
pub struct DynSpecAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for DynSpecAgent {
    fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let mut configs = spec.configs.take().unwrap_or_default();
        configs.set(CONFIG_DYN.to_string(), AgentValue::integer(42));
        spec.configs = Some(configs);

        let mut outputs = spec.outputs.take().unwrap_or_default();
        if !outputs.iter().any(|p| p == PORT_DYN_OUT) {
            outputs.push(PORT_DYN_OUT.to_string());
        }
        spec.outputs = Some(outputs);

        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }
}

#[async_trait]
impl AsAgent for CounterAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            count: 0,
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        self.count = 0;
        // self.emit_display(DISPLAY_COUNT, AgentData::new_integer(0))?;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        if port == PORT_RESET {
            self.count = 0;
        } else if port == PORT_IN {
            self.count += 1;
        }
        self.output(ctx, PORT_COUNT, AgentValue::integer(self.count))
            .await?;
        // self.emit_display(DISPLAY_COUNT, AgentValue::integer(self.count))?;
        Ok(())
    }
}
