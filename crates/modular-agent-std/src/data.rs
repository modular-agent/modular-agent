use std::time::Duration;
use std::{collections::VecDeque, vec};

use im::{HashMap, Vector};
use mini_moka::sync::Cache;
use modular_agent_core::{
    AgentConfigSpec, AgentConfigSpecs, AgentConfigs, AgentContext, AgentData, AgentError,
    AgentOutput, AgentSpec, AgentValue, AsAgent, ModularAgent, async_trait, modular_agent,
    parse_index,
};

const CATEGORY: &str = "Std/Data";

const PORT_JSON: &str = "json";
const PORT_OBJECT: &str = "object";
const PORT_VALUE: &str = "value";

const CONFIG_KEY: &str = "key";
const CONFIG_VALUE: &str = "value";
const CONFIG_N: &str = "n";
const CONFIG_USE_CTX: &str = "use_ctx";
const CONFIG_TTL_SEC: &str = "ttl_sec";
const CONFIG_CAPACITY: &str = "capacity";

// Get Value
#[modular_agent(
    title = "Get Value",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    outputs = [PORT_VALUE],
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

    fn extract(value: AgentValue, target_keys: &[String]) -> AgentValue {
        match value {
            // A root array broadcasts the key path over its elements — unless
            // the path starts with an index, which addresses the array itself.
            AgentValue::Array(arr) if parse_index(&target_keys[0]).is_none() => {
                let extracted: Vector<AgentValue> = arr
                    .iter()
                    .map(|item| get_nested_value(item, target_keys).unwrap_or(AgentValue::Unit))
                    .collect();
                AgentValue::Array(extracted)
            }

            other => get_nested_value(&other, target_keys).unwrap_or(AgentValue::Unit),
        }
    }
}

#[async_trait]
impl AsAgent for GetValueAgent {
    fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let target_keys = Self::update_spec(&mut spec)?;
        Ok(Self {
            data: AgentData::new(ma, id, spec),
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
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if self.target_keys.is_empty() {
            return Ok(());
        }

        let output_value = Self::extract(value, &self.target_keys);
        self.output(ctx, PORT_VALUE, output_value).await
    }
}

// Set Value
#[modular_agent(
    title = "Set Value",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    outputs = [PORT_VALUE],
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
        let target_keys = if key_str.is_empty() {
            Vec::new()
        } else {
            key_str.split('.').map(|s| s.to_string()).collect()
        };
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
    fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (target_keys, target_value) = Self::update_spec(&mut spec)?;
        Ok(Self {
            data: AgentData::new(ma, id, spec),
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
        _port: String,
        mut value: AgentValue,
    ) -> Result<(), AgentError> {
        if self.target_keys.is_empty() {
            return Ok(());
        }

        set_nested_value(&mut value, &self.target_keys, self.target_value.clone())?;
        self.output(ctx, PORT_VALUE, value).await
    }
}

// To Object
#[modular_agent(
    title = "To Object",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    outputs = [PORT_VALUE],
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
    fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let target_keys = Self::update_spec(&mut spec)?;
        Ok(Self {
            data: AgentData::new(ma, id, spec),
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
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if self.target_keys.is_empty() {
            return Ok(());
        }

        let mut new_value = AgentValue::object_default();
        set_nested_value(&mut new_value, &self.target_keys, value)?;

        self.output(ctx, PORT_VALUE, new_value).await
    }
}

// To JSON
#[modular_agent(
    title = "To JSON",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    outputs = [PORT_JSON]
)]
struct ToJsonAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ToJsonAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let json = serde_json::to_string_pretty(&value)
            .map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        self.output(ctx, PORT_JSON, AgentValue::string(json))
            .await?;
        Ok(())
    }
}

// From JSON
#[modular_agent(
    title = "From JSON",
    category = CATEGORY,
    inputs = [PORT_JSON],
    outputs = [PORT_VALUE]
)]
struct FromJsonAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for FromJsonAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let s = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("not a string".to_string()))?;
        let json_value: serde_json::Value =
            serde_json::from_str(s).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let value = AgentValue::from_json(json_value)?;
        self.output(ctx, PORT_VALUE, value).await?;
        Ok(())
    }
}

pub(crate) fn get_nested_value<K: AsRef<str>>(
    value: &AgentValue,
    keys: &[K],
) -> Option<AgentValue> {
    let mut current = value.clone(); // cheap: Arc/im structures
    for key in keys {
        current = current.get_prop(key.as_ref())?;
    }
    Some(current)
}

fn set_nested_value<K: AsRef<str>>(
    root: &mut AgentValue,
    keys: &[K],
    new_value: AgentValue,
) -> Result<(), AgentError> {
    let Some((first, rest)) = keys.split_first() else {
        return Ok(());
    };
    let key = first.as_ref();

    // A value without properties is overwritten with an empty Object, as before.
    if !root.has_props() {
        *root = AgentValue::object_default();
    }

    if rest.is_empty() {
        root.set_prop(key, new_value)
    } else if let Some(obj) = root.as_object_mut() {
        let sub = obj
            .entry(key.to_string())
            .or_insert_with(AgentValue::object_default);
        set_nested_value(sub, rest, new_value)
    } else {
        // Array: read-modify-write the materialized element, then store it
        // back through set_prop, which rejects a bad index with the root
        // untouched. A Message also lands here (its properties materialize
        // via get_prop) but its set_prop is always an error — Messages are
        // read-only through key paths.
        let mut sub = root
            .get_prop(key)
            .unwrap_or_else(AgentValue::object_default);
        set_nested_value(&mut sub, rest, new_value)?;
        root.set_prop(key, sub)
    }
}

/// Zips multiple inputs into an object.
///
/// The number of inputs n and keys are specified via configuration, and the input
/// ports are numbered `0`..`n-1`.
///
/// If n=2, it takes two inputs: `0` and `1`. Once all inputs are present,
/// it emits them as { k0: value of `0`, k1: value of `1` }.
///
/// If `1` arrives repeatedly before `0`, the `1` values are queued; when `0` arrives,
/// they’re paired in order from the head of the queue and emitted.
///
/// When the `use_ctx` config is true, inputs are matched by context key (including map frames)
/// so that mapped items zip correctly even when they interleave.
#[modular_agent(
    title = "ZipToObject",
    category = CATEGORY,
    inputs = ["0", "1"],
    outputs = [PORT_OBJECT],
    integer_config(name = CONFIG_N, default = 2),
    boolean_config(name = CONFIG_USE_CTX),
    integer_config(name = CONFIG_TTL_SEC, default = 60),
    integer_config(name = CONFIG_CAPACITY, default = 1000),
)]
struct ZipToObjectAgent {
    data: AgentData,
    n: usize,
    use_ctx: bool,
    ttl_sec: u64,
    capacity: usize,

    // Optimization: Pre-load and store key configuration (k0, k1...)
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
            .map(|c| c.get_integer_or(CONFIG_TTL_SEC, 60))
            .unwrap_or(60) as u64;

        let capacity = spec
            .configs
            .as_ref()
            .map(|c| c.get_integer_or(CONFIG_CAPACITY, 1000))
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

        configs.set(CONFIG_USE_CTX.to_string(), AgentValue::boolean(use_ctx));
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

        configs.set(
            CONFIG_TTL_SEC.to_string(),
            AgentValue::integer(ttl_sec as i64),
        );
        let Some(ttl_spec) = spec
            .config_specs
            .as_ref()
            .and_then(|cs| cs.get(CONFIG_TTL_SEC))
            .cloned()
        else {
            return Err(AgentError::InvalidConfig(
                "config ttl_sec must be present".into(),
            ));
        };
        config_specs.insert(CONFIG_TTL_SEC.to_string(), ttl_spec);

        configs.set(
            CONFIG_CAPACITY.to_string(),
            AgentValue::integer(capacity as i64),
        );
        let Some(capacity_spec) = spec
            .config_specs
            .as_ref()
            .and_then(|cs| cs.get(CONFIG_CAPACITY))
            .cloned()
        else {
            return Err(AgentError::InvalidConfig(
                "config capacity must be present".into(),
            ));
        };
        config_specs.insert(CONFIG_CAPACITY.to_string(), capacity_spec);

        let mut keys = Vec::with_capacity(n);
        for i in 0..n {
            let key_name = format!("k{}", i);
            let default_key = i.to_string();
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

        spec.inputs = Some((0..n).map(|i| i.to_string()).collect());

        Ok((n as usize, use_ctx, ttl_sec, capacity, keys))
    }

    fn reset_state(&mut self) {
        self.queues = vec![VecDeque::new(); self.n];
        // invalidate_all only marks entries stale; rebuild the cache so parked
        // values are released immediately
        self.ctx_buffers = Cache::builder()
            .max_capacity(self.capacity as u64)
            .time_to_live(Duration::from_secs(self.ttl_sec))
            .build();
    }
}

#[async_trait]
impl AsAgent for ZipToObjectAgent {
    fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (n, use_ctx, ttl_sec, capacity, keys) = Self::update_spec(&mut spec)?;
        let cache = Cache::builder()
            .max_capacity(capacity)
            .time_to_live(Duration::from_secs(ttl_sec))
            .build();
        let data = AgentData::new(ma, id, spec);
        Ok(Self {
            data,
            n,
            use_ctx,
            ttl_sec,
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
        if ttl_sec != self.ttl_sec {
            self.ttl_sec = ttl_sec;
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
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        // Parse port number
        let Some(idx) = port.parse::<usize>().ok().filter(|&i| i < self.n) else {
            return Err(AgentError::InvalidValue(format!(
                "Invalid input port: {}",
                port
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

                return self.output(ctx, PORT_OBJECT, AgentValue::Object(map)).await;
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

            self.output(ctx, PORT_OBJECT, AgentValue::Object(map)).await
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use im::{hashmap, vector};

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
        assert_eq!(result, Some(AgentValue::string("Alice")));

        // Case 2: Intermediate key does not exist (users -> guest)
        let keys_missing = vec!["users", "guest", "name"];
        let result_missing = get_nested_value(&root, &keys_missing);
        assert_eq!(result_missing, None);

        // Case 3: Intermediate path has no such property (users -> admin -> name -> something)
        // "name" is a string, so we cannot traverse deeper -> Should return None
        let keys_not_obj = vec!["users", "admin", "name", "length"];
        let result_not_obj = get_nested_value(&root, &keys_not_obj);
        assert_eq!(result_not_obj, None);

        // Case 4: Empty keys (Should return the root object)
        let keys_empty: Vec<&str> = vec![];
        let result_root = get_nested_value(&root, &keys_empty);
        assert_eq!(result_root, Some(root));
    }

    #[test]
    fn test_get_nested_value_through_message() {
        use modular_agent_core::llm::{Message, Usage};

        // Mattermost Listener shape: { message: Message, user, channel }
        let mut message = Message::user("hello world".to_string());
        message.usage = Some(Usage {
            input_tokens: 3,
            ..Default::default()
        });
        let root = AgentValue::object(hashmap! {
            "message".to_string() => AgentValue::message(message),
            "user".to_string() => AgentValue::string("alice"),
            "channel".to_string() => AgentValue::string("town-square"),
        });

        // Message at leaf
        let result = get_nested_value(&root, &["message"]);
        assert!(matches!(result, Some(AgentValue::Message(_))));

        // Properties through the Message
        assert_eq!(
            get_nested_value(&root, &["message", "content"]),
            Some(AgentValue::string("hello world"))
        );
        assert_eq!(
            get_nested_value(&root, &["message", "role"]),
            Some(AgentValue::string("user"))
        );
        assert_eq!(
            get_nested_value(&root, &["message", "usage", "input_tokens"]),
            Some(AgentValue::integer(3))
        );

        // Missing property on the Message
        assert_eq!(get_nested_value(&root, &["message", "nope"]), None);
    }

    /// Verify if a deeply nested structure (a.b.c) can be auto-generated from an empty state.
    #[test]
    fn test_create_deeply_nested_structure() {
        let mut root = AgentValue::object_default();
        let keys = vec!["users", "admin", "name"];
        let value = AgentValue::string("Alice");

        set_nested_value(&mut root, &keys, value).unwrap();

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

        set_nested_value(&mut root, &keys, value).unwrap();

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
        set_nested_value(&mut root, &keys, new_val).unwrap();

        // Verify
        let app = root.get_mut("app").unwrap();
        let version = app.get_mut("version").unwrap();
        assert_eq!(*version, AgentValue::string("v2"));
    }

    /// Regression test: update_spec must read ttl_sec/capacity by their config
    /// names and keep use_ctx/ttl_sec/capacity in the regenerated configs.
    #[test]
    fn test_zip_to_object_update_spec_preserves_configs() {
        let mut configs = AgentConfigs::new();
        configs.set(CONFIG_N.to_string(), AgentValue::integer(2));
        configs.set(CONFIG_USE_CTX.to_string(), AgentValue::boolean(true));
        configs.set(CONFIG_TTL_SEC.to_string(), AgentValue::integer(120));
        configs.set(CONFIG_CAPACITY.to_string(), AgentValue::integer(5));

        let mut config_specs = AgentConfigSpecs::default();
        for (key, type_, value) in [
            (CONFIG_N, "integer", AgentValue::integer(2)),
            (CONFIG_USE_CTX, "boolean", AgentValue::boolean(false)),
            (CONFIG_TTL_SEC, "integer", AgentValue::integer(60)),
            (CONFIG_CAPACITY, "integer", AgentValue::integer(1000)),
        ] {
            config_specs.insert(
                key.to_string(),
                AgentConfigSpec {
                    value,
                    type_: Some(type_.to_string()),
                    ..Default::default()
                },
            );
        }

        let mut spec = AgentSpec {
            configs: Some(configs),
            config_specs: Some(config_specs),
            ..Default::default()
        };

        let (n, use_ctx, ttl_sec, capacity, keys) =
            ZipToObjectAgent::update_spec(&mut spec).unwrap();
        assert_eq!(n, 2);
        assert!(use_ctx);
        assert_eq!(ttl_sec, 120);
        assert_eq!(capacity, 5);
        assert_eq!(keys, vec!["0".to_string(), "1".to_string()]);

        let configs = spec.configs.as_ref().unwrap();
        assert!(configs.get_bool_or_default(CONFIG_USE_CTX));
        assert_eq!(configs.get_integer_or(CONFIG_TTL_SEC, 0), 120);
        assert_eq!(configs.get_integer_or(CONFIG_CAPACITY, 0), 5);

        let config_specs = spec.config_specs.as_ref().unwrap();
        assert!(config_specs.get(CONFIG_USE_CTX).is_some());
        assert!(config_specs.get(CONFIG_TTL_SEC).is_some());
        assert!(config_specs.get(CONFIG_CAPACITY).is_some());
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
        set_nested_value(&mut root, &keys, value).unwrap();

        // Verify that "tags" remains a string
        let tags = root.get_mut("tags").unwrap();
        assert_eq!(
            *tags,
            AgentValue::object(hashmap! {
                "new_key".to_string() => AgentValue::string("value")
            })
        );
    }

    /// A key path into a Message is an error instead of silently replacing
    /// the Message with an empty Object; the Message survives untouched.
    /// Inserting a Message as a value still works.
    #[test]
    fn test_set_nested_value_message_is_read_only() {
        use modular_agent_core::llm::{Message, Usage};

        let mut message = Message::user("hello".to_string());
        message.usage = Some(Usage {
            input_tokens: 3,
            output_tokens: 7,
            ..Default::default()
        });
        let mut root = AgentValue::object(hashmap! {
            "message".to_string() => AgentValue::message(message),
        });

        assert!(
            set_nested_value(
                &mut root,
                &["message", "content"],
                AgentValue::string("edited"),
            )
            .is_err()
        );
        assert!(
            set_nested_value(
                &mut root,
                &["message", "usage", "input_tokens"],
                AgentValue::integer(99),
            )
            .is_err()
        );

        // A bare Message root rejects writes too, and stays intact
        let mut bare = AgentValue::message(Message::user("hello".to_string()));
        assert!(set_nested_value(&mut bare, &["content"], AgentValue::string("edited")).is_err());
        assert_eq!(bare.as_message().unwrap().text(), "hello");

        // The Message survives the failed writes intact
        let msg = root.get("message").unwrap().as_message().unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(msg.text(), "hello");
        let usage = msg.usage.as_ref().unwrap();
        assert_eq!(usage.input_tokens, 3);
        assert_eq!(usage.output_tokens, 7);

        // A Message as the new value is inserted as-is
        set_nested_value(
            &mut root,
            &["reply"],
            AgentValue::message(Message::assistant("hi".to_string())),
        )
        .unwrap();
        assert!(root.get("reply").unwrap().is_message());
    }

    /// Index segments resolve into arrays anywhere in the path.
    #[test]
    fn test_get_nested_value_through_array() {
        // { "items": [ { "name": "a" }, { "name": "b" } ] }
        let root = AgentValue::object(hashmap! {
            "items".to_string() => AgentValue::array(vector![
                AgentValue::object(hashmap! {
                    "name".to_string() => AgentValue::string("a"),
                }),
                AgentValue::object(hashmap! {
                    "name".to_string() => AgentValue::string("b"),
                }),
            ]),
        });

        assert_eq!(
            get_nested_value(&root, &["items", "0", "name"]),
            Some(AgentValue::string("a"))
        );
        assert_eq!(
            get_nested_value(&root, &["items", "1", "name"]),
            Some(AgentValue::string("b"))
        );

        // Out-of-range index and non-index key on the array
        assert_eq!(get_nested_value(&root, &["items", "2", "name"]), None);
        assert_eq!(get_nested_value(&root, &["items", "name"]), None);

        // Index on a root array
        let arr = AgentValue::array(vector![AgentValue::string("x"), AgentValue::string("y"),]);
        assert_eq!(
            get_nested_value(&arr, &["1"]),
            Some(AgentValue::string("y"))
        );
    }

    /// Writing through an index segment updates the element in place instead
    /// of destroying the array.
    #[test]
    fn test_set_nested_value_through_array() {
        let mut root = AgentValue::object(hashmap! {
            "items".to_string() => AgentValue::array(vector![
                AgentValue::object(hashmap! {
                    "name".to_string() => AgentValue::string("a"),
                }),
                AgentValue::object(hashmap! {
                    "name".to_string() => AgentValue::string("b"),
                }),
            ]),
        });

        // Field inside an element
        set_nested_value(
            &mut root,
            &["items", "0", "name"],
            AgentValue::string("edited"),
        )
        .unwrap();
        let items = root.get("items").unwrap().as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].get_str("name"), Some("edited"));
        assert_eq!(items[1].get_str("name"), Some("b"));

        // Direct element replacement
        set_nested_value(&mut root, &["items", "1"], AgentValue::integer(42)).unwrap();
        let items = root.get("items").unwrap().as_array().unwrap();
        assert_eq!(items[1], AgentValue::integer(42));
    }

    /// An out-of-range index or a non-index key on an array is an error
    /// instead of silently destroying the array.
    #[test]
    fn test_set_nested_value_array_errors() {
        let mut root = AgentValue::object(hashmap! {
            "items".to_string() => AgentValue::array(vector![AgentValue::string("a")]),
        });

        assert!(set_nested_value(&mut root, &["items", "5"], AgentValue::integer(1)).is_err());
        assert!(
            set_nested_value(&mut root, &["items", "5", "name"], AgentValue::integer(1)).is_err()
        );
        assert!(set_nested_value(&mut root, &["items", "foo"], AgentValue::integer(1)).is_err());

        // The array survives the failed writes
        let items = root.get("items").unwrap().as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], AgentValue::string("a"));
    }

    /// A root array broadcasts non-index key paths over its elements, but an
    /// index-first path addresses the array itself.
    #[test]
    fn test_get_value_extract_root_array() {
        let arr = AgentValue::array(vector![
            AgentValue::object(hashmap! {
                "name".to_string() => AgentValue::string("a"),
            }),
            AgentValue::object(hashmap! {
                "name".to_string() => AgentValue::string("b"),
            }),
        ]);

        // Broadcast: "name" applies to each element
        assert_eq!(
            GetValueAgent::extract(arr.clone(), &["name".to_string()]),
            AgentValue::array(vector![AgentValue::string("a"), AgentValue::string("b")]),
        );

        // Index-first: "1.name" addresses the array
        assert_eq!(
            GetValueAgent::extract(arr, &["1".to_string(), "name".to_string()]),
            AgentValue::string("b"),
        );

        // Deliberate precedence: even when the elements themselves carry a
        // numeric string key, an index-first path addresses the array — the
        // key "0" returns the first element, not a broadcast of each
        // element's "0" property.
        let numeric_keyed = AgentValue::array(vector![
            AgentValue::object(hashmap! {
                "0".to_string() => AgentValue::string("a"),
            }),
            AgentValue::object(hashmap! {
                "0".to_string() => AgentValue::string("b"),
            }),
        ]);
        assert_eq!(
            GetValueAgent::extract(numeric_keyed, &["0".to_string()]),
            AgentValue::object(hashmap! {
                "0".to_string() => AgentValue::string("a"),
            }),
        );
    }

    /// An empty key config is a no-op selection, like Get Value / To Object.
    #[test]
    fn test_set_value_update_spec_empty_key() {
        let mut spec = AgentSpec::default();
        let (target_keys, target_value) = SetValueAgent::update_spec(&mut spec).unwrap();
        assert!(target_keys.is_empty());
        assert_eq!(target_value, AgentValue::Unit);
    }
}
