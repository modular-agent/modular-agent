use std::time::Duration;
use std::{collections::VecDeque, vec};

use agent_stream_kit::{
    ASKit, AgentConfigSpec, AgentConfigSpecs, AgentConfigs, AgentContext, AgentData, AgentError,
    AgentOutput, AgentSpec, AgentValue, AsAgent, askit_agent, async_trait,
};
use im::{HashMap, Vector};
use mini_moka::sync::Cache;

static CATEGORY: &str = "Std/Data";

static PIN_IN1: &str = "in1";
static PIN_IN2: &str = "in2";
static PIN_JSON: &str = "json";
static PIN_OBJECT: &str = "object";
static PIN_VALUE: &str = "value";

static CONFIG_KEY: &str = "key";
static CONFIG_VALUE: &str = "value";
static CONFIG_N: &str = "n";
static CONFIG_USE_CTX: &str = "use_ctx";
static CONFIG_TTL_SECONDS: &str = "ttl_sec";
static CONFIG_CAPACITY: &str = "capacity";

// Get Value
#[askit_agent(
    title = "Get Value",
    category = CATEGORY,
    inputs = [PIN_VALUE],
    outputs = [PIN_VALUE],
    string_config(name = CONFIG_KEY)
)]
struct GetValueAgent {
    data: AgentData,
    target_keys: Vec<String>,
}

impl GetValueAgent {
    fn update_spec(spec: &mut AgentSpec) -> Result<Vec<String>, AgentError> {
        let key_str = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_string_or_default(CONFIG_KEY))
            .unwrap_or_default();
        if key_str.is_empty() {
            return Ok(Vec::new());
        }
        let target_keys = key_str.split('.').map(|s| s.to_string()).collect();
        Ok(target_keys)
    }
}

#[async_trait]
impl AsAgent for GetValueAgent {
    fn new(askit: ASKit, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let target_keys = Self::update_spec(&mut spec)?;
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            target_keys,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let target_keys = Self::update_spec(&mut self.data.spec)?;
        self.target_keys = target_keys;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if self.target_keys.is_empty() {
            return Ok(());
        }

        let output_value = match value {
            AgentValue::Array(arr) => {
                let extracted: Vector<AgentValue> = arr
                    .iter()
                    .map(|item| {
                        get_nested_value(item, &self.target_keys)
                            .cloned()
                            .unwrap_or(AgentValue::Unit)
                    })
                    .collect();
                AgentValue::Array(extracted)
            }

            AgentValue::Object(_) => get_nested_value(&value, &self.target_keys)
                .cloned()
                .unwrap_or(AgentValue::Unit),

            _ => AgentValue::Unit,
        };

        self.output(ctx, PIN_VALUE, output_value).await
    }
}

// Set Value
#[askit_agent(
    title = "Set Value",
    category = CATEGORY,
    inputs = [PIN_VALUE],
    outputs = [PIN_VALUE],
    string_config(name = CONFIG_KEY),
    object_config(name = CONFIG_VALUE),
)]
struct SetValueAgent {
    data: AgentData,
    target_keys: Vec<String>,
    target_value: AgentValue,
}

impl SetValueAgent {
    fn update_spec(spec: &mut AgentSpec) -> Result<(Vec<String>, AgentValue), AgentError> {
        let key_str = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_string_or_default(CONFIG_KEY))
            .unwrap_or_default();
        let target_keys = key_str.split('.').map(|s| s.to_string()).collect();
        let target_value = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get(CONFIG_VALUE).cloned().unwrap_or(AgentValue::Unit))
            .unwrap_or(AgentValue::Unit);
        Ok((target_keys, target_value))
    }
}

#[async_trait]
impl AsAgent for SetValueAgent {
    fn new(askit: ASKit, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (target_keys, target_value) = Self::update_spec(&mut spec)?;
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            target_keys,
            target_value,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let (target_keys, target_value) = Self::update_spec(&mut self.data.spec)?;
        self.target_keys = target_keys;
        self.target_value = target_value;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        mut value: AgentValue,
    ) -> Result<(), AgentError> {
        if self.target_keys.is_empty() {
            return Ok(());
        }

        set_nested_value(&mut value, &self.target_keys, self.target_value.clone());
        self.output(ctx, PIN_VALUE, value).await
    }
}

// To Object
#[askit_agent(
    title = "To Object",
    category = CATEGORY,
    inputs = [PIN_VALUE],
    outputs = [PIN_VALUE],
    string_config(name = CONFIG_KEY)
)]
struct ToObjectAgent {
    data: AgentData,
    target_keys: Vec<String>,
}

impl ToObjectAgent {
    fn update_spec(spec: &mut AgentSpec) -> Result<Vec<String>, AgentError> {
        let key_str = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_string_or_default(CONFIG_KEY))
            .unwrap_or_default();
        if key_str.is_empty() {
            return Ok(Vec::new());
        }
        let target_keys = key_str.split('.').map(|s| s.to_string()).collect();
        Ok(target_keys)
    }
}

#[async_trait]
impl AsAgent for ToObjectAgent {
    fn new(askit: ASKit, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let target_keys = Self::update_spec(&mut spec)?;
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            target_keys,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let target_keys = Self::update_spec(&mut self.data.spec)?;
        self.target_keys = target_keys;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if self.target_keys.is_empty() {
            return Ok(());
        }

        let mut new_value = AgentValue::object_default();
        set_nested_value(&mut new_value, &self.target_keys, value);

        self.output(ctx, PIN_VALUE, new_value).await
    }
}

// To JSON
#[askit_agent(
    title = "To JSON",
    category = CATEGORY,
    inputs = [PIN_VALUE],
    outputs = [PIN_JSON]
)]
struct ToJsonAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ToJsonAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let json = serde_json::to_string_pretty(&value)
            .map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        self.output(ctx, PIN_JSON, AgentValue::string(json)).await?;
        Ok(())
    }
}

// From JSON
#[askit_agent(
    title = "From JSON",
    category = CATEGORY,
    inputs = [PIN_JSON],
    outputs = [PIN_VALUE]
)]
struct FromJsonAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for FromJsonAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let s = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("not a string".to_string()))?;
        let json_value: serde_json::Value =
            serde_json::from_str(s).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let value = AgentValue::from_json(json_value)?;
        self.output(ctx, PIN_VALUE, value).await?;
        Ok(())
    }
}

fn get_nested_value<'a, K: AsRef<str>>(
    value: &'a AgentValue,
    keys: &[K],
) -> Option<&'a AgentValue> {
    let mut current_value = value;
    for key in keys {
        let obj = current_value.as_object()?;
        current_value = obj.get(key.as_ref())?;
    }
    Some(current_value)
}

fn set_nested_value<K: AsRef<str>>(root: &mut AgentValue, keys: &[K], new_value: AgentValue) {
    if keys.is_empty() {
        return;
    }

    // Split into the last key and the path before it
    // keys = ["a", "b", "c"] -> path=["a", "b"], last_key="c"
    let (last_key, path) = keys.split_last().unwrap();

    let mut current = root;

    // Traverse down to just before the target
    for key in path {
        // If current position is not an Object, forcibly overwrite it with an empty Object
        if !current.is_object() {
            *current = AgentValue::object_default();
        }

        let obj = current.as_object_mut().unwrap();

        current = obj
            .entry(key.as_ref().to_string())
            .or_insert_with(AgentValue::object_default);
    }

    // Set the value for the last key
    if !current.is_object() {
        *current = AgentValue::object_default();
    }

    if let Some(obj) = current.as_object_mut() {
        obj.insert(last_key.as_ref().to_string(), new_value);
    }
}

/// Zips multiple inputs into an object.
///
/// The number of inputs n and keys are specified via configuration.
///
/// If n=2, it takes two inputs: in1 and in2. Once all inputs are present,
/// it emits them as { key1: in1, key2: in2 }.
///
/// If in2 arrives repeatedly before in1, the in2 values are queued; when in1 arrives,
/// they’re paired in order from the head of the queue and emitted.
///
/// When the `use_ctx` config is true, inputs are matched by context key (including map frames)
/// so that mapped items zip correctly even when they interleave.
#[askit_agent(
    title = "ZipToObject",
    category = CATEGORY,
    inputs = [PIN_IN1, PIN_IN2],
    outputs = [PIN_OBJECT],
    integer_config(name = CONFIG_N, default = 2),
    boolean_config(name = CONFIG_USE_CTX),
    integer_config(name = CONFIG_TTL_SECONDS, default = 60),
    integer_config(name = CONFIG_CAPACITY, default = 1000),
)]
struct ZipToObjectAgent {
    data: AgentData,
    n: usize,
    use_ctx: bool,
    ttl_seconds: u64,
    capacity: usize,

    // Optimization: Pre-load and store key configuration (k1, k2...)
    keys: Vec<String>,

    // For simple mode: FIFO queues
    queues: Vec<VecDeque<AgentValue>>,

    // For use_ctx mode: Cache with TTL
    ctx_buffers: Cache<String, PendingZip>,
}

#[derive(Clone)]
struct PendingZip {
    values: Vec<Option<AgentValue>>,
    count: usize,
}

impl ZipToObjectAgent {
    fn update_spec(
        spec: &mut AgentSpec,
    ) -> Result<(usize, bool, u64, u64, Vec<String>), AgentError> {
        let n = spec
            .configs
            .as_ref()
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
            .map(|c| c.get_integer_or("ttl_seconds", 60))
            .unwrap_or(60) as u64;

        let capacity = spec
            .configs
            .as_ref()
            .map(|c| c.get_integer_or("capacity", 1000))
            .unwrap_or(1000) as u64;

        // Dynamic generation of config definitions (ConfigSpecs)
        let mut configs = AgentConfigs::new();
        let mut config_specs = AgentConfigSpecs::default();

        // Re-set required configurations
        configs.set(CONFIG_N.to_string(), AgentValue::integer(n as i64));
        let Some(n_spec) = spec
            .config_specs
            .as_ref()
            .and_then(|cs| cs.get(CONFIG_N))
            .cloned()
        else {
            return Err(AgentError::InvalidConfig("config n must be present".into()));
        };
        config_specs.insert(CONFIG_N.to_string(), n_spec);

        let Some(use_ctx_spec) = spec
            .config_specs
            .as_ref()
            .and_then(|cs| cs.get(CONFIG_USE_CTX))
            .cloned()
        else {
            return Err(AgentError::InvalidConfig(
                "config use_ctx must be present".into(),
            ));
        };
        config_specs.insert(CONFIG_USE_CTX.to_string(), use_ctx_spec);

        let mut keys = Vec::with_capacity(n);
        for i in 1..=n {
            let key_name = format!("k{}", i);
            let default_key = format!("in{}", i);
            let v = spec
                .configs
                .as_ref()
                .map(|cfg| cfg.get_string_or(&key_name, &default_key))
                .unwrap_or(default_key);

            keys.push(v.clone());

            configs.set(key_name.clone(), AgentValue::string(v));
            config_specs.insert(
                key_name,
                AgentConfigSpec {
                    value: AgentValue::string_default(),
                    type_: Some("string".to_string()),
                    ..Default::default()
                },
            );
        }

        spec.configs = Some(configs);
        spec.config_specs = Some(config_specs);

        spec.inputs = Some((1..=n).map(|i| format!("in{}", i)).collect());

        Ok((n as usize, use_ctx, ttl_sec, capacity, keys))
    }

    fn reset_state(&mut self) {
        self.queues = vec![VecDeque::new(); self.n];
        self.ctx_buffers.invalidate_all();
    }
}

#[async_trait]
impl AsAgent for ZipToObjectAgent {
    fn new(askit: ASKit, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (n, use_ctx, ttl_sec, capacity, keys) = Self::update_spec(&mut spec)?;
        let cache = Cache::builder()
            .max_capacity(capacity)
            .time_to_live(Duration::from_secs(ttl_sec))
            .build();
        let data = AgentData::new(askit, id, spec);
        Ok(Self {
            data,
            n,
            use_ctx,
            ttl_seconds: ttl_sec,
            capacity: capacity as usize,
            keys,
            queues: vec![VecDeque::new(); n],
            ctx_buffers: cache,
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let (n, use_ctx, ttl_sec, capacity, keys) = Self::update_spec(&mut self.data.spec)?;
        let mut changed = false;
        if n != self.n {
            self.n = n;
            changed = true;
        }
        if use_ctx != self.use_ctx {
            self.use_ctx = use_ctx;
            changed = true;
        }
        if ttl_sec != self.ttl_seconds {
            self.ttl_seconds = ttl_sec;
            changed = true;
        }
        if capacity != self.capacity as u64 {
            self.capacity = capacity as usize;
            changed = true;
        }
        if keys != self.keys {
            self.keys = keys;
            changed = true;
        }
        if changed {
            self.reset_state();
            // Rebuild cache with new capacity and ttl
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

        // Context Mode
        if self.use_ctx {
            let ctx_key = ctx.ctx_key()?;

            let mut entry = self
                .ctx_buffers
                .get(&ctx_key)
                .unwrap_or_else(|| PendingZip {
                    values: vec![None; self.n],
                    count: 0,
                });

            if entry.values[idx].is_none() {
                entry.count += 1;
            }
            entry.values[idx] = Some(value);

            if entry.count == self.n {
                self.ctx_buffers.invalidate(&ctx_key);

                // Zip keys and values, then collect
                let map: HashMap<String, AgentValue> = self
                    .keys
                    .iter()
                    .zip(entry.values.into_iter().map(|v| v.unwrap()))
                    .map(|(k, v)| (k.clone(), v))
                    .collect();

                return self.output(ctx, PIN_OBJECT, AgentValue::Object(map)).await;
            } else {
                self.ctx_buffers.insert(ctx_key, entry);
            }
            return Ok(());
        }

        // Simple FIFO Mode
        self.queues[idx].push_back(value);

        if self.queues.iter().all(|q| !q.is_empty()) {
            // Take from head and combine with keys to create Map
            let map: HashMap<String, AgentValue> = self
                .keys
                .iter()
                .zip(self.queues.iter_mut())
                .map(|(k, q)| (k.clone(), q.pop_front().unwrap()))
                .collect();

            self.output(ctx, PIN_OBJECT, AgentValue::Object(map)).await
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use im::hashmap;

    use super::*;

    #[test]
    fn test_get_nested_value() {
        // Setup data: { "users": { "admin": { "name": "Alice" } } }
        let mut root = AgentValue::object_default();
        let mut users = AgentValue::object_default();
        let mut admin = AgentValue::object_default();

        admin
            .set("name".to_string(), AgentValue::string("Alice"))
            .unwrap();
        users.set("admin".to_string(), admin).unwrap();
        root.set("users".to_string(), users).unwrap();

        // Case 1: Successfully retrieve an existing value
        let keys = vec!["users", "admin", "name"];
        let result = get_nested_value(&root, &keys);
        assert_eq!(result, Some(&AgentValue::string("Alice")));

        // Case 2: Intermediate key does not exist (users -> guest)
        let keys_missing = vec!["users", "guest", "name"];
        let result_missing = get_nested_value(&root, &keys_missing);
        assert_eq!(result_missing, None);

        // Case 3: Intermediate path is not an object (users -> admin -> name -> something)
        // "name" is a string, so we cannot traverse deeper -> Should return None
        let keys_not_obj = vec!["users", "admin", "name", "length"];
        let result_not_obj = get_nested_value(&root, &keys_not_obj);
        assert_eq!(result_not_obj, None); // Filtered out by as_object()?

        // Case 4: Empty keys (Should return the root object)
        let keys_empty: Vec<&str> = vec![];
        let result_root = get_nested_value(&root, &keys_empty);
        assert_eq!(result_root, Some(&root));
    }

    /// Verify if a deeply nested structure (a.b.c) can be auto-generated from an empty state.
    #[test]
    fn test_create_deeply_nested_structure() {
        let mut root = AgentValue::object_default();
        let keys = vec!["users", "admin", "name"];
        let value = AgentValue::string("Alice");

        set_nested_value(&mut root, &keys, value);

        // Verify: root["users"]["admin"]["name"] == "Alice"
        if let Some(users) = root.get_mut("users") {
            if let Some(admin) = users.get_mut("admin") {
                if let Some(name) = admin.get_mut("name") {
                    assert_eq!(*name, AgentValue::string("Alice"));
                    return;
                }
            }
        }
        panic!("Nested structure was not created correctly: {:?}", root);
    }

    /// Verify if a new key can be added without breaking existing structures.
    #[test]
    fn test_add_to_existing_structure() {
        let mut root = AgentValue::object_default();
        // Pre-create { "config": {} }
        root.set("config".to_string(), AgentValue::object_default())
            .unwrap();

        let keys = vec!["config", "timeout"];
        let value = AgentValue::string("30s");

        set_nested_value(&mut root, &keys, value);

        // Verify
        let config = root.get_mut("config").unwrap();
        let timeout = config.get_mut("timeout").unwrap();
        assert_eq!(*timeout, AgentValue::string("30s"));
    }

    /// Verify if an existing value can be overwritten.
    #[test]
    fn test_overwrite_existing_value() {
        let mut root = AgentValue::object_default();
        // Pre-create { "app": { "version": "v1" } }
        let mut app = AgentValue::object_default();
        app.set("version".to_string(), AgentValue::string("v1"))
            .unwrap();
        root.set("app".to_string(), app).unwrap();

        // Execute overwrite
        let keys = vec!["app", "version"];
        let new_val = AgentValue::string("v2");
        set_nested_value(&mut root, &keys, new_val);

        // Verify
        let app = root.get_mut("app").unwrap();
        let version = app.get_mut("version").unwrap();
        assert_eq!(*version, AgentValue::string("v2"));
    }

    /// Verify if an intermediate path is not an Object, forcibly overwrite it with an empty Object.
    /// Example: Try setting ["tags", "new_key"] against { "tags": "immutable_string" }
    #[test]
    fn test_overwrite_if_path_is_not_object() {
        let mut root = AgentValue::object_default();
        // "tags" is a string, not an object
        root.set("tags".to_string(), AgentValue::string("some_string"))
            .unwrap();

        let keys = vec!["tags", "new_key"];
        let value = AgentValue::string("value");

        // Ensure it returns without crashing
        set_nested_value(&mut root, &keys, value);

        // Verify that "tags" remains a string
        let tags = root.get_mut("tags").unwrap();
        assert_eq!(
            *tags,
            AgentValue::object(hashmap! {
                "new_key".to_string() => AgentValue::string("value")
            })
        );
    }
}
