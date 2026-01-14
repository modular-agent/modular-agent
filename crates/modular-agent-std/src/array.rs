use std::collections::VecDeque;
use std::time::Duration;

use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use im::{Vector, vector};
use mini_moka::sync::Cache;

const CATEGORY: &str = "Std/Array";

const PIN_ARRAY: &str = "array";
const PIN_IN1: &str = "in1";
const PIN_IN2: &str = "in2";
const PIN_T: &str = "T";
const PIN_F: &str = "F";
const PIN_VALUE: &str = "value";

const CONFIG_N: &str = "n";
const CONFIG_USE_CTX: &str = "use_ctx";
const CONFIG_TTL_SEC: &str = "ttl_sec";
const CONFIG_CAPACITY: &str = "capacity";

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
            self.output(ctx, PIN_T, value).await
        } else {
            self.output(ctx, PIN_F, value).await
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
            self.output(ctx, PIN_T, value).await
        } else {
            self.output(ctx, PIN_F, value).await
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
        self.output(ctx, PIN_VALUE, AgentValue::integer(length)).await
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
        match value {
            AgentValue::Array(mut arr) => {
                if let Some(first_item) = arr.pop_front() {
                    self.output(ctx, PIN_VALUE, first_item).await
                } else {
                    Err(AgentError::InvalidValue(
                        "Input array is empty, no first item".into(),
                    ))
                }
            }
            other => self.output(ctx, PIN_VALUE, other).await,
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
        if let Some(mut arr) = value.into_array() {
            if arr.is_empty() {
                return self.output(ctx, PIN_ARRAY, AgentValue::array_default()).await;
            }
            arr.pop_front();
            self.output(ctx, PIN_ARRAY, AgentValue::array(arr)).await
        } else {
            self.output(ctx, PIN_ARRAY, AgentValue::array_default()).await
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
        match value {
            AgentValue::Array(mut arr) => {
                if let Some(last_item) = arr.pop_back() {
                    self.output(ctx, PIN_VALUE, last_item).await
                } else {
                    Err(AgentError::InvalidValue(
                        "Input array is empty, no last item".into(),
                    ))
                }
            }
            other => self.output(ctx, PIN_VALUE, other).await,
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

        match value {
            AgentValue::Array(arr) => {
                if let Some(item) = arr.get(n) {
                    self.output(ctx, PIN_VALUE, item.clone()).await
                } else {
                    Err(AgentError::InvalidValue(format!(
                        "Input array length {} is less than n+1={}",
                        arr.len(),
                        n + 1
                    )))
                }
            }
            other => {
                if n == 0 {
                    self.output(ctx, PIN_VALUE, other).await
                } else {
                    Err(AgentError::InvalidValue(
                        "Input is not an array and n != 0".into(),
                    ))
                }
            }
        }
    }
}

/// Takes the first n items from the input array.
/// If the input is not an array, outputs an array with the input as the only item.
/// If n is greater than the array length, outputs the entire array.
#[askit_agent(
    title = "ArrayTake",
    category = CATEGORY,
    inputs = [PIN_ARRAY],
    outputs = [PIN_ARRAY],
    integer_config(name = CONFIG_N, default = 0),
)]
struct ArrayTakeAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ArrayTakeAgent {
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
        if n <= 0 {
            // output empty array
            return self.output(ctx, PIN_ARRAY, AgentValue::array_default()).await;
        }
        let n = n as usize;

        if value.is_array() {
            let arr = value.as_array().unwrap();
            if n >= arr.len() {
                return self.output(ctx, PIN_ARRAY, value).await;
            }
            let taken_items = arr.take(n);
            self.output(ctx, PIN_ARRAY, AgentValue::array(taken_items)).await
        } else {
            self.output(ctx, PIN_ARRAY, AgentValue::array(vector![value])).await
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
        match value {
            AgentValue::Array(arr) => {
                let n = arr.len();
                for (i, item) in arr.into_iter().enumerate() {
                    let c = ctx.push_map_frame(i, n)?;
                    self.output(c, PIN_VALUE, item).await?;
                }
            }
            other => {
                let c = ctx.push_map_frame(0, 1)?;
                self.output(c, PIN_VALUE, other).await?;
            }
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

    // Records the context ID being processed to prevent other contexts from mixing
    current_ctx_id: Option<usize>,

    // Data buffer
    input_values: Vec<Option<AgentValue>>,

    // Expected size of the array
    expected_size: usize,

    // Number of items received (counter to avoid scanning input_values every time)
    received_count: usize,
}

#[async_trait]
impl AsAgent for CollectAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let data = AgentData::new(askit, id, spec);
        Ok(Self {
            data,
            current_ctx_id: None,
            input_values: Vec::new(),
            expected_size: 0,
            received_count: 0,
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        // Check for map frame
        // If not within a map, pass the value through as-is.
        let Some((idx, n)) = ctx.current_map_frame()? else {
            return self.output(ctx, PIN_ARRAY, value).await;
        };

        // Detect context switch and flush processing
        // If a new context ID arrives while the previous context hasn't finished processing
        let ctx_id = ctx.id();
        if let Some(last_id) = &self.current_ctx_id {
            if last_id != &ctx_id {
                log::warn!("Context changed before collection completed. Dropping partial data.");
                self.reset_state();
            }
        }

        // Initialize state (when the first item of this context arrives)
        if self.input_values.is_empty() {
            self.current_ctx_id = Some(ctx_id);
            self.expected_size = n;
            // Fill with None for the required size
            self.input_values = vec![None; n];
            self.received_count = 0;
        }

        // Validation
        if n != self.expected_size {
            // Size shouldn't change within the same context ID, but check just in case
            return Err(AgentError::InvalidValue(
                "Map frame size mismatch within the same context".into(),
            ));
        }
        if idx >= n {
            return Err(AgentError::InvalidValue(
                "Map frame index is out of bounds".into(),
            ));
        }

        // Store data
        // Check if attempting to write to a position that's already filled (duplicate index)
        if self.input_values[idx].is_some() {
            // If duplicate data arrives, overwrite (could also error instead).
        } else {
            self.received_count += 1;
        }
        self.input_values[idx] = Some(value);

        // Check for completion
        if self.received_count == self.expected_size {
            // All items collected, output the result
            let arr = self.drain_buffer_to_vector();

            // Reset state
            self.reset_state();

            // Pop one map frame and output
            let next_ctx = ctx.pop_map_frame()?;
            self.output(next_ctx, PIN_ARRAY, AgentValue::array(arr)).await
        } else {
            // Not yet complete, keep waiting
            Ok(())
        }
    }
}

impl CollectAgent {
    fn reset_state(&mut self) {
        self.current_ctx_id = None;
        self.input_values.clear(); // Capacity is preserved for efficient reuse
        self.expected_size = 0;
        self.received_count = 0;
    }

    // Drain the buffer contents and convert to im::Vector
    fn drain_buffer_to_vector(&mut self) -> Vector<AgentValue> {
        self.input_values
            .drain(..)
            .map(|v| v.unwrap_or(AgentValue::Unit)) // Fill missing values with Unit
            .collect()
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
    integer_config(name = CONFIG_TTL_SEC, default = 60), 
    integer_config(name = CONFIG_CAPACITY, default = 1000),
)]
struct ZipToArrayAgent {
    data: AgentData,
    n: usize,
    use_ctx: bool,

    ttl_sec: u64,
    capacity: u64,
    queues: Vec<VecDeque<AgentValue>>, // for non-ctx mode

    // Context Key -> PendingZip
    ctx_buffers: Cache<String, PendingZip>,
}

#[derive(Clone)]
struct PendingZip {
    values: Vec<Option<AgentValue>>,
    count: usize,
}

impl ZipToArrayAgent {
    fn update_spec(spec: &mut AgentSpec) -> Result<(usize, bool, u64, u64), AgentError> {
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

        Ok((n, use_ctx, ttl_sec, capacity))
    }

    fn reset_state(&mut self) {
        self.queues = vec![VecDeque::new(); self.n];
        self.ctx_buffers.invalidate_all();
    }
}

#[async_trait]
impl AsAgent for ZipToArrayAgent {
    fn new(askit: ASKit, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (n, use_ctx, ttl_sec, capacity) = Self::update_spec(&mut spec)?;

        let cache = Cache::builder()
            .max_capacity(capacity) // Capacity limit (oldest entries are evicted on overflow)
            .time_to_live(Duration::from_secs(ttl_sec)) // TTL (entries expire X seconds after write)
            .build();

        let data = AgentData::new(askit, id, spec);

        Ok(Self {
            data,
            n,
            use_ctx,
            ttl_sec,
            capacity,
            queues: vec![VecDeque::new(); n],
            ctx_buffers: cache,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let (n, use_ctx, ttl_sec, capacity) = Self::update_spec(&mut self.data.spec)?;
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
            // Rebuild cache with new capacity and TTL
            self.ctx_buffers = Cache::builder()
                .max_capacity(capacity)
                .time_to_live(Duration::from_secs(ttl_sec))
                .build();
            self.emit_agent_spec_updated();
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
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
            return Err(AgentError::InvalidValue(format!(
                "Invalid input pin: {}",
                pin
            )));
        };

        if self.use_ctx {
            let ctx_key = ctx.ctx_key()?;

            // Get from cache (or create new if not present)
            let mut entry = self.ctx_buffers.get(&ctx_key).unwrap_or_else(|| PendingZip {
                values: vec![None; self.n],
                count: 0,
            });

            // Update
            if entry.values[idx].is_none() {
                entry.count += 1;
            }
            entry.values[idx] = Some(value);

            // Check for completion
            if entry.count == self.n {
                // All inputs collected, remove from cache (invalidate)
                self.ctx_buffers.invalidate(&ctx_key);

                let arr: Vector<AgentValue> = entry.values
                    .into_iter()
                    .map(|v| v.unwrap())
                    .collect();

                return self.output(ctx, PIN_ARRAY, AgentValue::array(arr)).await;
            }

            return Ok(());
        }

        // Simple FIFO mode processing
        self.queues[idx].push_back(value);

        // Check if all queues have data
        if self.queues.iter().all(|q| !q.is_empty()) {
            let arr: Vector<AgentValue> = self.queues
                .iter_mut()
                .map(|q| q.pop_front().unwrap())
                .collect();

            self.output(ctx, PIN_ARRAY, AgentValue::array(arr)).await
        } else {
            Ok(())
        }
    }
}
