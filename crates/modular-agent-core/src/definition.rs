use std::ops::Not;

use serde::{Deserialize, Serialize};

use crate::FnvIndexMap;
use crate::agent::Agent;
use crate::modular_agent::ModularAgent;
use crate::error::AgentError;
use crate::id::new_id;
use crate::spec::AgentSpec;
use crate::value::AgentValue;

/// A map of agent definition names to their definitions.
pub type AgentDefinitions = FnvIndexMap<String, AgentDefinition>;

/// The definition (blueprint) of an agent type.
///
/// An agent definition describes the metadata and capabilities of an agent type,
/// including its ports, configuration options, and factory function.
/// Multiple agent instances can be created from a single definition.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentDefinition {
    /// The kind/category identifier for this agent type (e.g., "Agent", "Board").
    pub kind: String,

    /// Unique name of this agent definition.
    pub name: String,

    /// Human-readable title for display in UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Whether to hide the title in UI.
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub hide_title: bool,

    /// Description of what this agent does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Category path for organizing agents (e.g., "Flow/Control").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Default input port names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<String>>,

    /// Default output port names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<String>>,

    /// Configuration specifications for this agent type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configs: Option<AgentConfigSpecs>,

    /// Global configuration specifications (shared across instances).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_configs: Option<AgentGlobalConfigSpecs>,

    /// Whether to run this agent on a native OS thread instead of the async runtime.
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub native_thread: bool,

    /// Factory function to create new agent instances.
    #[serde(skip)]
    pub new_boxed: Option<AgentNewBoxedFn>,
}

/// A map of configuration keys to their specifications.
pub type AgentConfigSpecs = FnvIndexMap<String, AgentConfigSpec>;

/// A map of global configuration keys to their specifications.
pub type AgentGlobalConfigSpecs = FnvIndexMap<String, AgentConfigSpec>;

/// Specification for a configuration entry.
///
/// Defines the metadata for a configuration option, including its default value,
/// type, display properties, and access control.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentConfigSpec {
    /// Default value for this configuration.
    pub value: AgentValue,

    /// Type of this configuration (e.g., "string", "integer", "boolean").
    #[serde(rename = "type")]
    pub type_: Option<String>,

    /// Human-readable title for display in UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Whether to hide the title in UI.
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub hide_title: bool,

    /// Description of this configuration option.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether this configuration entry should be hidden from the user interface.
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub hidden: bool,

    /// Whether this configuration entry is read-only.
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub readonly: bool,

    /// Whether this configuration entry should only be shown in the detail view.
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub detail: bool,
}

/// Factory function type for creating new agent instances.
///
/// Takes a `ModularAgent` orchestrator, agent ID, and spec, and returns
/// a boxed `Agent` trait object or an error.
pub type AgentNewBoxedFn =
    fn(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Box<dyn Agent>, AgentError>;

impl AgentDefinition {
    /// Creates a new agent definition.
    ///
    /// # Arguments
    ///
    /// * `kind` - The kind/category identifier (e.g., "std", "llm")
    /// * `name` - Unique name for this agent definition
    /// * `new_boxed` - Optional factory function to create agent instances
    pub fn new(
        kind: impl Into<String>,
        name: impl Into<String>,
        new_boxed: Option<AgentNewBoxedFn>,
    ) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
            new_boxed,
            ..Default::default()
        }
    }

    /// Sets the display title. Returns self for method chaining.
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Hides the title in UI. Returns self for method chaining.
    pub fn hide_title(mut self) -> Self {
        self.hide_title = true;
        self
    }

    /// Sets the description. Returns self for method chaining.
    pub fn description(mut self, description: &str) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the category path. Returns self for method chaining.
    pub fn category(mut self, category: &str) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Sets the input port names. Returns self for method chaining.
    pub fn inputs(mut self, inputs: Vec<&str>) -> Self {
        self.inputs = Some(inputs.into_iter().map(|x| x.into()).collect());
        self
    }

    /// Sets the output port names. Returns self for method chaining.
    pub fn outputs(mut self, outputs: Vec<&str>) -> Self {
        self.outputs = Some(outputs.into_iter().map(|x| x.into()).collect());
        self
    }

    // Config Spec

    /// Sets all configuration specifications at once.
    pub fn configs(mut self, configs: Vec<(&str, AgentConfigSpec)>) -> Self {
        self.configs = Some(configs.into_iter().map(|(k, v)| (k.into(), v)).collect());
        self
    }

    /// Adds a unit (trigger/signal) configuration.
    pub fn unit_config(self, key: &str) -> Self {
        self.unit_config_with(key, |entry| entry)
    }

    /// Adds a unit configuration with customization callback.
    pub fn unit_config_with<F>(self, key: &str, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.config_type_with(key, (), "unit", f)
    }

    /// Adds a boolean configuration with a default value.
    pub fn boolean_config(self, key: &str, default: bool) -> Self {
        self.boolean_config_with(key, default, |entry| entry)
    }

    /// Adds a boolean configuration with customization callback.
    pub fn boolean_config_with<F>(self, key: &str, default: bool, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.config_type_with(key, default, "boolean", f)
    }

    /// Adds a boolean configuration with default value `false`.
    pub fn boolean_config_default(self, key: &str) -> Self {
        self.boolean_config(key, false)
    }

    /// Adds an integer configuration with a default value.
    pub fn integer_config(self, key: &str, default: i64) -> Self {
        self.integer_config_with(key, default, |entry| entry)
    }

    /// Adds an integer configuration with customization callback.
    pub fn integer_config_with<F>(self, key: &str, default: i64, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.config_type_with(key, default, "integer", f)
    }

    /// Adds an integer configuration with default value `0`.
    pub fn integer_config_default(self, key: &str) -> Self {
        self.integer_config(key, 0)
    }

    /// Adds a number (f64) configuration with a default value.
    pub fn number_config(self, key: &str, default: f64) -> Self {
        self.number_config_with(key, default, |entry| entry)
    }

    /// Adds a number configuration with customization callback.
    pub fn number_config_with<F>(self, key: &str, default: f64, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.config_type_with(key, default, "number", f)
    }

    /// Adds a number configuration with default value `0.0`.
    pub fn number_config_default(self, key: &str) -> Self {
        self.number_config(key, 0.0)
    }

    /// Adds a string configuration with a default value.
    pub fn string_config(self, key: &str, default: impl Into<String>) -> Self {
        self.string_config_with(key, default, |entry| entry)
    }

    /// Adds a string configuration with customization callback.
    pub fn string_config_with<F>(self, key: &str, default: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        let default = default.into();
        self.config_type_with(key, AgentValue::string(default), "string", f)
    }

    /// Adds a string configuration with empty default value.
    pub fn string_config_default(self, key: &str) -> Self {
        self.string_config(key, "")
    }

    /// Adds a multiline text configuration with a default value.
    pub fn text_config(self, key: &str, default: impl Into<String>) -> Self {
        self.text_config_with(key, default, |entry| entry)
    }

    /// Adds a text configuration with customization callback.
    pub fn text_config_with<F>(self, key: &str, default: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        let default = default.into();
        self.config_type_with(key, AgentValue::string(default), "text", f)
    }

    /// Adds a text configuration with empty default value.
    pub fn text_config_default(self, key: &str) -> Self {
        self.text_config(key, "")
    }

    /// Adds an array configuration with a default value.
    pub fn array_config(self, key: &str, default: impl Into<AgentValue>) -> Self {
        self.array_config_with(key, default, |entry| entry)
    }

    /// Adds an array configuration with customization callback.
    pub fn array_config_with<V: Into<AgentValue>, F>(self, key: &str, default: V, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.config_type_with(key, default, "array", f)
    }

    /// Adds an array configuration with empty default value.
    pub fn array_config_default(self, key: &str) -> Self {
        self.array_config(key, AgentValue::array_default())
    }

    /// Adds an object configuration with a default value.
    pub fn object_config<V: Into<AgentValue>>(self, key: &str, default: V) -> Self {
        self.object_config_with(key, default, |entry| entry)
    }

    /// Adds an object configuration with customization callback.
    pub fn object_config_with<V: Into<AgentValue>, F>(self, key: &str, default: V, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.config_type_with(key, default, "object", f)
    }

    /// Adds an object configuration with empty default value.
    pub fn object_config_default(self, key: &str) -> Self {
        self.object_config(key, AgentValue::object_default())
    }

    /// Adds a custom-typed configuration with customization callback.
    pub fn custom_config_with<V: Into<AgentValue>, F>(
        self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.config_type_with(key, default, type_, f)
    }

    /// Internal: adds a configuration with specified type.
    fn config_type_with<V: Into<AgentValue>, F>(
        mut self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        let entry = AgentConfigSpec::new(default, type_);
        self.insert_config_entry(key.into(), f(entry));
        self
    }

    fn insert_config_entry(&mut self, key: String, entry: AgentConfigSpec) {
        if let Some(configs) = self.configs.as_mut() {
            configs.insert(key, entry);
        } else {
            let mut map = FnvIndexMap::default();
            map.insert(key, entry);
            self.configs = Some(map);
        }
    }

    // Global Configs
    //
    // Global configurations are shared across all instances of this agent type.

    /// Sets all global configuration specifications at once.
    pub fn global_configs(mut self, configs: Vec<(&str, AgentConfigSpec)>) -> Self {
        self.global_configs = Some(configs.into_iter().map(|(k, v)| (k.into(), v)).collect());
        self
    }

    /// Adds a boolean global configuration.
    pub fn boolean_global_config(self, key: &str, default: bool) -> Self {
        self.boolean_global_config_with(key, default, |entry| entry)
    }

    /// Adds a boolean global configuration with customization callback.
    pub fn boolean_global_config_with<F>(self, key: &str, default: bool, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.global_config_type_with(key, default, "boolean", f)
    }

    /// Adds an integer global configuration.
    pub fn integer_global_config(self, key: &str, default: i64) -> Self {
        self.integer_global_config_with(key, default, |entry| entry)
    }

    /// Adds an integer global configuration with customization callback.
    pub fn integer_global_config_with<F>(self, key: &str, default: i64, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.global_config_type_with(key, default, "integer", f)
    }

    /// Adds a number (f64) global configuration.
    pub fn number_global_config(self, key: &str, default: f64) -> Self {
        self.number_global_config_with(key, default, |entry| entry)
    }

    /// Adds a number global configuration with customization callback.
    pub fn number_global_config_with<F>(self, key: &str, default: f64, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.global_config_type_with(key, default, "number", f)
    }

    /// Adds a string global configuration.
    pub fn string_global_config(self, key: &str, default: impl Into<String>) -> Self {
        self.string_global_config_with(key, default, |entry| entry)
    }

    /// Adds a string global configuration with customization callback.
    pub fn string_global_config_with<F>(self, key: &str, default: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        let default = default.into();
        self.global_config_type_with(key, AgentValue::string(default), "string", f)
    }

    /// Adds a multiline text global configuration.
    pub fn text_global_config(self, key: &str, default: impl Into<String>) -> Self {
        self.text_global_config_with(key, default, |entry| entry)
    }

    /// Adds a text global configuration with customization callback.
    pub fn text_global_config_with<F>(self, key: &str, default: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        let default = default.into();
        self.global_config_type_with(key, AgentValue::string(default), "text", f)
    }

    /// Adds an array global configuration.
    pub fn array_global_config(self, key: &str, default: impl Into<AgentValue>) -> Self {
        self.array_global_config_with(key, default, |entry| entry)
    }

    /// Adds an array global configuration with customization callback.
    pub fn array_global_config_with<V: Into<AgentValue>, F>(
        self,
        key: &str,
        default: V,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.global_config_type_with(key, default, "array", f)
    }

    /// Adds an array global configuration with empty default value.
    pub fn array_global_config_default(self, key: &str) -> Self {
        self.array_global_config(key, AgentValue::array_default())
    }

    /// Adds an object global configuration.
    pub fn object_global_config<V: Into<AgentValue>>(self, key: &str, default: V) -> Self {
        self.object_global_config_with(key, default, |entry| entry)
    }

    /// Adds an object global configuration with customization callback.
    pub fn object_global_config_with<V: Into<AgentValue>, F>(
        self,
        key: &str,
        default: V,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.global_config_type_with(key, default, "object", f)
    }

    /// Adds a custom-typed global configuration with customization callback.
    pub fn custom_global_config_with<V: Into<AgentValue>, F>(
        self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        self.global_config_type_with(key, default, type_, f)
    }

    fn global_config_type_with<V: Into<AgentValue>, F>(
        mut self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigSpec) -> AgentConfigSpec,
    {
        let entry = AgentConfigSpec::new(default, type_);
        self.insert_global_config_entry(key.into(), f(entry));
        self
    }

    fn insert_global_config_entry(&mut self, key: String, entry: AgentConfigSpec) {
        if let Some(configs) = self.global_configs.as_mut() {
            configs.insert(key, entry);
        } else {
            let mut map = FnvIndexMap::default();
            map.insert(key, entry);
            self.global_configs = Some(map);
        }
    }

    /// Configures this agent to run on a native OS thread.
    ///
    /// Use this for agents that perform blocking I/O or CPU-intensive operations
    /// that would block the async runtime.
    pub fn use_native_thread(mut self) -> Self {
        self.native_thread = true;
        self
    }

    /// Creates a new agent specification from this definition.
    ///
    /// Generates a unique ID and copies the definition's ports and configs
    /// to create a new instance specification.
    pub fn to_spec(&self) -> AgentSpec {
        AgentSpec {
            id: new_id(),
            def_name: self.name.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            configs: self.configs.as_ref().map(|cfgs| {
                cfgs.iter()
                    .map(|(k, v)| (k.clone(), v.value.clone()))
                    .collect()
            }),
            config_specs: self.configs.clone(),
            #[allow(deprecated)]
            enabled: false,
            disabled: false,
            extensions: FnvIndexMap::default(),
        }
    }
}

impl AgentConfigSpec {
    /// Creates a new configuration specification.
    ///
    /// # Arguments
    ///
    /// * `value` - Default value for this configuration
    /// * `type_` - Type identifier (e.g., "string", "integer", "boolean")
    pub fn new<V: Into<AgentValue>>(value: V, type_: &str) -> Self {
        Self {
            value: value.into(),
            type_: Some(type_.into()),
            ..Default::default()
        }
    }

    /// Sets the display title. Returns self for method chaining.
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Hides the title in UI. Returns self for method chaining.
    pub fn hide_title(mut self) -> Self {
        self.hide_title = true;
        self
    }

    /// Sets the description. Returns self for method chaining.
    pub fn description(mut self, description: &str) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Marks this config as hidden from UI. Returns self for method chaining.
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    /// Marks this config as read-only. Returns self for method chaining.
    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    /// Marks this config as detail-only (shown only in detail view). Returns self for method chaining.
    pub fn detail(mut self) -> Self {
        self.detail = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use im::{hashmap, vector};

    use super::*;

    #[test]
    fn test_agent_definition() {
        let def = AgentDefinition::default();
        assert_eq!(def.name, "");
    }

    #[test]
    fn test_agent_definition_new_default() {
        let def = AgentDefinition::new(
            "test",
            "echo",
            Some(|_app, _id, _spec| Err(AgentError::NotImplemented("Echo agent".into()))),
        );

        assert_eq!(def.kind, "test");
        assert_eq!(def.name, "echo");
        assert!(def.title.is_none());
        assert!(def.category.is_none());
        assert!(def.inputs.is_none());
        assert!(def.outputs.is_none());
        assert!(def.configs.is_none());
    }

    #[test]
    fn test_agent_definition_new() {
        let def = echo_agent_definition();

        assert_eq!(def.kind, "test");
        assert_eq!(def.name, "echo");
        assert_eq!(def.title.unwrap(), "Echo");
        assert_eq!(def.category.unwrap(), "Test");
        assert_eq!(def.inputs.unwrap(), vec!["in"]);
        assert_eq!(def.outputs.unwrap(), vec!["out"]);
        let default_configs = def.configs.unwrap();
        assert_eq!(default_configs.len(), 2);
        let entry = default_configs.get("value").unwrap();
        assert_eq!(entry.value, AgentValue::string("abc"));
        assert_eq!(entry.type_.as_ref().unwrap(), "string");
        assert_eq!(entry.title.as_ref().unwrap(), "display_title");
        assert_eq!(entry.description.as_ref().unwrap(), "display_description");
        assert_eq!(entry.hide_title, false);
        assert_eq!(entry.readonly, true);
        assert_eq!(entry.detail, true);
        let entry = default_configs.get("hide_title_value").unwrap();
        assert_eq!(entry.value, AgentValue::integer(1));
        assert_eq!(entry.type_.as_ref().unwrap(), "integer");
        assert_eq!(entry.title, None);
        assert_eq!(entry.description, None);
        assert_eq!(entry.hide_title, true);
        assert_eq!(entry.readonly, true);
        assert_eq!(entry.detail, false);
    }

    #[test]
    fn test_serialize_agent_definition() {
        let def = AgentDefinition::new(
            "test",
            "echo",
            Some(|_app, _id, _spec| Err(AgentError::NotImplemented("Echo agent".into()))),
        );
        let json = serde_json::to_string(&def).unwrap();
        assert_eq!(json, r#"{"kind":"test","name":"echo"}"#);
    }

    #[test]
    fn test_serialize_echo_agent_definition() {
        let def = echo_agent_definition();
        let json = serde_json::to_string(&def).unwrap();
        print!("{}", json);
        assert_eq!(
            json,
            r#"{"kind":"test","name":"echo","title":"Echo","category":"Test","inputs":["in"],"outputs":["out"],"configs":{"value":{"value":"abc","type":"string","title":"display_title","description":"display_description","readonly":true,"detail":true},"hide_title_value":{"value":1,"type":"integer","hide_title":true,"readonly":true}}}"#
        );
    }

    #[test]
    fn test_deserialize_echo_agent_definition() {
        let json = r#"{"kind":"test","name":"echo","title":"Echo","category":"Test","inputs":["in"],"outputs":["out"],"configs":{"value":{"value":"abc","type":"string","title":"display_title","description":"display_description","readonly":true,"detail":true},"hide_title_value":{"value":1,"type":"integer","hide_title":true,"readonly":true}}}"#;
        let def: AgentDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(def.kind, "test");
        assert_eq!(def.name, "echo");
        assert_eq!(def.title.unwrap(), "Echo");
        assert_eq!(def.category.unwrap(), "Test");
        assert_eq!(def.inputs.unwrap(), vec!["in"]);
        assert_eq!(def.outputs.unwrap(), vec!["out"]);
        let default_configs = def.configs.unwrap();
        assert_eq!(default_configs.len(), 2);
        let (key, entry) = default_configs.get_index(0).unwrap();
        assert_eq!(key, "value");
        assert_eq!(entry.type_.as_ref().unwrap(), "string");
        assert_eq!(entry.title.as_ref().unwrap(), "display_title");
        assert_eq!(entry.description.as_ref().unwrap(), "display_description");
        assert_eq!(entry.hide_title, false);
        assert_eq!(entry.detail, true);
        let (key, entry) = default_configs.get_index(1).unwrap();
        assert_eq!(key, "hide_title_value");
        assert_eq!(entry.type_.as_ref().unwrap(), "integer");
        assert_eq!(entry.title, None);
        assert_eq!(entry.description, None);
        assert_eq!(entry.hide_title, true);
    }

    #[test]
    fn test_default_config_helpers() {
        let custom_object_value =
            AgentValue::object(hashmap! {"key".into() => AgentValue::string("value")});
        let custom_array_value =
            AgentValue::array(vector![AgentValue::integer(1), AgentValue::string("two")]);

        let def = AgentDefinition::new("test", "helpers", None)
            .unit_config("unit_value")
            .boolean_config_default("boolean_value")
            .boolean_config("boolean_custom", true)
            .integer_config_default("integer_value")
            .integer_config("integer_custom", 42)
            .number_config_default("number_value")
            .number_config("number_custom", 1.5)
            .string_config_default("string_default")
            .string_config("string_value", "value")
            .text_config_default("text_value")
            .text_config("text_custom", "custom")
            .array_config_default("array_value")
            .array_config("array_custom", custom_array_value.clone())
            .object_config_default("object_value")
            .object_config("object_custom", custom_object_value.clone());

        let configs = def.configs.clone().expect("default configs should exist");
        assert_eq!(configs.len(), 15);
        let config_map: std::collections::HashMap<_, _> = configs.into_iter().collect();

        let unit_entry = config_map.get("unit_value").unwrap();
        assert_eq!(unit_entry.type_.as_deref(), Some("unit"));
        assert_eq!(unit_entry.value, AgentValue::unit());

        let boolean_entry = config_map.get("boolean_value").unwrap();
        assert_eq!(boolean_entry.type_.as_deref(), Some("boolean"));
        assert_eq!(boolean_entry.value, AgentValue::boolean(false));

        let boolean_custom_entry = config_map.get("boolean_custom").unwrap();
        assert_eq!(boolean_custom_entry.type_.as_deref(), Some("boolean"));
        assert_eq!(boolean_custom_entry.value, AgentValue::boolean(true));

        let integer_entry = config_map.get("integer_value").unwrap();
        assert_eq!(integer_entry.type_.as_deref(), Some("integer"));
        assert_eq!(integer_entry.value, AgentValue::integer(0));

        let integer_custom_entry = config_map.get("integer_custom").unwrap();
        assert_eq!(integer_custom_entry.type_.as_deref(), Some("integer"));
        assert_eq!(integer_custom_entry.value, AgentValue::integer(42));

        let number_entry = config_map.get("number_value").unwrap();
        assert_eq!(number_entry.type_.as_deref(), Some("number"));
        assert_eq!(number_entry.value, AgentValue::number(0.0));

        let number_custom_entry = config_map.get("number_custom").unwrap();
        assert_eq!(number_custom_entry.type_.as_deref(), Some("number"));
        assert_eq!(number_custom_entry.value, AgentValue::number(1.5));

        let string_default_entry = config_map.get("string_default").unwrap();
        assert_eq!(string_default_entry.type_.as_deref(), Some("string"));
        assert_eq!(string_default_entry.value, AgentValue::string(""));

        let string_entry = config_map.get("string_value").unwrap();
        assert_eq!(string_entry.type_.as_deref(), Some("string"));
        assert_eq!(string_entry.value, AgentValue::string("value"));

        let text_entry = config_map.get("text_value").unwrap();
        assert_eq!(text_entry.type_.as_deref(), Some("text"));
        assert_eq!(text_entry.value, AgentValue::string(""));

        let text_custom_entry = config_map.get("text_custom").unwrap();
        assert_eq!(text_custom_entry.type_.as_deref(), Some("text"));
        assert_eq!(text_custom_entry.value, AgentValue::string("custom"));

        let array_entry = config_map.get("array_value").unwrap();
        assert_eq!(array_entry.type_.as_deref(), Some("array"));
        assert_eq!(array_entry.value, AgentValue::array_default());

        let array_custom_entry = config_map.get("array_custom").unwrap();
        assert_eq!(array_custom_entry.type_.as_deref(), Some("array"));
        assert_eq!(array_custom_entry.value, custom_array_value);

        let object_entry = config_map.get("object_value").unwrap();
        assert_eq!(object_entry.type_.as_deref(), Some("object"));
        assert_eq!(object_entry.value, AgentValue::object_default());

        let object_custom_entry = config_map.get("object_custom").unwrap();
        assert_eq!(object_custom_entry.type_.as_deref(), Some("object"));
        assert_eq!(object_custom_entry.value, custom_object_value);
    }

    #[test]
    fn test_global_config_helpers() {
        let custom_object_value =
            AgentValue::object(hashmap! {"key".into() => AgentValue::string("value")});
        let custom_array_value =
            AgentValue::array(vector![AgentValue::integer(1), AgentValue::string("two")]);

        let def = AgentDefinition::new("test", "helpers", None)
            .boolean_global_config("global_boolean", true)
            .integer_global_config("global_integer", 42)
            .number_global_config("global_number", 1.5)
            .string_global_config("global_string", "value")
            .text_global_config("global_text", "global")
            .array_global_config_default("global_array")
            .array_global_config("global_array_custom", custom_array_value.clone())
            .object_global_config("global_object", custom_object_value.clone());

        let global_configs = def.global_configs.expect("global configs should exist");
        assert_eq!(global_configs.len(), 8);
        let config_map: std::collections::HashMap<_, _> = global_configs.into_iter().collect();

        let entry = config_map.get("global_boolean").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("boolean"));
        assert_eq!(entry.value, AgentValue::boolean(true));

        let entry = config_map.get("global_integer").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("integer"));
        assert_eq!(entry.value, AgentValue::integer(42));

        let entry = config_map.get("global_number").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("number"));
        assert_eq!(entry.value, AgentValue::number(1.5));

        let entry = config_map.get("global_string").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("string"));
        assert_eq!(entry.value, AgentValue::string("value"));

        let entry = config_map.get("global_text").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("text"));
        assert_eq!(entry.value, AgentValue::string("global"));

        let entry = config_map.get("global_array").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("array"));
        assert_eq!(entry.value, AgentValue::array_default());

        let entry = config_map.get("global_array_custom").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("array"));
        assert_eq!(entry.value, custom_array_value);

        let entry = config_map.get("global_object").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("object"));
        assert_eq!(entry.value, custom_object_value);
    }

    #[test]
    fn test_config_helper_customization() {
        let def = AgentDefinition::new("test", "custom", None)
            .integer_config_with("custom_default", 1, |entry| entry.title("Custom"))
            .text_global_config_with("custom_global", "value", |entry| {
                entry.description("Global Desc")
            });
        // .text_display_config_with("custom_display", |entry| entry.title("Display"));

        let default_entry = def.configs.as_ref().unwrap().get("custom_default").unwrap();
        assert_eq!(default_entry.title.as_deref(), Some("Custom"));

        let global_entry = def
            .global_configs
            .as_ref()
            .unwrap()
            .get("custom_global")
            .unwrap();
        assert_eq!(global_entry.description.as_deref(), Some("Global Desc"));
    }

    fn echo_agent_definition() -> AgentDefinition {
        AgentDefinition::new(
            "test",
            "echo",
            Some(|_app, _id, _spec| Err(AgentError::NotImplemented("Echo agent".into()))),
        )
        .title("Echo")
        .category("Test")
        .inputs(vec!["in"])
        .outputs(vec!["out"])
        .string_config_with("value", "abc", |entry| {
            entry
                .title("display_title")
                .description("display_description")
                .readonly()
                .detail()
        })
        .integer_config_with("hide_title_value", 1, |entry| entry.hide_title().readonly())
    }
}
