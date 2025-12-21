use std::collections::VecDeque;

use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};

use crate::ctx_utils::find_first_common_key;

static CATEGORY: &str = "Std/Sequence";

static PIN_IN: &str = "in";
static PIN_IN1: &str = "in1";
static PIN_IN2: &str = "in2";
static PIN_OUT1: &str = "out1";
static PIN_OUT2: &str = "out2";

static CONFIG_N: &str = "n";
static CONFIG_USE_CTX: &str = "use_ctx";

/// Receives an input and emits it sequentially to n outputs.
#[askit_agent(
    title = "Sequence",
    category = CATEGORY,
    inputs = [PIN_IN],
    outputs = [PIN_OUT1, PIN_OUT2],
    integer_config(name = CONFIG_N, default = 2),
)]
struct SequenceAgent {
    data: AgentData,
    n: usize,
}

impl SequenceAgent {
    fn update_spec(spec: &mut AgentSpec) -> Result<usize, AgentError> {
        let mut n = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or(CONFIG_N, 2))
            .unwrap_or(2) as usize;
        if n < 1 {
            n = 1;
        }

        spec.outputs = Some((1..=n).map(|i| format!("out{}", i)).collect());

        Ok(n)
    }
}

#[async_trait]
impl AsAgent for SequenceAgent {
    fn new(askit: ASKit, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let n = Self::update_spec(&mut spec)?;
        let data = AgentData::new(askit, id, spec);
        Ok(Self { data, n })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let n = Self::update_spec(&mut self.data.spec)?;
        let mut changed = false;
        if n != self.n {
            self.n = n;
            changed = true;
        }
        if changed {
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
    inputs = [PIN_IN1, PIN_IN2],
    outputs = [PIN_OUT1, PIN_OUT2],
    integer_config(name = CONFIG_N, default = 2),
    boolean_config(name = CONFIG_USE_CTX),
)]
struct SyncAgent {
    data: AgentData,
    n: usize,
    use_ctx: bool,
    input_values: Vec<Vec<AgentValue>>,
    ctx_input_values: Vec<VecDeque<(String, AgentValue)>>,
}

impl SyncAgent {
    fn update_spec(spec: &mut AgentSpec) -> Result<(usize, bool), AgentError> {
        let mut n = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or(CONFIG_N, 2))
            .unwrap_or(2) as usize;
        if n < 1 {
            n = 1;
        }

        let use_ctx = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_bool_or_default(CONFIG_USE_CTX))
            .unwrap_or(false);

        spec.inputs = Some((1..=n).map(|i| format!("in{}", i)).collect());
        spec.outputs = Some((1..=n).map(|i| format!("out{}", i)).collect());

        Ok((n, use_ctx))
    }
}

#[async_trait]
impl AsAgent for SyncAgent {
    fn new(askit: ASKit, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (n, use_ctx) = Self::update_spec(&mut spec)?;
        let data = AgentData::new(askit, id, spec);
        Ok(Self {
            data,
            n,
            use_ctx,
            input_values: vec![Vec::new(); n],
            ctx_input_values: vec![VecDeque::new(); n],
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let (n, use_ctx) = Self::update_spec(&mut self.data.spec)?;
        let mut changed = false;
        if n != self.n {
            self.n = n;
            changed = true;
        }
        if use_ctx != self.use_ctx {
            self.use_ctx = use_ctx;
            changed = true;
        }
        if changed {
            self.input_values = vec![Vec::new(); self.n];
            self.ctx_input_values = vec![VecDeque::new(); self.n];
            self.emit_agent_spec_updated();
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        // Clear input queues on stop
        self.input_values = vec![Vec::new(); self.n];
        self.ctx_input_values = vec![VecDeque::new(); self.n];
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
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

        if self.use_ctx {
            if self.ctx_input_values.len() != self.n {
                self.ctx_input_values = vec![VecDeque::new(); self.n];
            }

            let ctx_key = ctx.ctx_key()?;
            self.ctx_input_values[i].push_back((ctx_key, value));

            if self.ctx_input_values.iter().any(|q| q.is_empty()) {
                return Ok(());
            }

            let Some((_target_key, positions)) = find_first_common_key(&self.ctx_input_values)
            else {
                return Ok(());
            };

            for (queue, pos) in self.ctx_input_values.iter_mut().zip(positions) {
                for _ in 0..pos {
                    queue.pop_front();
                }
            }

            // Now all heads share target_key
            let arr: Vec<AgentValue> = self
                .ctx_input_values
                .iter()
                .map(|q| q.front().unwrap().1.clone())
                .collect();
            for q in self.ctx_input_values.iter_mut() {
                q.pop_front();
            }

            // output in order
            for i in 0..self.n {
                self.try_output(
                    ctx.clone(),
                    self.data.spec.outputs.as_ref().unwrap()[i].clone(),
                    arr[i].clone(),
                )?;
            }

            return Ok(());
        }

        self.input_values[i].push(value);

        // Check if some input is still missing
        if self.input_values.iter().any(|v| v.is_empty()) {
            return Ok(());
        }

        // All inputs are present, emit the array
        let arr: Vec<AgentValue> = self.input_values.iter().map(|v| v[0].clone()).collect();
        for v in &mut self.input_values {
            v.remove(0);
        }

        // output in order
        for i in 0..self.n {
            self.try_output(
                ctx.clone(),
                self.data.spec.outputs.as_ref().unwrap()[i].clone(),
                arr[i].clone(),
            )?;
        }

        Ok(())
    }
}
