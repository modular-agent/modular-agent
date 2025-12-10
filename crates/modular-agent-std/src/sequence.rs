use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};

static CATEGORY: &str = "Std/Sequence";

static PIN_IN: &str = "in";
static PIN_OUT1: &str = "out1";
static PIN_OUT2: &str = "out2";

/// Receives an input and emits it sequentially to n outputs.
#[askit_agent(
    title = "Sequence",
    category = CATEGORY,
    inputs = [PIN_IN],
    outputs = [PIN_OUT1, PIN_OUT2],
    integer_config(name = "n", default = 2),
)]
struct SequenceAgent {
    data: AgentData,
    n: usize,
}

#[async_trait]
impl AsAgent for SequenceAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let n = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or("n", 2))
            .unwrap_or(2) as usize;
        let mut spec = spec;
        spec.outputs = Some((1..=n).map(|i| format!("out{}", i)).collect());
        let data = AgentData::new(askit, id, spec);
        Ok(Self { data, n })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let cfg_n = self
            .data
            .spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or("n", 2))
            .unwrap_or(2) as usize;
        if cfg_n != self.n {
            self.n = cfg_n;
            self.data.spec.outputs = Some((1..=self.n).map(|i| format!("out{}", i)).collect());
            self.emit_agent_spec_updated();
        }
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        for i in 0..self.n {
            let out_pin = format!("out{}", i + 1);
            self.try_output(ctx.clone(), out_pin, value.clone())?;
        }
        Ok(())
    }
}

/// Receives inputs in any order and, once all are present, emits them sequentially.
#[askit_agent(
    title = "Sync",
    category = CATEGORY,
    inputs = [PIN_IN],
    outputs = [PIN_OUT1, PIN_OUT2],
    integer_config(name = "n", default = 2),
)]
struct SyncAgent {
    data: AgentData,
    n: usize,
    input_values: Vec<Option<AgentValue>>,
    current_id: usize,
}

#[async_trait]
impl AsAgent for SyncAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let n = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or("n", 2))
            .unwrap_or(2) as usize;
        let mut spec = spec;
        spec.inputs = Some((1..=n).map(|i| format!("in{}", i)).collect());
        spec.outputs = Some((1..=n).map(|i| format!("out{}", i)).collect());
        let data = AgentData::new(askit, id, spec);
        Ok(Self {
            data,
            n,
            input_values: vec![None; n],
            current_id: 0,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let cfg_n = self
            .data
            .spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or("n", 2))
            .unwrap_or(2) as usize;
        if cfg_n < 1 {
            return Err(AgentError::InvalidConfig("n must be at least 1".into()));
        }
        if cfg_n != self.n {
            self.n = cfg_n;
            self.data.spec.inputs = Some((1..=self.n).map(|i| format!("in{}", i)).collect());
            self.data.spec.outputs = Some((1..=self.n).map(|i| format!("out{}", i)).collect());
            self.input_values = vec![None; self.n];
            self.current_id = 0;
            self.emit_agent_spec_updated();
        }
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        // Reset input values if context ID changes
        let ctx_id = ctx.id();
        if ctx_id != self.current_id {
            self.current_id = ctx_id;
            for slot in &mut self.input_values {
                *slot = None;
            }
        }

        // Store the input value
        let Some(i) = pin
            .strip_prefix("in")
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|idx| {
                if idx >= 1 && idx <= self.n {
                    Some(idx - 1)
                } else {
                    None
                }
            })
        else {
            return Err(AgentError::InvalidValue(format!(
                "Invalid input pin: {}",
                pin
            )));
        };

        self.input_values[i] = Some(value);

        // Check if some input is still missing
        if self.input_values.iter().any(|v| v.is_none()) {
            return Ok(());
        }

        // All inputs are present, output in order
        for i in 0..self.n {
            let out_value = self.input_values[i].take().unwrap();
            self.try_output(
                ctx.clone(),
                self.data.spec.outputs.as_ref().unwrap()[i].clone(),
                out_value,
            )?;
        }

        Ok(())
    }
}
