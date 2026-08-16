use modular_agent_core::{
    AgentConfigSpec, AgentConfigSpecs, AgentConfigs, AgentContext, AgentData, AgentError,
    AgentOutput, AgentSpec, AgentValue, AsAgent, ModularAgent, async_trait, modular_agent,
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
pub const CONFIG_N: &str = "n";
const CONFIG_C0: &str = "c0";
const CONFIG_C1: &str = "c1";
const PORT_0: &str = "0";
const PORT_1: &str = "1";

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
/// construction time; the mutation must survive into the patch spec.
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

/// Regenerates `c0`..`c(n-1)` and the numbered output ports from the `n`
/// config, in new() as well as in configs_changed(). Models the Switch /
/// Match agents, whose live spec - not their definition - knows which config
/// keys currently exist.
#[modular_agent(
    title = "Numbered Config",
    category = CATEGORY,
    inputs = [PORT_IN],
    outputs = [PORT_0, PORT_1],
    integer_config(name = CONFIG_N, default = 2),
    string_config(name = CONFIG_C0),
    string_config(name = CONFIG_C1),
)]
pub struct NumberedConfigAgent {
    data: AgentData,
}

/// A condition value that update_numbered_spec rejects AFTER committing it,
/// the way Switch keeps an unparsable condition as never-matching.
pub const INVALID_CONDITION: &str = "#err";

/// Rebuilds the numbered configs, their config specs and the output ports
/// from `n`.
fn update_numbered_spec(spec: &mut AgentSpec) -> Result<usize, AgentError> {
    let n = spec
        .configs
        .as_ref()
        .map(|configs| configs.get_integer_or(CONFIG_N, 2))
        .unwrap_or(2)
        .clamp(1, 16) as usize;

    let Some(n_spec) = spec
        .config_specs
        .as_ref()
        .and_then(|specs| specs.get(CONFIG_N))
        .cloned()
    else {
        return Err(AgentError::InvalidConfig(format!(
            "config {} must be present",
            CONFIG_N
        )));
    };

    let mut configs = AgentConfigs::new();
    let mut config_specs = AgentConfigSpecs::default();
    configs.set(CONFIG_N.to_string(), AgentValue::integer(n as i64));
    config_specs.insert(CONFIG_N.to_string(), n_spec);

    for i in 0..n {
        let name = format!("c{}", i);
        // `AgentDefinition::reconcile_spec` parks every config the definition
        // does not declare - which includes the generated ones - under a
        // `_`-prefixed key, so fall back to it to survive a reload.
        let value = spec
            .configs
            .as_ref()
            .map(|cfg| {
                if cfg.contains_key(&name) {
                    cfg.get_string_or(&name, "")
                } else {
                    cfg.get_string_or(&format!("_{}", name), "")
                }
            })
            .unwrap_or_default();
        configs.set(name.clone(), AgentValue::string(value));
        config_specs.insert(
            name,
            AgentConfigSpec {
                value: AgentValue::string_default(),
                type_: Some("string".to_string()),
                ..Default::default()
            },
        );
    }

    spec.configs = Some(configs);
    spec.config_specs = Some(config_specs);
    spec.outputs = Some((0..n).map(|i| i.to_string()).collect());

    // Reported after the commit above, like Switch reporting a condition it
    // parsed and stored as never-matching: callers must see the error while
    // the spec keeps the value.
    if let Some(configs) = &spec.configs {
        for i in 0..n {
            if configs.get_string_or(&format!("c{}", i), "") == INVALID_CONDITION {
                return Err(AgentError::InvalidConfig(format!(
                    "condition c{} is not parsable",
                    i
                )));
            }
        }
    }

    Ok(n)
}

#[async_trait]
impl AsAgent for NumberedConfigAgent {
    fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        update_numbered_spec(&mut spec)?;
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        update_numbered_spec(&mut self.data.spec)?;
        self.emit_agent_spec_updated();
        Ok(())
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
