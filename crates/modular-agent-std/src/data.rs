use std::{collections::VecDeque, vec};

use agent_stream_kit::{
    ASKit, Agent, AgentConfigSpec, AgentConfigSpecs, AgentConfigs, AgentContext, AgentData,
    AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent, askit_agent, async_trait,
};

use crate::ctx_utils::find_first_common_key;

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
}

#[async_trait]
impl AsAgent for GetValueAgent {
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
        let key = self.configs()?.get_string(CONFIG_KEY)?;
        if key.is_empty() {
            return Ok(());
        }
        let keys = key.split('.').collect::<Vec<_>>();

        if value.is_object() {
            if let Some(value) = get_nested_value(&value, &keys) {
                self.try_output(ctx, PIN_VALUE, value.to_owned())?;
            } else {
                self.try_output(ctx, PIN_VALUE, AgentValue::unit())?;
            }
        } else if value.is_array() {
            let mut out_arr = Vec::new();
            for v in value
                .as_array()
                .ok_or_else(|| AgentError::InvalidValue("failed as_array".to_string()))?
            {
                let value = get_nested_value(v, &keys);
                if let Some(v) = value {
                    out_arr.push(v.to_owned());
                } else {
                    out_arr.push(AgentValue::unit());
                }
            }
            self.try_output(ctx, PIN_VALUE, AgentValue::array(out_arr))?;
        }

        Ok(())
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
}

#[async_trait]
impl AsAgent for SetValueAgent {
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
        // parse key
        let key = self.configs()?.get_string(CONFIG_KEY)?;
        if key.is_empty() {
            return Ok(());
        }
        let keys = key.split('.').collect::<Vec<_>>();

        let v = self.configs()?.get(CONFIG_VALUE)?;
        let mut value = value;
        set_nested_value(&mut value, keys, v.clone());

        self.try_output(ctx, PIN_VALUE, value)?;

        Ok(())
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
}

#[async_trait]
impl AsAgent for ToObjectAgent {
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
        let key = self.configs()?.get_string(CONFIG_KEY)?;
        if key.is_empty() {
            return Ok(());
        }

        let keys = key.split('.').collect::<Vec<_>>();
        let mut new_value = AgentValue::object_default();
        set_nested_value(&mut new_value, keys, value);

        self.try_output(ctx, PIN_VALUE, new_value)?;
        Ok(())
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
        self.try_output(ctx, PIN_JSON, AgentValue::string(json))?;
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
        self.try_output(ctx, PIN_VALUE, value)?;
        Ok(())
    }
}

fn get_nested_value<'a>(value: &'a AgentValue, keys: &[&str]) -> Option<&'a AgentValue> {
    let mut current_value = value;
    for key in keys {
        let obj = current_value.as_object()?;
        current_value = obj.get(*key)?;
    }
    Some(current_value)
}

fn set_nested_value<'a>(value: &'a mut AgentValue, keys: Vec<&str>, new_value: AgentValue) {
    let mut current_value = value;

    if keys.is_empty() {
        return;
    }

    for key in keys[..keys.len() - 1].iter() {
        if !current_value.is_object() {
            return;
        }

        if current_value.get(*key).is_none() {
            let _ = current_value.set((*key).to_string(), AgentValue::object_default());
        }

        if let Some(v) = current_value.get_mut(*key) {
            current_value = v;
        } else {
            // just in case
            return;
        }
    }

    let last_key = keys.last().unwrap();
    if let Some(obj) = current_value.as_object_mut() {
        obj.insert((*last_key).to_string(), new_value);
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
)]
struct ZipToObjectAgent {
    data: AgentData,
    n: usize,
    use_ctx: bool,
    input_values: Vec<Vec<AgentValue>>,
    ctx_input_values: Vec<VecDeque<(String, AgentValue)>>,
}

impl ZipToObjectAgent {
    fn update_spec(spec: &mut AgentSpec) -> Result<(usize, bool), AgentError> {
        let mut n = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_integer_or(CONFIG_N, 2))
            .unwrap_or(2);
        if n < 1 {
            n = 1;
        }

        let use_ctx = spec
            .configs
            .as_ref()
            .map(|cfg| cfg.get_bool_or_default(CONFIG_USE_CTX))
            .unwrap_or(false);

        let mut configs = AgentConfigs::new();
        let mut config_specs = AgentConfigSpecs::default();

        configs.set(CONFIG_N.to_string(), AgentValue::integer(n));
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

        for i in 1..=n {
            let key_cfg = format!("k{}", i);
            let v = spec
                .configs
                .as_ref()
                .map(|cfg| cfg.get_string_or_default(&key_cfg))
                .unwrap_or_default();
            configs.set(key_cfg.clone(), AgentValue::string(v));
            config_specs.insert(
                key_cfg,
                AgentConfigSpec {
                    value: AgentValue::string_default(),
                    type_: Some("string".to_string()),
                    ..Default::default()
                },
            );
        }

        spec.configs = Some(configs);
        spec.config_specs = Some(config_specs);

        Ok((n as usize, use_ctx))
    }
}

#[async_trait]
impl AsAgent for ZipToObjectAgent {
    fn new(askit: ASKit, id: String, mut spec: AgentSpec) -> Result<Self, AgentError> {
        let (n, use_ctx) = Self::update_spec(&mut spec)?;
        let data = AgentData::new(askit, id, spec);
        Ok(Self {
            data,
            n,
            input_values: vec![Vec::new(); n],
            use_ctx,
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

            let mut obj = AgentValue::object_default();
            for j in 0..self.n {
                let key_cfg = format!("k{}", j + 1);
                let key = self.configs()?.get_string_or_default(&key_cfg);
                let val = self.ctx_input_values[j]
                    .front()
                    .map(|(_, v)| v.clone())
                    .ok_or_else(|| AgentError::InvalidValue("missing queued value".into()))?;
                obj.set(key, val)?;
            }
            for q in self.ctx_input_values.iter_mut() {
                q.pop_front();
            }
            return self.try_output(ctx, PIN_OBJECT, obj);
        }

        self.input_values[i].push(value);

        // Check if some input is still missing
        if self.input_values.iter().any(|v| v.is_empty()) {
            return Ok(());
        }

        // All inputs are present, emit an object
        let mut obj = AgentValue::object_default();
        for j in 0..self.n {
            let key_cfg = format!("k{}", j + 1);
            let key = self.configs()?.get_string_or_default(&key_cfg);
            let val = self.input_values[j].remove(0);
            obj.set(key, val)?;
        }
        self.try_output(ctx, PIN_OBJECT, obj)
    }
}

#[cfg(test)]
mod tests {
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

    /// Test 1: Verify if a deeply nested structure (a.b.c) can be auto-generated from an empty state.
    /// This confirms the fix for the previous bug (failure to traverse down levels).
    #[test]
    fn test_create_deeply_nested_structure() {
        let mut root = AgentValue::object_default();
        let keys = vec!["users", "admin", "name"];
        let value = AgentValue::string("Alice");

        set_nested_value(&mut root, keys, value);

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

    /// Test 2: Verify if a new key can be added without breaking existing structures.
    #[test]
    fn test_add_to_existing_structure() {
        let mut root = AgentValue::object_default();
        // Pre-create { "config": {} }
        root.set("config".to_string(), AgentValue::object_default())
            .unwrap();

        let keys = vec!["config", "timeout"];
        let value = AgentValue::string("30s");

        set_nested_value(&mut root, keys, value);

        // Verify
        let config = root.get_mut("config").unwrap();
        let timeout = config.get_mut("timeout").unwrap();
        assert_eq!(*timeout, AgentValue::string("30s"));
    }

    /// Test 3: Verify if an existing value can be overwritten.
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
        set_nested_value(&mut root, keys, new_val);

        // Verify
        let app = root.get_mut("app").unwrap();
        let version = app.get_mut("version").unwrap();
        assert_eq!(*version, AgentValue::string("v2"));
    }

    /// Test 4: Verify if the operation stops safely when an intermediate path is not an object.
    /// Example: Try setting ["tags", "new_key"] against { "tags": "immutable_string" }
    #[test]
    fn test_stop_if_path_is_not_object() {
        let mut root = AgentValue::object_default();
        // "tags" is a string, not an object
        root.set("tags".to_string(), AgentValue::string("some_string"))
            .unwrap();

        let keys = vec!["tags", "new_key"];
        let value = AgentValue::string("value");

        // Ensure it returns without crashing
        set_nested_value(&mut root, keys, value);

        // Verify that "tags" remains a string
        let tags = root.get_mut("tags").unwrap();
        assert_eq!(*tags, AgentValue::string("some_string"));
    }
}
