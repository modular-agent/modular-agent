use std::collections::VecDeque;
use std::time::Duration;

use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use mini_moka::sync::Cache;
static CONFIG_TTL_SEC: &str = "ttl_sec";
static CONFIG_CAPACITY: &str = "capacity";

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
            self.output(ctx.clone(), out_pin, value.clone()).await?;
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
    integer_config(name = CONFIG_TTL_SEC, default = 60), 
    integer_config(name = CONFIG_CAPACITY, default = 1000),
)]
struct SyncAgent {
    data: AgentData,
    n: usize,
    use_ctx: bool,
        ttl_sec: u64,
    capacity: u64,

    // Optimization: Pre-generate and store output pin names ("out1", "out2"...)
    output_pins: Vec<String>,

    // For simple mode
    queues: Vec<VecDeque<AgentValue>>,

    // For use_ctx mode: Cache with TTL
    ctx_buffers: Cache<String, PendingSync>,
}

#[derive(Clone)]
struct PendingSync {
    values: Vec<Option<AgentValue>>,
    count: usize,
}

impl SyncAgent {
    fn update_spec(spec: &mut AgentSpec) -> Result<(usize, bool, u64, u64, Vec<String>), AgentError> {
        let n = spec.configs.as_ref()
            .map(|cfg| cfg.get_integer_or(CONFIG_N, 2))
            .unwrap_or(2) as usize;
        let n = if n < 1 { 1 } else { n };

        let use_ctx = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_bool_or_default(CONFIG_USE_CTX))
            .unwrap_or(false);

        let ttl_sec = spec
            .configs
            .as_ref()
            .map(|c| c.get_integer_or(CONFIG_TTL_SEC, 60))
            .unwrap_or(60) as u64;

        let capacity = spec
            .configs
            .as_ref()
            .map(|c| c.get_integer_or(CONFIG_CAPACITY, 1000))
            .unwrap_or(1000) as u64;

        spec.inputs = Some((1..=n).map(|i| format!("in{}", i)).collect());

        let output_pins: Vec<String> = (1..=n).map(|i| format!("out{}", i)).collect();
        spec.outputs = Some(output_pins.clone());

        Ok((n, use_ctx, ttl_sec, capacity, output_pins))
    }

    fn reset_state(&mut self) {
        self.queues = vec![VecDeque::new(); self.n];
        self.ctx_buffers.invalidate_all();
    }
}

#[async_trait]
impl AsAgent for SyncAgent {
    fn new(askit: ASKit, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (n, use_ctx, ttl_sec, capacity, output_pins) = Self::update_spec(&mut spec)?;

        let cache = Cache::builder()
            .max_capacity(capacity)
            .time_to_live(Duration::from_secs(ttl_sec))
            .build();

        let data = AgentData::new(askit, id, spec);
        Ok(Self {
            data,
            n,
            use_ctx,
            ttl_sec,
            capacity,
            output_pins,
            queues: vec![VecDeque::new(); n],
            ctx_buffers: cache,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let (n, use_ctx, ttl_sec, capacity, output_pins) = Self::update_spec(&mut self.data.spec)?;
        let mut changed = false;
        if n != self.n {
            self.n = n;
            changed = true;
        }
        if use_ctx != self.use_ctx {
            self.use_ctx = use_ctx;
            changed = true;
        }
        if ttl_sec != self.ttl_sec {
            self.ttl_sec = ttl_sec;
            changed = true;
        }
        if capacity != self.capacity {
            self.capacity = capacity;
            changed = true;
        }
        if changed {
            self.reset_state();
            self.output_pins = output_pins;
            self.ctx_buffers = Cache::builder()
                .max_capacity(capacity)
                .time_to_live(Duration::from_secs(ttl_sec))
                .build();
            self.emit_agent_spec_updated();
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        // Clear input queues on stop
        self.reset_state();
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        // Parse pin number
        let Some(idx) = pin
            .strip_prefix("in")
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&i| i >= 1 && i <= self.n)
            .map(|i| i - 1)
        else {
            return Err(AgentError::InvalidValue(format!("Invalid input pin: {}", pin)));
        };

        // Context Mode
        if self.use_ctx {
            let ctx_key = ctx.ctx_key()?;

            // Get from cache or create new
            let mut entry = self.ctx_buffers.get(&ctx_key).unwrap_or_else(|| PendingSync {
                values: vec![None; self.n],
                count: 0,
            });

            if entry.values[idx].is_none() {
                entry.count += 1;
            }
            entry.values[idx] = Some(value);

            if entry.count == self.n {
                // All inputs collected, remove from cache
                self.ctx_buffers.invalidate(&ctx_key);

                // Output sequentially
                for (i, val_opt) in entry.values.into_iter().enumerate() {
                    if let Some(val) = val_opt {
                        self.output(ctx.clone(), &self.output_pins[i], val).await?;
                    }
                }
            }
            return Ok(());
        }

        // Simple FIFO Mode
        self.queues[idx].push_back(value);

        // Check if all queues have data
        if self.queues.iter().all(|q| !q.is_empty()) {
            let ready_values: Vec<AgentValue> = self.queues
                .iter_mut()
                .map(|q| q.pop_front().unwrap())
                .collect();

            for (i, val) in ready_values.into_iter().enumerate() {
                self.output(ctx.clone(), &self.output_pins[i], val).await?;
            }
        }

        Ok(())
    }
}
