use agent_stream_kit::{
    ASKit, AgentConfigs, AgentContext, AgentData, AgentError, AgentOutput, AgentValue, AsAgent,
    async_trait,
};
use askit_macros::askit_agent;

static CATEGORY: &str = "Std/Stream";

static PIN_IN1: &str = "in1";
static PIN_IN2: &str = "in2";
static PIN_IN3: &str = "in3";
static PIN_IN4: &str = "in4";
static PIN_OUT1: &str = "out1";
static PIN_OUT2: &str = "out2";
static PIN_OUT3: &str = "out3";
static PIN_OUT4: &str = "out4";

/// Receives inputs in any order and, once all are present, emits them sequentially.
struct SyncAgent {
    n: usize,
    in_ports: Vec<String>,
    input_values: Vec<Option<AgentValue>>,
    current_id: usize,
}

impl SyncAgent {
    fn new_with_n(n: usize) -> Self {
        let in_ports = (0..n).map(|i| format!("in{}", i + 1)).collect();
        let input_values = vec![None; n];
        Self {
            n,
            in_ports,
            input_values,
            current_id: 0,
        }
    }

    async fn process_impl(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<Vec<AgentValue>, AgentError> {
        // Reset input values if context ID changes
        let ctx_id = ctx.id();
        if ctx_id != self.current_id {
            self.current_id = ctx_id;
            for slot in &mut self.input_values {
                *slot = None;
            }
        }

        // Store the input value
        for i in 0..self.n {
            if pin == self.in_ports[i] {
                self.input_values[i] = Some(value.clone());
            }
        }

        // Check if all inputs are present
        if self.input_values.iter().any(|v| v.is_none()) {
            return Ok(Vec::new());
        }

        // All inputs are present, output in order
        let mut outputs = Vec::new();
        for i in 0..self.n {
            let out_value = self.input_values[i].take().unwrap();
            outputs.push(out_value);
        }

        Ok(outputs)
    }
}

#[askit_agent(
    title = "Sync2",
    category = CATEGORY,
    inputs = [PIN_IN1, PIN_IN2],
    outputs = [PIN_OUT1, PIN_OUT2],
)]
struct Sync2Agent {
    data: AgentData,
    inner: SyncAgent,
}

#[async_trait]
impl AsAgent for Sync2Agent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        let data = AgentData::new(askit, id, def_name, config);
        let inner = SyncAgent::new_with_n(2);
        Ok(Self { data, inner })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let out = self.inner.process_impl(ctx.clone(), pin, value).await?;
        if out.len() != 2 {
            return Ok(());
        }
        self.try_output(ctx.clone(), PIN_OUT1, out[0].clone())?;
        self.try_output(ctx, PIN_OUT2, out[1].clone())?;

        Ok(())
    }
}

#[askit_agent(
    title = "Sync3",
    category = CATEGORY,
    inputs = [PIN_IN1, PIN_IN2, PIN_IN3],
    outputs = [PIN_OUT1, PIN_OUT2, PIN_OUT3],
)]
struct Sync3Agent {
    data: AgentData,
    inner: SyncAgent,
}

#[async_trait]
impl AsAgent for Sync3Agent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        let data = AgentData::new(askit, id, def_name, config);
        let inner = SyncAgent::new_with_n(3);
        Ok(Self { data, inner })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let out = self.inner.process_impl(ctx.clone(), pin, value).await?;
        if out.len() != 3 {
            return Ok(());
        }
        self.try_output(ctx.clone(), PIN_OUT1, out[0].clone())?;
        self.try_output(ctx.clone(), PIN_OUT2, out[1].clone())?;
        self.try_output(ctx, PIN_OUT3, out[2].clone())?;

        Ok(())
    }
}

#[askit_agent(
    title = "Sync4",
    category = CATEGORY,
    inputs = [PIN_IN1, PIN_IN2, PIN_IN3, PIN_IN4],
    outputs = [PIN_OUT1, PIN_OUT2, PIN_OUT3, PIN_OUT4],
)]
struct Sync4Agent {
    data: AgentData,
    inner: SyncAgent,
}

#[async_trait]
impl AsAgent for Sync4Agent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        let data = AgentData::new(askit, id, def_name, config);
        let inner = SyncAgent::new_with_n(3);
        Ok(Self { data, inner })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let out = self.inner.process_impl(ctx.clone(), pin, value).await?;
        if out.len() != 4 {
            return Ok(());
        }
        self.try_output(ctx.clone(), PIN_OUT1, out[0].clone())?;
        self.try_output(ctx.clone(), PIN_OUT2, out[1].clone())?;
        self.try_output(ctx.clone(), PIN_OUT3, out[2].clone())?;
        self.try_output(ctx, PIN_OUT4, out[3].clone())?;

        Ok(())
    }
}
