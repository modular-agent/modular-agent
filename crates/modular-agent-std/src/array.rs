use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use std::collections::VecDeque;

static CATEGORY: &str = "Std/Array";

static PIN_ARRAY: &str = "array";
static PIN_IN1: &str = "in1";
static PIN_IN2: &str = "in2";
static PIN_T: &str = "T";
static PIN_F: &str = "F";
static PIN_VALUE: &str = "value";

static CONFIG_N: &str = "n";
static CONFIG_USE_CTX: &str = "use_ctx";

/// Check if an input is an array.
#[askit_agent(
    title = "IsArray",
    category = CATEGORY,
    inputs = [PIN_VALUE],
    outputs = [PIN_T, PIN_F],
)]
struct IsArrayAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for IsArrayAgent {
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
            self.try_output(ctx, PIN_T, value)
        } else {
            self.try_output(ctx, PIN_F, value)
        }
    }
}

/// Checks if an input array is empty, emitting to T or F accordingly.
/// If the input is not an array, it is treated as non-empty.
#[askit_agent(
    title = "IsEmptyArray",
    category = CATEGORY,
    inputs = [PIN_ARRAY],
    outputs = [PIN_T, PIN_F],
)]
struct IsEmptyArrayAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for IsEmptyArrayAgent {
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
        let mut is_empty = false;
        if value.is_array() {
            let arr = value.as_array().unwrap();
            if arr.is_empty() {
                is_empty = true;
            }
        }
        if is_empty {
            self.try_output(ctx, PIN_T, value)
        } else {
            self.try_output(ctx, PIN_F, value)
        }
    }
}

/// Outputs the length of the input array.
/// If the input is not an array, outputs 1.
/// This is different from IsEmpty, but is designed for consistency with Map.
#[askit_agent(
    title = "ArrayLength",
    category = CATEGORY,
    inputs = [PIN_ARRAY],
    outputs = [PIN_VALUE],
)]
struct ArrayLengthAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ArrayLengthAgent {
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
        let length = if value.is_array() {
            let arr = value.as_array().unwrap();
            arr.len() as i64
        } else {
            1
        };
        self.try_output(ctx, PIN_VALUE, AgentValue::integer(length))
    }
}

/// Output the first item of the input array.
/// If the input is not an array, outputs the input itself.
/// Errors if the input array is empty.
#[askit_agent(
    title = "ArrayFirst",
    category = CATEGORY,
    inputs = [PIN_ARRAY],
    outputs = [PIN_VALUE],
)]
struct ArrayFirstAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ArrayFirstAgent {
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
            let arr = value.as_array().unwrap();
            if arr.is_empty() {
                return Err(AgentError::InvalidValue(
                    "Input array is empty, no first item".into(),
                ));
            }
            let first_item = arr[0].clone();
            self.try_output(ctx, PIN_VALUE, first_item)
        } else {
            self.try_output(ctx, PIN_VALUE, value)
        }
    }
}

/// Output the rest of the input array after removing the first item.
/// If the input is not an array, outputs an empty array.
/// Output an empty array if the input array is empty.
#[askit_agent(
    title = "ArrayRest",
    category = CATEGORY,
    inputs = [PIN_ARRAY],
    outputs = [PIN_ARRAY],
)]
struct ArrayRestAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ArrayRestAgent {
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
            let arr = value.as_array().unwrap();
            if arr.is_empty() {
                return self.try_output(ctx, PIN_ARRAY, AgentValue::array(Vec::new()));
            }
            let rest_items = arr[1..].to_vec();
            self.try_output(ctx, PIN_ARRAY, AgentValue::array(rest_items))
        } else {
            self.try_output(ctx, PIN_ARRAY, AgentValue::array_default())
        }
    }
}

//// Output the last item of the input array.
/// If the input is not an array, outputs the input itself.
/// Errors if the input array is empty.
#[askit_agent(
    title = "ArrayLast",
    category = CATEGORY,
    inputs = [PIN_ARRAY],
    outputs = [PIN_VALUE],
)]
struct ArrayLastAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ArrayLastAgent {
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
            let arr = value.as_array().unwrap();
            if arr.is_empty() {
                return Err(AgentError::InvalidValue(
                    "Input array is empty, no last item".into(),
                ));
            }
            let last_item = arr[arr.len() - 1].clone();
            self.try_output(ctx, PIN_VALUE, last_item)
        } else {
            self.try_output(ctx, PIN_VALUE, value)
        }
    }
}

/// Output the nth-item of the input array.
/// If the input is not an array, outputs the input itself if n=0, else errors.
/// Errors if the input array is shorter than n+1.
#[askit_agent(
    title = "ArrayNth",
    category = CATEGORY,
    inputs = [PIN_ARRAY],
    outputs = [PIN_VALUE],
    integer_config(name = CONFIG_N, default = 0),
)]
struct ArrayNthAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ArrayNthAgent {
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
        let n = self
            .data
            .spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or(CONFIG_N, 0))
            .unwrap_or(0);
        if n < 0 {
            return Err(AgentError::InvalidConfig("n must be non-negative".into()));
        }
        let n = n as usize;

        if value.is_array() {
            let arr = value.as_array().unwrap();
            if n >= arr.len() {
                return Err(AgentError::InvalidValue(format!(
                    "Input array length {} is less than n+1={}",
                    arr.len(),
                    n + 1
                )));
            }
            let nth_item = arr[n].clone();
            self.try_output(ctx, PIN_VALUE, nth_item)
        } else {
            if n == 0 {
                self.try_output(ctx, PIN_VALUE, value)
            } else {
                Err(AgentError::InvalidValue(
                    "Input is not an array and n != 0".into(),
                ))
            }
        }
    }
}

/// Maps over an input array, emitting each item individually with a `map` frame that captures the index and length.
/// Nested maps accumulate frames to preserve lineage. If the input is not an array, it is treated as a single-item array.
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
        if !value.is_array() {
            let c = ctx.push_map_frame(0, 1)?;
            return self.try_output(c, PIN_VALUE, value);
        }

        let arr = value
            .as_array()
            .ok_or_else(|| AgentError::InvalidValue("Failed to get array".into()))?;

        let n = arr.len();
        for (i, item) in arr.iter().cloned().enumerate() {
            let c = ctx.push_map_frame(i, n)?;
            self.try_output(c, PIN_VALUE, item)?;
        }
        Ok(())
    }
}

/// Collects input values into an array.
///
/// Expects a `map` frame to determine the position and length for each input value.
/// The `map` frame stores keys `i` (index) and `n` (length). Nested maps stack frames.
/// If a `map` frame is not present, the input value is emitted directly.
///
/// Incomplete arrays are emitted when the context changes.
#[askit_agent(
    title = "Collect",
    category = CATEGORY,
    description = "Collects input values into an array",
    inputs = [PIN_VALUE],
    outputs = [PIN_ARRAY],
)]
struct CollectAgent {
    data: AgentData,
    input_values: Vec<Option<AgentValue>>,
    last_ctx: Option<AgentContext>,
}

fn drain_input_values(values: &mut Vec<Option<AgentValue>>) -> Vec<AgentValue> {
    values
        .drain(..)
        .map(|v| v.unwrap_or_else(AgentValue::unit))
        .collect()
}

#[async_trait]
impl AsAgent for CollectAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let data = AgentData::new(askit, id, spec);
        Ok(Self {
            data,
            input_values: Vec::new(),
            last_ctx: None,
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        // Reset input values if context ID changes
        let ctx_id = ctx.id();
        if let Some(last_ctx) = &self.last_ctx {
            if ctx_id != last_ctx.id() {
                if !self.input_values.is_empty() {
                    // Output incomplete array from previous context
                    let arr = drain_input_values(&mut self.input_values);
                    let next_ctx = last_ctx.pop_map_frame()?;
                    self.try_output(next_ctx, PIN_ARRAY, AgentValue::array(arr))?;
                }
                self.input_values = Vec::new();
            }
        }
        self.last_ctx = None;

        let Some((idx_usize, n_usize)) = ctx.current_map_frame()? else {
            self.input_values = Vec::new();
            return self.try_output(ctx, PIN_ARRAY, value);
        };

        if idx_usize >= n_usize {
            return Err(AgentError::InvalidValue(
                "map frame index is out of bounds".into(),
            ));
        }

        if self.input_values.len() != n_usize {
            self.input_values = vec![None; n_usize];
        }

        self.input_values[idx_usize] = Some(value);

        // Check if some input is still missing
        if self.input_values.iter().any(|v| v.is_none()) {
            self.last_ctx = Some(ctx.clone());
            return Ok(());
        }

        // All inputs are present, emit the array
        let arr: Vec<AgentValue> = self
            .input_values
            .iter()
            .map(|v| v.clone().unwrap())
            .collect();
        self.input_values = Vec::new();
        let next_ctx = ctx.pop_map_frame()?;
        self.try_output(next_ctx, PIN_ARRAY, AgentValue::array(arr))
    }
}

/// Zips multiple inputs into an array.
///
/// The number of inputs n is specified via configuration.
///
/// If n=2, it takes two inputs: in1 and in2. Once all inputs are present,
/// it emits them as [in1, in2].
///
/// If in2 arrives repeatedly before in1, the in2 values are queued; when in1 arrives,
/// they’re paired in order from the head of the queue and emitted.
///
/// When the `use_ctx` config is true, inputs are matched by context key (including map frames)
/// so that mapped items zip correctly even when they interleave.
#[askit_agent(
    title = "ZipToArray",
    category = CATEGORY,
    inputs = [PIN_IN1, PIN_IN2],
    outputs = [PIN_ARRAY],
    integer_config(name = CONFIG_N, default = 2),
    boolean_config(name = CONFIG_USE_CTX),
)]
struct ZipToArrayAgent {
    data: AgentData,
    n: usize,
    use_ctx: bool,
    input_values: Vec<Vec<AgentValue>>,
    ctx_input_values: Vec<VecDeque<(String, AgentValue)>>,
}

impl ZipToArrayAgent {
    fn find_first_common_key(
        queues: &Vec<VecDeque<(String, AgentValue)>>,
    ) -> Option<(String, Vec<usize>)> {
        let (base_idx, base_queue) = queues
            .iter()
            .enumerate()
            .filter(|(_, q)| !q.is_empty())
            .min_by_key(|(_, q)| q.len())?;

        for (pos, (key, _)) in base_queue.iter().enumerate() {
            let mut positions = vec![usize::MAX; queues.len()];
            positions[base_idx] = pos;
            let mut found_in_all = true;
            for (idx, queue) in queues.iter().enumerate() {
                if idx == base_idx {
                    continue;
                }
                if let Some(p) = queue.iter().position(|(k, _)| k == key) {
                    positions[idx] = p;
                } else {
                    found_in_all = false;
                    break;
                }
            }
            if found_in_all {
                return Some((key.clone(), positions));
            }
        }
        None
    }
}

#[async_trait]
impl AsAgent for ZipToArrayAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let mut n = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or(CONFIG_N, 2))
            .unwrap_or(2) as usize;
        if n < 1 {
            n = 1;
        }
        let mut spec = spec;
        let use_ctx = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_bool_or_default(CONFIG_USE_CTX))
            .unwrap_or(false);
        spec.inputs = Some((1..=n).map(|i| format!("in{}", i)).collect());
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
        let cfg_n = self
            .data
            .spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or(CONFIG_N, 2))
            .unwrap_or(2) as usize;
        let cfg_use_ctx = self
            .data
            .spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_bool_or_default(CONFIG_USE_CTX))
            .unwrap_or(false);
        if cfg_n < 1 {
            return Err(AgentError::InvalidConfig("n must be at least 1".into()));
        }
        let mut changed = false;
        if cfg_n != self.n {
            self.n = cfg_n;
            self.data.spec.inputs = Some((1..=self.n).map(|i| format!("in{}", i)).collect());
            changed = true;
        }
        if cfg_use_ctx != self.use_ctx {
            self.use_ctx = cfg_use_ctx;
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

            let Some((_target_key, positions)) =
                ZipToArrayAgent::find_first_common_key(&self.ctx_input_values)
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
            return self.try_output(ctx, PIN_ARRAY, AgentValue::array(arr));
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
        self.try_output(ctx, PIN_ARRAY, AgentValue::array(arr))
    }
}
