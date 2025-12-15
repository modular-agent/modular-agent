use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};

static CATEGORY: &str = "Std/Array";

static PIN_ARRAY: &str = "array";
static PIN_VALUE: &str = "value";

#[askit_agent(
    title = "Map",
    category = CATEGORY,
    inputs = [PIN_ARRAY],
    outputs = [PIN_VALUE],
)]
struct MapAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for MapAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let data = AgentData::new(askit, id, spec);
        Ok(Self { data })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if value.is_array() {
            let arr = value
                .as_array()
                .ok_or_else(|| AgentError::InvalidValue("Failed to get array".into()))?;
            let n = arr.len();
            for (i, item) in arr.iter().cloned().enumerate() {
                let c = ctx
                    .with_var("map_i".into(), AgentValue::integer(i as i64))
                    .with_var("map_n".into(), AgentValue::integer(n as i64));
                self.try_output(c, PIN_VALUE, item.clone())?;
            }
        } else {
            return Err(AgentError::InvalidValue(
                "Input value is not an array".into(),
            ));
        }
        Ok(())
    }
}

/// Collects input values into an array and emits the array.
#[askit_agent(
    title = "Collect",
    category = CATEGORY,
    inputs = [PIN_VALUE],
    outputs = [PIN_ARRAY],
    string_config(name = "timeout", default = "10s")
)]
struct CollectAgent {
    data: AgentData,
    input_values: Vec<Option<AgentValue>>,
    current_id: usize,
}

#[async_trait]
impl AsAgent for CollectAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let data = AgentData::new(askit, id, spec);
        Ok(Self {
            data,
            input_values: Vec::new(),
            current_id: 0,
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let Some(map_i) = ctx.get_var("map_i").and_then(|v| v.as_i64()) else {
            // no need to collect
            return self.try_output(ctx, PIN_ARRAY, value);
        };
        let Some(map_n) = ctx.get_var("map_n").and_then(|v| v.as_i64()) else {
            // no need to collect
            return self.try_output(ctx, PIN_ARRAY, value);
        };

        // Reset input values if context ID changes
        let ctx_id = ctx.id();
        if ctx_id != self.current_id {
            self.current_id = ctx_id;
            self.input_values = vec![None; map_n as usize];
        }

        self.input_values[map_i as usize] = Some(value);

        // Check if some input is still missing
        if self.input_values.iter().any(|v| v.is_none()) {
            return Ok(());
        }

        // All inputs are present, emit the array
        let arr: Vec<AgentValue> = self
            .input_values
            .iter()
            .map(|v| v.clone().unwrap())
            .collect();
        self.try_output(ctx, PIN_ARRAY, AgentValue::array(arr))
    }
}
