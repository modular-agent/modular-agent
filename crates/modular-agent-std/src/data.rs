use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentError, AgentOutput, AgentValue, AsAgent,
    AsAgentData, async_trait,
};
use askit_macros::askit_agent;

static CATEGORY: &str = "Std/Data";

static PIN_DATA: &str = "data";
static PIN_JSON: &str = "json";
static PIN_VALUE: &str = "value";

static CONFIG_KEY: &str = "key";

// Get Value
#[askit_agent(
    title = "Get Value",
    category = CATEGORY,
    inputs = [PIN_DATA],
    outputs = [PIN_VALUE],
    string_config(name = CONFIG_KEY)
)]
struct GetValueAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for GetValueAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
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
    inputs = [PIN_DATA, PIN_VALUE],
    outputs = [PIN_DATA],
    string_config(name = CONFIG_KEY)
)]
struct SetValueAgent {
    data: AsAgentData,
    input_data: Option<AgentValue>,
    input_value: Option<AgentValue>,
    current_id: usize,
}

#[async_trait]
impl AsAgent for SetValueAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            input_data: None,
            input_value: None,
            current_id: 0,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
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
            self.input_data = None;
            self.input_value = None;
        }

        // Store input data or value
        if pin == PIN_DATA {
            if value.is_object() {
                self.input_data = Some(value);
            }
        } else if pin == PIN_VALUE {
            self.input_value = Some(value);
        }
        if self.input_data.is_none() || self.input_value.is_none() {
            return Ok(());
        }

        // parse key
        let key = self.configs()?.get_string(CONFIG_KEY)?;
        if key.is_empty() {
            return Ok(());
        }
        let keys = key.split('.').collect::<Vec<_>>();

        // set value
        let new_value = self.input_value.take().unwrap();
        let mut value = self.input_data.take().unwrap();
        set_nested_value(&mut value, keys, new_value);

        self.try_output(ctx, PIN_DATA, value)?;

        Ok(())
    }
}

// To Object
#[askit_agent(
    title = "To Object",
    category = CATEGORY,
    inputs = [PIN_DATA],
    outputs = [PIN_DATA],
    string_config(name = CONFIG_KEY)
)]
struct ToObjectAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ToObjectAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
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

        self.try_output(ctx, PIN_DATA, new_value)?;
        Ok(())
    }
}

// To JSON
#[askit_agent(
    title = "To JSON",
    category = CATEGORY,
    inputs = [PIN_DATA],
    outputs = [PIN_JSON]
)]
struct ToJsonAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ToJsonAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
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
    outputs = [PIN_DATA]
)]
struct FromJsonAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for FromJsonAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
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
        self.try_output(ctx, PIN_DATA, value)?;
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
