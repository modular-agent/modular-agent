use std::ops::Not;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::FnvIndexMap;
use crate::config::ModuleConfigs;
use crate::error::Result;
use crate::id::new_id;
use crate::modular_agent::ModularAgent;
use crate::module::Module;
use crate::spec::ModuleSpec;
use crate::value::Value;

/// A map of module definition names to their definitions.
pub type ModuleDefinitions = FnvIndexMap<String, ModuleDefinition>;

/// The definition (blueprint) of a module type.
///
/// A module definition describes the metadata and capabilities of a module type,
/// including its ports, configuration options, and factory function.
/// Multiple module instances can be created from a single definition.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ModuleDefinition {
    /// The kind/category identifier for this module type (e.g., "Module", "Board").
    pub kind: String,

    /// Unique name of this module definition.
    pub name: String,

    /// Human-readable title for display in UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Whether to hide the title in UI.
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub hide_title: bool,

    /// Description of what this module does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Category path for organizing modules (e.g., "Flow/Control").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Default input port names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<String>>,

    /// Default output port names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<String>>,

    /// Configuration specifications for this module type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configs: Option<ModuleConfigSpecs>,

    /// Global configuration specifications (shared across instances).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_configs: Option<ModuleGlobalConfigSpecs>,

    /// Hint metadata for UI presentation (e.g., color, size).
    #[serde(default, skip_serializing_if = "FnvIndexMap::is_empty")]
    pub hints: FnvIndexMap<String, JsonValue>,

    /// Factory function to create new module instances.
    #[serde(skip)]
    pub new_boxed: Option<ModuleNewBoxedFn>,
}

/// A map of configuration keys to their specifications.
pub type ModuleConfigSpecs = FnvIndexMap<String, ModuleConfigSpec>;

/// A map of global configuration keys to their specifications.
pub type ModuleGlobalConfigSpecs = FnvIndexMap<String, ModuleConfigSpec>;

/// Specification for a configuration entry.
///
/// Defines the metadata for a configuration option, including its default value,
/// type, display properties, and access control.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ModuleConfigSpec {
    /// Default value for this configuration.
    pub value: Value,

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

/// Factory function type for creating new module instances.
///
/// Takes a `ModularAgent` orchestrator, module ID, and spec, and returns
/// a boxed `Module` trait object or an error.
pub type ModuleNewBoxedFn =
    fn(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Box<dyn Module>>;

impl ModuleDefinition {
    /// Creates a new module definition.
    ///
    /// # Arguments
    ///
    /// * `kind` - The kind/category identifier (e.g., "std", "llm")
    /// * `name` - Unique name for this module definition
    /// * `new_boxed` - Optional factory function to create module instances
    pub fn new(
        kind: impl Into<String>,
        name: impl Into<String>,
        new_boxed: Option<ModuleNewBoxedFn>,
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
    pub fn configs(mut self, configs: Vec<(&str, ModuleConfigSpec)>) -> Self {
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
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
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
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
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
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
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
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
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
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        let default = default.into();
        self.config_type_with(key, Value::string(default), "string", f)
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
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        let default = default.into();
        self.config_type_with(key, Value::string(default), "text", f)
    }

    /// Adds a text configuration with empty default value.
    pub fn text_config_default(self, key: &str) -> Self {
        self.text_config(key, "")
    }

    /// Adds an array configuration with a default value.
    pub fn array_config(self, key: &str, default: impl Into<Value>) -> Self {
        self.array_config_with(key, default, |entry| entry)
    }

    /// Adds an array configuration with customization callback.
    pub fn array_config_with<V: Into<Value>, F>(self, key: &str, default: V, f: F) -> Self
    where
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        self.config_type_with(key, default, "array", f)
    }

    /// Adds an array configuration with empty default value.
    pub fn array_config_default(self, key: &str) -> Self {
        self.array_config(key, Value::array_default())
    }

    /// Adds an object configuration with a default value.
    pub fn object_config<V: Into<Value>>(self, key: &str, default: V) -> Self {
        self.object_config_with(key, default, |entry| entry)
    }

    /// Adds an object configuration with customization callback.
    pub fn object_config_with<V: Into<Value>, F>(self, key: &str, default: V, f: F) -> Self
    where
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        self.config_type_with(key, default, "object", f)
    }

    /// Adds an object configuration with empty default value.
    pub fn object_config_default(self, key: &str) -> Self {
        self.object_config(key, Value::object_default())
    }

    /// Adds a custom-typed configuration with customization callback.
    pub fn custom_config_with<V: Into<Value>, F>(
        self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        self.config_type_with(key, default, type_, f)
    }

    /// Internal: adds a configuration with specified type.
    fn config_type_with<V: Into<Value>, F>(
        mut self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        let entry = ModuleConfigSpec::new(default, type_);
        self.insert_config_entry(key.into(), f(entry));
        self
    }

    fn insert_config_entry(&mut self, key: String, entry: ModuleConfigSpec) {
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
    // Global configurations are shared across all instances of this module type.

    /// Sets all global configuration specifications at once.
    pub fn global_configs(mut self, configs: Vec<(&str, ModuleConfigSpec)>) -> Self {
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
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
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
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
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
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
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
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        let default = default.into();
        self.global_config_type_with(key, Value::string(default), "string", f)
    }

    /// Adds a multiline text global configuration.
    pub fn text_global_config(self, key: &str, default: impl Into<String>) -> Self {
        self.text_global_config_with(key, default, |entry| entry)
    }

    /// Adds a text global configuration with customization callback.
    pub fn text_global_config_with<F>(self, key: &str, default: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        let default = default.into();
        self.global_config_type_with(key, Value::string(default), "text", f)
    }

    /// Adds an array global configuration.
    pub fn array_global_config(self, key: &str, default: impl Into<Value>) -> Self {
        self.array_global_config_with(key, default, |entry| entry)
    }

    /// Adds an array global configuration with customization callback.
    pub fn array_global_config_with<V: Into<Value>, F>(self, key: &str, default: V, f: F) -> Self
    where
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        self.global_config_type_with(key, default, "array", f)
    }

    /// Adds an array global configuration with empty default value.
    pub fn array_global_config_default(self, key: &str) -> Self {
        self.array_global_config(key, Value::array_default())
    }

    /// Adds an object global configuration.
    pub fn object_global_config<V: Into<Value>>(self, key: &str, default: V) -> Self {
        self.object_global_config_with(key, default, |entry| entry)
    }

    /// Adds an object global configuration with customization callback.
    pub fn object_global_config_with<V: Into<Value>, F>(self, key: &str, default: V, f: F) -> Self
    where
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        self.global_config_type_with(key, default, "object", f)
    }

    /// Adds a custom-typed global configuration with customization callback.
    pub fn custom_global_config_with<V: Into<Value>, F>(
        self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        self.global_config_type_with(key, default, type_, f)
    }

    fn global_config_type_with<V: Into<Value>, F>(
        mut self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(ModuleConfigSpec) -> ModuleConfigSpec,
    {
        let entry = ModuleConfigSpec::new(default, type_);
        self.insert_global_config_entry(key.into(), f(entry));
        self
    }

    fn insert_global_config_entry(&mut self, key: String, entry: ModuleConfigSpec) {
        if let Some(configs) = self.global_configs.as_mut() {
            configs.insert(key, entry);
        } else {
            let mut map = FnvIndexMap::default();
            map.insert(key, entry);
            self.global_configs = Some(map);
        }
    }

    /// Adds a UI hint. Returns self for method chaining.
    pub fn hint(mut self, key: &str, value: impl Into<JsonValue>) -> Self {
        self.hints.insert(key.into(), value.into());
        self
    }

    /// Creates a new module specification from this definition.
    ///
    /// Generates a unique ID and copies the definition's ports and configs
    /// to create a new instance specification.
    pub fn to_spec(&self) -> ModuleSpec {
        ModuleSpec {
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
            disabled: false,
            extensions: FnvIndexMap::default(),
        }
    }

    /// Reconciles an existing `ModuleSpec` with this definition for backward compatibility.
    ///
    /// When loading old JSON patches, the spec may not match the current definition.
    /// This method:
    /// - Fills missing config keys with definition defaults
    /// - Renames stale keys (not in definition) with `_` prefix for lazy migration
    /// - Overwrites `config_specs` with current definition metadata
    /// - Overwrites ports with current definition ports
    ///
    /// Keys already starting with `_` are skipped during rename (idempotency).
    /// `_`-prefixed keys are cleaned up by `ModuleData::new()`.
    ///
    /// Config names must not start with `_` (reserved for stale key migration).
    pub fn reconcile_spec(&self, spec: &mut ModuleSpec) {
        // Ports
        if let Some(ref inputs) = self.inputs {
            spec.inputs = Some(inputs.clone());
        }
        if let Some(ref outputs) = self.outputs {
            spec.outputs = Some(outputs.clone());
        }

        // config_specs
        spec.config_specs = self.configs.clone();

        // Configs
        let def_keys: Option<std::collections::HashSet<&str>> = self
            .configs
            .as_ref()
            .map(|c| c.keys().map(|k| k.as_str()).collect());

        if let Some(ref mut spec_configs) = spec.configs {
            // Rename stale keys with `_` prefix (skip already-prefixed for idempotency)
            let stale: Vec<String> = spec_configs
                .keys()
                .filter(|k| {
                    !k.starts_with('_')
                        && !def_keys.as_ref().is_some_and(|dk| dk.contains(k.as_str()))
                })
                .cloned()
                .collect();
            for key in stale {
                if let Some(value) = spec_configs.remove(&key) {
                    spec_configs.set(format!("_{key}"), value);
                }
            }

            // Fill missing keys with definition defaults
            if let Some(ref def_configs) = self.configs {
                for (key, cs) in def_configs.iter() {
                    if !spec_configs.contains_key(key) {
                        spec_configs.set(key.clone(), cs.value.clone());
                    }
                }
            }
        } else if let Some(ref def_configs) = self.configs {
            // spec.configs is None → create from definition defaults
            spec.configs = Some(
                def_configs
                    .iter()
                    .map(|(k, v)| (k.clone(), v.value.clone()))
                    .collect(),
            );
        }

        // Reorder configs to match definition key order
        if let Some(ref mut spec_configs) = spec.configs
            && let Some(ref def_configs) = self.configs
        {
            let mut reordered = ModuleConfigs::new();
            // First: definition keys in definition order
            for (key, _) in def_configs.iter() {
                if let Ok(value) = spec_configs.get(key) {
                    reordered.set(key.clone(), value.clone());
                }
            }
            // Then: remaining keys (stale `_`-prefixed, etc.)
            for (key, value) in &*spec_configs {
                if !reordered.contains_key(key) {
                    reordered.set(key.clone(), value.clone());
                }
            }
            *spec_configs = reordered;
        }
    }
}

impl ModuleConfigSpec {
    /// Creates a new configuration specification.
    ///
    /// # Arguments
    ///
    /// * `value` - Default value for this configuration
    /// * `type_` - Type identifier (e.g., "string", "integer", "boolean")
    pub fn new<V: Into<Value>>(value: V, type_: &str) -> Self {
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
    use crate::error::Error;
    use im::{hashmap, vector};

    use super::*;
    use crate::config::ModuleConfigs;

    #[test]
    fn test_module_definition() {
        let def = ModuleDefinition::default();
        assert_eq!(def.name, "");
    }

    #[test]
    fn test_module_definition_new_default() {
        let def = ModuleDefinition::new(
            "test",
            "echo",
            Some(|_app, _id, _spec| Err(Error::NotImplemented("Echo module".into()))),
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
    fn test_module_definition_new() {
        let def = echo_module_definition();

        assert_eq!(def.kind, "test");
        assert_eq!(def.name, "echo");
        assert_eq!(def.title.unwrap(), "Echo");
        assert_eq!(def.category.unwrap(), "Test");
        assert_eq!(def.inputs.unwrap(), vec!["in"]);
        assert_eq!(def.outputs.unwrap(), vec!["out"]);
        let default_configs = def.configs.unwrap();
        assert_eq!(default_configs.len(), 2);
        let entry = default_configs.get("value").unwrap();
        assert_eq!(entry.value, Value::string("abc"));
        assert_eq!(entry.type_.as_ref().unwrap(), "string");
        assert_eq!(entry.title.as_ref().unwrap(), "display_title");
        assert_eq!(entry.description.as_ref().unwrap(), "display_description");
        assert!(!entry.hide_title);
        assert!(entry.readonly);
        assert!(entry.detail);
        let entry = default_configs.get("hide_title_value").unwrap();
        assert_eq!(entry.value, Value::integer(1));
        assert_eq!(entry.type_.as_ref().unwrap(), "integer");
        assert_eq!(entry.title, None);
        assert_eq!(entry.description, None);
        assert!(entry.hide_title);
        assert!(entry.readonly);
        assert!(!entry.detail);
    }

    #[test]
    fn test_serialize_module_definition() {
        let def = ModuleDefinition::new(
            "test",
            "echo",
            Some(|_app, _id, _spec| Err(Error::NotImplemented("Echo module".into()))),
        );
        let json = serde_json::to_string(&def).unwrap();
        assert_eq!(json, r#"{"kind":"test","name":"echo"}"#);
    }

    #[test]
    fn test_serialize_echo_module_definition() {
        let def = echo_module_definition();
        let json = serde_json::to_string(&def).unwrap();
        print!("{}", json);
        assert_eq!(
            json,
            r#"{"kind":"test","name":"echo","title":"Echo","category":"Test","inputs":["in"],"outputs":["out"],"configs":{"value":{"value":"abc","type":"string","title":"display_title","description":"display_description","readonly":true,"detail":true},"hide_title_value":{"value":1,"type":"integer","hide_title":true,"readonly":true}}}"#
        );
    }

    #[test]
    fn test_deserialize_echo_module_definition() {
        let json = r#"{"kind":"test","name":"echo","title":"Echo","category":"Test","inputs":["in"],"outputs":["out"],"configs":{"value":{"value":"abc","type":"string","title":"display_title","description":"display_description","readonly":true,"detail":true},"hide_title_value":{"value":1,"type":"integer","hide_title":true,"readonly":true}}}"#;
        let def: ModuleDefinition = serde_json::from_str(json).unwrap();
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
        assert!(!entry.hide_title);
        assert!(entry.detail);
        let (key, entry) = default_configs.get_index(1).unwrap();
        assert_eq!(key, "hide_title_value");
        assert_eq!(entry.type_.as_ref().unwrap(), "integer");
        assert_eq!(entry.title, None);
        assert_eq!(entry.description, None);
        assert!(entry.hide_title);
    }

    #[test]
    fn test_default_config_helpers() {
        let custom_object_value = Value::object(hashmap! {"key".into() => Value::string("value")});
        let custom_array_value = Value::array(vector![Value::integer(1), Value::string("two")]);

        let def = ModuleDefinition::new("test", "helpers", None)
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
        assert_eq!(unit_entry.value, Value::unit());

        let boolean_entry = config_map.get("boolean_value").unwrap();
        assert_eq!(boolean_entry.type_.as_deref(), Some("boolean"));
        assert_eq!(boolean_entry.value, Value::boolean(false));

        let boolean_custom_entry = config_map.get("boolean_custom").unwrap();
        assert_eq!(boolean_custom_entry.type_.as_deref(), Some("boolean"));
        assert_eq!(boolean_custom_entry.value, Value::boolean(true));

        let integer_entry = config_map.get("integer_value").unwrap();
        assert_eq!(integer_entry.type_.as_deref(), Some("integer"));
        assert_eq!(integer_entry.value, Value::integer(0));

        let integer_custom_entry = config_map.get("integer_custom").unwrap();
        assert_eq!(integer_custom_entry.type_.as_deref(), Some("integer"));
        assert_eq!(integer_custom_entry.value, Value::integer(42));

        let number_entry = config_map.get("number_value").unwrap();
        assert_eq!(number_entry.type_.as_deref(), Some("number"));
        assert_eq!(number_entry.value, Value::number(0.0));

        let number_custom_entry = config_map.get("number_custom").unwrap();
        assert_eq!(number_custom_entry.type_.as_deref(), Some("number"));
        assert_eq!(number_custom_entry.value, Value::number(1.5));

        let string_default_entry = config_map.get("string_default").unwrap();
        assert_eq!(string_default_entry.type_.as_deref(), Some("string"));
        assert_eq!(string_default_entry.value, Value::string(""));

        let string_entry = config_map.get("string_value").unwrap();
        assert_eq!(string_entry.type_.as_deref(), Some("string"));
        assert_eq!(string_entry.value, Value::string("value"));

        let text_entry = config_map.get("text_value").unwrap();
        assert_eq!(text_entry.type_.as_deref(), Some("text"));
        assert_eq!(text_entry.value, Value::string(""));

        let text_custom_entry = config_map.get("text_custom").unwrap();
        assert_eq!(text_custom_entry.type_.as_deref(), Some("text"));
        assert_eq!(text_custom_entry.value, Value::string("custom"));

        let array_entry = config_map.get("array_value").unwrap();
        assert_eq!(array_entry.type_.as_deref(), Some("array"));
        assert_eq!(array_entry.value, Value::array_default());

        let array_custom_entry = config_map.get("array_custom").unwrap();
        assert_eq!(array_custom_entry.type_.as_deref(), Some("array"));
        assert_eq!(array_custom_entry.value, custom_array_value);

        let object_entry = config_map.get("object_value").unwrap();
        assert_eq!(object_entry.type_.as_deref(), Some("object"));
        assert_eq!(object_entry.value, Value::object_default());

        let object_custom_entry = config_map.get("object_custom").unwrap();
        assert_eq!(object_custom_entry.type_.as_deref(), Some("object"));
        assert_eq!(object_custom_entry.value, custom_object_value);
    }

    #[test]
    fn test_global_config_helpers() {
        let custom_object_value = Value::object(hashmap! {"key".into() => Value::string("value")});
        let custom_array_value = Value::array(vector![Value::integer(1), Value::string("two")]);

        let def = ModuleDefinition::new("test", "helpers", None)
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
        assert_eq!(entry.value, Value::boolean(true));

        let entry = config_map.get("global_integer").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("integer"));
        assert_eq!(entry.value, Value::integer(42));

        let entry = config_map.get("global_number").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("number"));
        assert_eq!(entry.value, Value::number(1.5));

        let entry = config_map.get("global_string").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("string"));
        assert_eq!(entry.value, Value::string("value"));

        let entry = config_map.get("global_text").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("text"));
        assert_eq!(entry.value, Value::string("global"));

        let entry = config_map.get("global_array").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("array"));
        assert_eq!(entry.value, Value::array_default());

        let entry = config_map.get("global_array_custom").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("array"));
        assert_eq!(entry.value, custom_array_value);

        let entry = config_map.get("global_object").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("object"));
        assert_eq!(entry.value, custom_object_value);
    }

    #[test]
    fn test_config_helper_customization() {
        let def = ModuleDefinition::new("test", "custom", None)
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

    fn echo_module_definition() -> ModuleDefinition {
        ModuleDefinition::new(
            "test",
            "echo",
            Some(|_app, _id, _spec| Err(Error::NotImplemented("Echo module".into()))),
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

    // --- reconcile_spec tests ---

    fn reconcile_def() -> ModuleDefinition {
        ModuleDefinition::new("test", "reconcile", None)
            .inputs(vec!["in1", "in2"])
            .outputs(vec!["out"])
            .string_config("name", "default_name")
            .integer_config("count", 10)
            .boolean_config("enabled", true)
    }

    #[test]
    fn test_reconcile_fills_missing_configs() {
        let def = reconcile_def();
        let mut configs = ModuleConfigs::new();
        configs.set("name".into(), Value::string("hello"));
        let mut spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        let c = spec.configs.as_ref().unwrap();
        assert_eq!(c.get_string_or_default("name"), "hello");
        assert_eq!(c.get_integer_or_default("count"), 10);
        assert!(c.get_bool_or_default("enabled"));
    }

    #[test]
    fn test_reconcile_renames_stale_keys() {
        let def = ModuleDefinition::new("test", "r", None).string_config("name", "default");
        let mut configs = ModuleConfigs::new();
        configs.set("name".into(), Value::string("hello"));
        configs.set("old_key".into(), Value::string("stale_val"));
        configs.set("removed".into(), Value::integer(42));
        let mut spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        let c = spec.configs.as_ref().unwrap();
        assert_eq!(c.get_string_or_default("name"), "hello");
        assert!(c.get("old_key").is_err());
        assert_eq!(c.get("_old_key").unwrap(), &Value::string("stale_val"));
        assert!(c.get("removed").is_err());
        assert_eq!(c.get("_removed").unwrap(), &Value::integer(42));
    }

    #[test]
    fn test_reconcile_skips_already_prefixed() {
        let def = ModuleDefinition::new("test", "r", None).string_config("name", "default");
        let mut configs = ModuleConfigs::new();
        configs.set("name".into(), Value::string("hello"));
        configs.set("_old".into(), Value::string("from_prev_reconcile"));
        let mut spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        let c = spec.configs.as_ref().unwrap();
        assert_eq!(
            c.get("_old").unwrap(),
            &Value::string("from_prev_reconcile")
        );
        assert!(c.get("__old").is_err());
    }

    #[test]
    fn test_reconcile_overwrites_config_specs() {
        let def = reconcile_def();
        let mut spec = ModuleSpec {
            config_specs: Some(FnvIndexMap::default()),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        let specs = spec.config_specs.as_ref().unwrap();
        assert!(specs.contains_key("name"));
        assert!(specs.contains_key("count"));
        assert!(specs.contains_key("enabled"));
        assert_eq!(specs.len(), 3);
    }

    #[test]
    fn test_reconcile_overwrites_ports() {
        let def = reconcile_def();
        let mut spec = ModuleSpec {
            inputs: Some(vec!["old_in".into()]),
            outputs: Some(vec!["old_out".into()]),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        assert_eq!(
            spec.inputs.as_ref().unwrap(),
            &vec!["in1".to_string(), "in2".to_string()]
        );
        assert_eq!(spec.outputs.as_ref().unwrap(), &vec!["out".to_string()]);
    }

    #[test]
    fn test_reconcile_preserves_ports_when_def_none() {
        let def = ModuleDefinition::new("test", "r", None);
        let mut spec = ModuleSpec {
            inputs: Some(vec!["custom_in".into()]),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        assert_eq!(
            spec.inputs.as_ref().unwrap(),
            &vec!["custom_in".to_string()]
        );
    }

    #[test]
    fn test_reconcile_configs_none_creates_defaults() {
        let def = reconcile_def();
        let mut spec = ModuleSpec::default();
        assert!(spec.configs.is_none());

        def.reconcile_spec(&mut spec);

        let c = spec.configs.as_ref().unwrap();
        assert_eq!(c.get_string_or_default("name"), "default_name");
        assert_eq!(c.get_integer_or_default("count"), 10);
        assert!(c.get_bool_or_default("enabled"));
        // Key order matches definition order
        let keys: Vec<&String> = c.keys().collect();
        assert_eq!(keys, vec!["name", "count", "enabled"]);
    }

    #[test]
    fn test_reconcile_def_configs_none_marks_all_stale() {
        let def = ModuleDefinition::new("test", "r", None);
        let mut configs = ModuleConfigs::new();
        configs.set("old_a".into(), Value::string("a"));
        configs.set("old_b".into(), Value::integer(1));
        let mut spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        let c = spec.configs.as_ref().unwrap();
        assert!(c.get("old_a").is_err());
        assert!(c.get("old_b").is_err());
        assert_eq!(c.get("_old_a").unwrap(), &Value::string("a"));
        assert_eq!(c.get("_old_b").unwrap(), &Value::integer(1));
    }

    #[test]
    fn test_reconcile_preserves_user_values() {
        let def = reconcile_def();
        let mut configs = ModuleConfigs::new();
        configs.set("name".into(), Value::string("custom"));
        configs.set("count".into(), Value::integer(42));
        configs.set("enabled".into(), Value::boolean(false));
        let mut spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        let c = spec.configs.as_ref().unwrap();
        assert_eq!(c.get_string_or_default("name"), "custom");
        assert_eq!(c.get_integer_or_default("count"), 42);
        assert!(!c.get_bool_or_default("enabled"));
    }

    #[test]
    fn test_reconcile_idempotent() {
        let def = reconcile_def();
        let mut configs = ModuleConfigs::new();
        configs.set("name".into(), Value::string("hello"));
        configs.set("old".into(), Value::string("stale"));
        let mut spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);
        let first = spec.clone();

        def.reconcile_spec(&mut spec);

        let c1 = first.configs.as_ref().unwrap();
        let c2 = spec.configs.as_ref().unwrap();
        assert_eq!(
            c1.get_string_or_default("name"),
            c2.get_string_or_default("name")
        );
        assert_eq!(
            c1.get_integer_or_default("count"),
            c2.get_integer_or_default("count")
        );
        assert_eq!(c1.get("_old").unwrap(), c2.get("_old").unwrap());
    }

    #[test]
    fn test_reconcile_to_spec_is_noop() {
        let def = reconcile_def();
        let mut spec = def.to_spec();
        let original = spec.clone();

        def.reconcile_spec(&mut spec);

        let c1 = spec.configs.as_ref().unwrap();
        let c2 = original.configs.as_ref().unwrap();
        assert_eq!(
            c1.get_string_or_default("name"),
            c2.get_string_or_default("name")
        );
        assert_eq!(
            c1.get_integer_or_default("count"),
            c2.get_integer_or_default("count")
        );
        assert_eq!(
            c1.get_bool_or_default("enabled"),
            c2.get_bool_or_default("enabled")
        );
        assert_eq!(spec.inputs, original.inputs);
        assert_eq!(spec.outputs, original.outputs);
    }

    #[test]
    fn test_reconcile_empty_configs() {
        let def = reconcile_def();
        let mut spec = ModuleSpec {
            configs: Some(ModuleConfigs::new()),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        let c = spec.configs.as_ref().unwrap();
        assert_eq!(c.get_string_or_default("name"), "default_name");
        assert_eq!(c.get_integer_or_default("count"), 10);
        assert!(c.get_bool_or_default("enabled"));
    }

    #[test]
    fn test_reconcile_mixed_stale_and_prefixed() {
        let def = ModuleDefinition::new("test", "r", None).string_config("name", "default");
        let mut configs = ModuleConfigs::new();
        configs.set("name".into(), Value::string("hello"));
        configs.set("_prev_stale".into(), Value::string("from_prev"));
        configs.set("removed".into(), Value::integer(99));
        let mut spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        let c = spec.configs.as_ref().unwrap();
        assert_eq!(c.get_string_or_default("name"), "hello");
        // _prev_stale is kept as-is (already prefixed)
        assert_eq!(c.get("_prev_stale").unwrap(), &Value::string("from_prev"));
        assert!(c.get("__prev_stale").is_err());
        // removed is newly prefixed
        assert!(c.get("removed").is_err());
        assert_eq!(c.get("_removed").unwrap(), &Value::integer(99));
    }

    #[test]
    fn test_reconcile_reorders_configs_to_definition_order() {
        let def = reconcile_def(); // defines: name, count, enabled
        let mut configs = ModuleConfigs::new();
        // Insert in reverse order
        configs.set("enabled".into(), Value::boolean(false));
        configs.set("count".into(), Value::integer(42));
        configs.set("name".into(), Value::string("custom"));
        let mut spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        let c = spec.configs.as_ref().unwrap();
        let keys: Vec<&String> = c.keys().collect();
        assert_eq!(keys, vec!["name", "count", "enabled"]);
        // Values are preserved
        assert_eq!(c.get_string_or_default("name"), "custom");
        assert_eq!(c.get_integer_or_default("count"), 42);
        assert!(!c.get_bool_or_default("enabled"));
    }

    #[test]
    fn test_reconcile_reorder_stale_keys_at_end() {
        let def = reconcile_def(); // defines: name, count, enabled
        let mut configs = ModuleConfigs::new();
        configs.set("old_key".into(), Value::string("stale"));
        configs.set("enabled".into(), Value::boolean(true));
        configs.set("name".into(), Value::string("hello"));
        let mut spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);

        let c = spec.configs.as_ref().unwrap();
        let keys: Vec<&String> = c.keys().collect();
        // Definition keys first in definition order, then stale keys at end
        assert_eq!(keys, vec!["name", "count", "enabled", "_old_key"]);
    }

    #[test]
    fn test_reconcile_reorder_is_idempotent() {
        let def = reconcile_def();
        let mut configs = ModuleConfigs::new();
        configs.set("enabled".into(), Value::boolean(false));
        configs.set("name".into(), Value::string("hello"));
        configs.set("old".into(), Value::string("stale"));
        let mut spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        def.reconcile_spec(&mut spec);
        let order_first: Vec<String> = spec.configs.as_ref().unwrap().keys().cloned().collect();

        def.reconcile_spec(&mut spec);
        let order_second: Vec<String> = spec.configs.as_ref().unwrap().keys().cloned().collect();

        assert_eq!(order_first, order_second);
    }

    // --- hints tests ---

    #[test]
    fn test_hint_builder() {
        let def = ModuleDefinition::new("test", "hinted", None)
            .hint("color", 3)
            .hint("width", 2)
            .hint("height", 1);
        assert_eq!(def.hints.len(), 3);
        assert_eq!(def.hints["color"], serde_json::json!(3));
        assert_eq!(def.hints["width"], serde_json::json!(2));
        assert_eq!(def.hints["height"], serde_json::json!(1));
    }

    #[test]
    fn test_hint_string_value() {
        let def = ModuleDefinition::new("test", "hinted", None).hint("label", "red");
        assert_eq!(def.hints["label"], serde_json::json!("red"));
    }

    #[test]
    fn test_hint_boolean_value() {
        let def = ModuleDefinition::new("test", "hinted", None).hint("resizable", true);
        assert_eq!(def.hints["resizable"], serde_json::json!(true));
    }

    #[test]
    fn test_no_hints_serialization() {
        let def = ModuleDefinition::new("test", "empty", None);
        let json = serde_json::to_string(&def).unwrap();
        assert!(!json.contains("hints"));
    }

    #[test]
    fn test_hints_serialization_roundtrip() {
        let def = ModuleDefinition::new("test", "hinted", None)
            .hint("color", 3)
            .hint("width", 2);
        let json = serde_json::to_string(&def).unwrap();
        assert!(json.contains(r#""hints""#));
        let parsed: ModuleDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.hints.len(), 2);
        assert_eq!(parsed.hints["color"], serde_json::json!(3));
        assert_eq!(parsed.hints["width"], serde_json::json!(2));
    }

    #[test]
    fn test_hints_deserialization_missing_field() {
        let json = r#"{"kind":"test","name":"no_hints"}"#;
        let def: ModuleDefinition = serde_json::from_str(json).unwrap();
        assert!(def.hints.is_empty());
    }
}
