use std::collections::HashMap;
use std::ops::Not;

use serde::{Deserialize, Serialize};

use super::agent::Agent;
use super::askit::ASKit;
use super::config::AgentConfigs;
use super::data::AgentValue;
use super::error::AgentError;

pub type AgentDefinitions = HashMap<String, AgentDefinition>;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentDefinition {
    pub kind: String,

    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_configs: Option<AgentDefaultConfigs>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_configs: Option<AgentGlobalConfigs>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_configs: Option<AgentDisplayConfigs>,

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub native_thread: bool,

    #[serde(skip)]
    pub new_boxed: Option<AgentNewBoxedFn>,
}

pub type AgentDefaultConfigs = Vec<(String, AgentConfigEntry)>;
pub type AgentGlobalConfigs = Vec<(String, AgentConfigEntry)>;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentConfigEntry {
    pub value: AgentValue,

    #[serde(rename = "type")]
    pub type_: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Indicates whether this configuration entry should be hidden from the user interface.
    /// If set to `true`, the entry will be hidden. The default behavior is to show the entry.
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub hidden: bool,
}

pub type AgentDisplayConfigs = Vec<(String, AgentDisplayConfigEntry)>;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentDisplayConfigEntry {
    #[serde(rename = "type")]
    pub type_: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub hide_title: bool,
}

// #[derive(Debug, Default, Serialize, Deserialize, Clone)]
// pub struct CommandConfig {
//     pub cmd: String,
//     pub args: Option<Vec<String>>,

//     pub dir: Option<String>,
// }

pub type AgentNewBoxedFn = fn(
    askit: ASKit,
    id: String,
    def_name: String,
    configs: Option<AgentConfigs>,
) -> Result<Box<dyn Agent + Send + Sync>, AgentError>;

impl AgentDefinition {
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

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_category(mut self, category: &str) -> Self {
        self.category = Some(category.into());
        self
    }

    pub fn with_inputs(mut self, inputs: Vec<&str>) -> Self {
        self.inputs = Some(inputs.into_iter().map(|x| x.into()).collect());
        self
    }

    pub fn with_outputs(mut self, outputs: Vec<&str>) -> Self {
        self.outputs = Some(outputs.into_iter().map(|x| x.into()).collect());
        self
    }

    // Default Configs

    pub fn with_default_configs(mut self, configs: Vec<(&str, AgentConfigEntry)>) -> Self {
        self.default_configs = Some(configs.into_iter().map(|(k, v)| (k.into(), v)).collect());
        self
    }

    pub fn with_unit_config(self, key: &str) -> Self {
        self.with_unit_config_with(key, |entry| entry)
    }

    pub fn with_unit_config_with<F>(self, key: &str, f: F) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_config_type_with(key, (), "unit", f)
    }

    pub fn with_boolean_config(self, key: &str, default: bool) -> Self {
        self.with_boolean_config_with(key, default, |entry| entry)
    }

    pub fn with_boolean_config_with<F>(self, key: &str, default: bool, f: F) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_config_type_with(key, default, "boolean", f)
    }

    pub fn with_boolean_config_default(self, key: &str) -> Self {
        self.with_boolean_config(key, false)
    }

    pub fn with_integer_config(self, key: &str, default: i64) -> Self {
        self.with_integer_config_with(key, default, |entry| entry)
    }

    pub fn with_integer_config_with<F>(self, key: &str, default: i64, f: F) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_config_type_with(key, default, "integer", f)
    }

    pub fn with_integer_config_default(self, key: &str) -> Self {
        self.with_integer_config(key, 0)
    }

    pub fn with_number_config(self, key: &str, default: f64) -> Self {
        self.with_number_config_with(key, default, |entry| entry)
    }

    pub fn with_number_config_with<F>(self, key: &str, default: f64, f: F) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_config_type_with(key, default, "number", f)
    }

    pub fn with_number_config_default(self, key: &str) -> Self {
        self.with_number_config(key, 0.0)
    }

    pub fn with_string_config(self, key: &str, default: impl Into<String>) -> Self {
        self.with_string_config_with(key, default, |entry| entry)
    }

    pub fn with_string_config_with<F>(
        self,
        key: &str,
        default: impl Into<String>,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        let default = default.into();
        self.with_config_type_with(key, AgentValue::string(default), "string", f)
    }

    pub fn with_string_config_default(self, key: &str) -> Self {
        self.with_string_config(key, "")
    }

    pub fn with_text_config(self, key: &str, default: impl Into<String>) -> Self {
        self.with_text_config_with(key, default, |entry| entry)
    }

    pub fn with_text_config_with<F>(
        self,
        key: &str,
        default: impl Into<String>,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        let default = default.into();
        self.with_config_type_with(key, AgentValue::string(default), "text", f)
    }

    pub fn with_text_config_default(self, key: &str) -> Self {
        self.with_text_config(key, "")
    }

    pub fn with_object_config<V: Into<AgentValue>>(self, key: &str, default: V) -> Self {
        self.with_object_config_with(key, default, |entry| entry)
    }

    pub fn with_object_config_with<V: Into<AgentValue>, F>(
        self,
        key: &str,
        default: V,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_config_type_with(key, default, "object", f)
    }

    pub fn with_object_config_default(self, key: &str) -> Self {
        self.with_object_config(key, AgentValue::object_default())
    }

    pub fn with_custom_config_with<V: Into<AgentValue>, F>(
        self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_config_type_with(key, default, type_, f)
    }

    fn with_config_type<V: Into<AgentValue>>(self, key: &str, default: V, type_: &str) -> Self {
        self.with_config_type_with(key, default, type_, |entry| entry)
    }

    fn with_config_type_with<V: Into<AgentValue>, F>(
        mut self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        let entry = AgentConfigEntry::new(default, type_);
        self.push_default_config_entry(key.into(), f(entry));
        self
    }

    fn push_default_config_entry(&mut self, key: String, entry: AgentConfigEntry) {
        if let Some(configs) = self.default_configs.as_mut() {
            configs.push((key, entry));
        } else {
            self.default_configs = Some(vec![(key, entry)]);
        }
    }

    // Global Configs

    pub fn with_global_configs(mut self, configs: Vec<(&str, AgentConfigEntry)>) -> Self {
        self.global_configs = Some(configs.into_iter().map(|(k, v)| (k.into(), v)).collect());
        self
    }

    pub fn with_unit_global_config(self, key: &str) -> Self {
        self.with_unit_global_config_with(key, |entry| entry)
    }

    pub fn with_unit_global_config_with<F>(self, key: &str, f: F) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_global_config_type_with(key, (), "unit", f)
    }

    pub fn with_boolean_global_config(self, key: &str, default: bool) -> Self {
        self.with_boolean_global_config_with(key, default, |entry| entry)
    }

    pub fn with_boolean_global_config_with<F>(
        self,
        key: &str,
        default: bool,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_global_config_type_with(key, default, "boolean", f)
    }

    pub fn with_integer_global_config(self, key: &str, default: i64) -> Self {
        self.with_integer_global_config_with(key, default, |entry| entry)
    }

    pub fn with_integer_global_config_with<F>(
        self,
        key: &str,
        default: i64,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_global_config_type_with(key, default, "integer", f)
    }

    pub fn with_number_global_config(self, key: &str, default: f64) -> Self {
        self.with_number_global_config_with(key, default, |entry| entry)
    }

    pub fn with_number_global_config_with<F>(
        self,
        key: &str,
        default: f64,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_global_config_type_with(key, default, "number", f)
    }

    pub fn with_string_global_config(self, key: &str, default: impl Into<String>) -> Self {
        self.with_string_global_config_with(key, default, |entry| entry)
    }

    pub fn with_string_global_config_with<F>(
        self,
        key: &str,
        default: impl Into<String>,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        let default = default.into();
        self.with_global_config_type_with(key, AgentValue::string(default), "string", f)
    }

    pub fn with_text_global_config(self, key: &str, default: impl Into<String>) -> Self {
        self.with_text_global_config_with(key, default, |entry| entry)
    }

    pub fn with_text_global_config_with<F>(
        self,
        key: &str,
        default: impl Into<String>,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        let default = default.into();
        self.with_global_config_type_with(key, AgentValue::string(default), "text", f)
    }

    pub fn with_object_global_config<V: Into<AgentValue>>(self, key: &str, default: V) -> Self {
        self.with_object_global_config_with(key, default, |entry| entry)
    }

    pub fn with_object_global_config_with<V: Into<AgentValue>, F>(
        self,
        key: &str,
        default: V,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_global_config_type_with(key, default, "object", f)
    }

    pub fn with_custom_global_config_with<V: Into<AgentValue>, F>(
        self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        self.with_global_config_type_with(key, default, type_, f)
    }

    fn with_global_config_type<V: Into<AgentValue>>(
        self,
        key: &str,
        default: V,
        type_: &str,
    ) -> Self {
        self.with_global_config_type_with(key, default, type_, |entry| entry)
    }

    fn with_global_config_type_with<V: Into<AgentValue>, F>(
        mut self,
        key: &str,
        default: V,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentConfigEntry) -> AgentConfigEntry,
    {
        let entry = AgentConfigEntry::new(default, type_);
        self.push_global_config_entry(key.into(), f(entry));
        self
    }

    fn push_global_config_entry(&mut self, key: String, entry: AgentConfigEntry) {
        if let Some(configs) = self.global_configs.as_mut() {
            configs.push((key, entry));
        } else {
            self.global_configs = Some(vec![(key, entry)]);
        }
    }

    // Display Configs

    pub fn with_display_configs(mut self, configs: Vec<(&str, AgentDisplayConfigEntry)>) -> Self {
        self.display_configs = Some(configs.into_iter().map(|(k, v)| (k.into(), v)).collect());
        self
    }

    pub fn with_unit_display_config(self, key: &str) -> Self {
        self.with_unit_display_config_with(key, |entry| entry)
    }

    pub fn with_unit_display_config_with<F>(self, key: &str, f: F) -> Self
    where
        F: FnOnce(AgentDisplayConfigEntry) -> AgentDisplayConfigEntry,
    {
        self.with_display_config_type_with(key, "unit", f)
    }

    pub fn with_boolean_display_config(self, key: &str) -> Self {
        self.with_boolean_display_config_with(key, |entry| entry)
    }

    pub fn with_boolean_display_config_with<F>(self, key: &str, f: F) -> Self
    where
        F: FnOnce(AgentDisplayConfigEntry) -> AgentDisplayConfigEntry,
    {
        self.with_display_config_type_with(key, "boolean", f)
    }

    pub fn with_integer_display_config(self, key: &str) -> Self {
        self.with_integer_display_config_with(key, |entry| entry)
    }

    pub fn with_integer_display_config_with<F>(self, key: &str, f: F) -> Self
    where
        F: FnOnce(AgentDisplayConfigEntry) -> AgentDisplayConfigEntry,
    {
        self.with_display_config_type_with(key, "integer", f)
    }

    pub fn with_number_display_config(self, key: &str) -> Self {
        self.with_number_display_config_with(key, |entry| entry)
    }

    pub fn with_number_display_config_with<F>(self, key: &str, f: F) -> Self
    where
        F: FnOnce(AgentDisplayConfigEntry) -> AgentDisplayConfigEntry,
    {
        self.with_display_config_type_with(key, "number", f)
    }

    pub fn with_string_display_config(self, key: &str) -> Self {
        self.with_string_display_config_with(key, |entry| entry)
    }

    pub fn with_string_display_config_with<F>(self, key: &str, f: F) -> Self
    where
        F: FnOnce(AgentDisplayConfigEntry) -> AgentDisplayConfigEntry,
    {
        self.with_display_config_type_with(key, "string", f)
    }

    pub fn with_text_display_config(self, key: &str) -> Self {
        self.with_text_display_config_with(key, |entry| entry)
    }

    pub fn with_text_display_config_with<F>(self, key: &str, f: F) -> Self
    where
        F: FnOnce(AgentDisplayConfigEntry) -> AgentDisplayConfigEntry,
    {
        self.with_display_config_type_with(key, "text", f)
    }

    pub fn with_object_display_config(self, key: &str) -> Self {
        self.with_object_display_config_with(key, |entry| entry)
    }

    pub fn with_object_display_config_with<F>(self, key: &str, f: F) -> Self
    where
        F: FnOnce(AgentDisplayConfigEntry) -> AgentDisplayConfigEntry,
    {
        self.with_display_config_type_with(key, "object", f)
    }

    pub fn with_custom_display_config_with<F>(
        self,
        key: &str,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentDisplayConfigEntry) -> AgentDisplayConfigEntry,
    {
        self.with_display_config_type_with(key, type_, f)
    }

    fn with_display_config_type(self, key: &str, type_: &str) -> Self {
        self.with_display_config_type_with(key, type_, |entry| entry)
    }

    fn with_display_config_type_with<F>(
        mut self,
        key: &str,
        type_: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(AgentDisplayConfigEntry) -> AgentDisplayConfigEntry,
    {
        let entry = AgentDisplayConfigEntry::new(type_);
        self.push_display_config_entry(key.into(), f(entry));
        self
    }

    fn push_display_config_entry(&mut self, key: String, entry: AgentDisplayConfigEntry) {
        if let Some(configs) = self.display_configs.as_mut() {
            configs.push((key, entry));
        } else {
            self.display_configs = Some(vec![(key, entry)]);
        }
    }

    pub fn use_native_thread(mut self) -> Self {
        self.native_thread = true;
        self
    }
}

impl AgentConfigEntry {
    pub fn new<V: Into<AgentValue>>(value: V, type_: &str) -> Self {
        Self {
            value: value.into(),
            type_: Some(type_.into()),
            ..Default::default()
        }
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_hidden(mut self) -> Self {
        self.hidden = true;
        self
    }
}

impl AgentDisplayConfigEntry {
    pub fn new(type_: &str) -> Self {
        Self {
            type_: Some(type_.into()),
            ..Default::default()
        }
    }

    pub fn with_hide_title(mut self) -> Self {
        self.hide_title = true;
        self
    }

    #[allow(unused)]
    pub fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.into());
        self
    }

    #[allow(unused)]
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[cfg(test)]
mod tests {
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
            Some(|_app, _id, _def_name, _configs| {
                Err(AgentError::NotImplemented("Echo agent".into()))
            }),
        );

        assert_eq!(def.kind, "test");
        assert_eq!(def.name, "echo");
        assert!(def.title.is_none());
        assert!(def.category.is_none());
        assert!(def.inputs.is_none());
        assert!(def.outputs.is_none());
        assert!(def.display_configs.is_none());
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
        let display_configs = def.display_configs.unwrap();
        assert_eq!(display_configs.len(), 2);
        let entry = &display_configs[0];
        assert_eq!(entry.0, "value");
        assert_eq!(entry.1.type_.as_ref().unwrap(), "string");
        assert_eq!(entry.1.title.as_ref().unwrap(), "display_title");
        assert_eq!(entry.1.description.as_ref().unwrap(), "display_description");
        assert_eq!(entry.1.hide_title, false);
        let entry = &display_configs[1];
        assert_eq!(entry.0, "hide_title_value");
        assert_eq!(entry.1.type_.as_ref().unwrap(), "integer");
        assert_eq!(entry.1.title, None);
        assert_eq!(entry.1.description, None);
        assert_eq!(entry.1.hide_title, true);
    }

    #[test]
    fn test_serialize_agent_definition() {
        let def = AgentDefinition::new(
            "test",
            "echo",
            Some(|_app, _id, _def_name, _configs| {
                Err(AgentError::NotImplemented("Echo agent".into()))
            }),
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
            r#"{"kind":"test","name":"echo","title":"Echo","category":"Test","inputs":["in"],"outputs":["out"],"display_configs":[["value",{"type":"string","title":"display_title","description":"display_description"}],["hide_title_value",{"type":"integer","hide_title":true}]]}"#
        );
    }

    #[test]
    fn test_deserialize_echo_agent_definition() {
        let json = r#"{"kind":"test","name":"echo","title":"Echo","category":"Test","inputs":["in"],"outputs":["out"],"display_configs":[["value",{"type":"string","title":"display_title","description":"display_description"}],["hide_title_value",{"type":"integer","hide_title":true}]]}"#;
        let def: AgentDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(def.kind, "test");
        assert_eq!(def.name, "echo");
        assert_eq!(def.title.unwrap(), "Echo");
        assert_eq!(def.category.unwrap(), "Test");
        assert_eq!(def.inputs.unwrap(), vec!["in"]);
        assert_eq!(def.outputs.unwrap(), vec!["out"]);
        let display_configs = def.display_configs.unwrap();
        assert_eq!(display_configs.len(), 2);
        let entry = &display_configs[0];
        assert_eq!(entry.0, "value");
        assert_eq!(entry.1.type_.as_ref().unwrap(), "string");
        assert_eq!(entry.1.title.as_ref().unwrap(), "display_title");
        assert_eq!(entry.1.description.as_ref().unwrap(), "display_description");
        assert_eq!(entry.1.hide_title, false);
        let entry = &display_configs[1];
        assert_eq!(entry.0, "hide_title_value");
        assert_eq!(entry.1.type_.as_ref().unwrap(), "integer");
        assert_eq!(entry.1.title, None);
        assert_eq!(entry.1.description, None);
        assert_eq!(entry.1.hide_title, true);
    }

    #[test]
    fn test_default_config_helpers() {
        let custom_object_value = AgentValue::object(
            [("key".to_string(), AgentValue::string("value"))].into(),
        );

        let def = AgentDefinition::new("test", "helpers", None)
            .with_unit_config("unit_value")
            .with_boolean_config_default("boolean_value")
            .with_boolean_config("boolean_custom", true)
            .with_integer_config_default("integer_value")
            .with_integer_config("integer_custom", 42)
            .with_number_config_default("number_value")
            .with_number_config("number_custom", 1.5)
            .with_string_config_default("string_default")
            .with_string_config("string_value", "value")
            .with_text_config_default("text_value")
            .with_text_config("text_custom", "custom")
            .with_object_config_default("object_value")
            .with_object_config("object_custom", custom_object_value.clone());

        let configs = def
            .default_configs
            .clone()
            .expect("default configs should exist");
        assert_eq!(configs.len(), 13);
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

        let object_entry = config_map.get("object_value").unwrap();
        assert_eq!(object_entry.type_.as_deref(), Some("object"));
        assert_eq!(object_entry.value, AgentValue::object_default());

        let object_custom_entry = config_map.get("object_custom").unwrap();
        assert_eq!(object_custom_entry.type_.as_deref(), Some("object"));
        assert_eq!(object_custom_entry.value, custom_object_value);
    }

    #[test]
    fn test_global_config_helpers() {
        let custom_object_value = AgentValue::object(
            [("key".to_string(), AgentValue::string("value"))].into(),
        );

        let def = AgentDefinition::new("test", "helpers", None)
            .with_unit_global_config("global_unit")
            .with_boolean_global_config("global_boolean", true)
            .with_integer_global_config("global_integer", 42)
            .with_number_global_config("global_number", 1.5)
            .with_string_global_config("global_string", "value")
            .with_text_global_config("global_text", "global")
            .with_object_global_config("global_object", custom_object_value.clone());

        let global_configs = def.global_configs.expect("global configs should exist");
        assert_eq!(global_configs.len(), 7);
        let config_map: std::collections::HashMap<_, _> = global_configs.into_iter().collect();

        let entry = config_map.get("global_unit").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("unit"));
        assert_eq!(entry.value, AgentValue::unit());

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

        let entry = config_map.get("global_object").unwrap();
        assert_eq!(entry.type_.as_deref(), Some("object"));
        assert_eq!(entry.value, custom_object_value);
    }

    #[test]
    fn test_display_config_helpers() {
        let def = AgentDefinition::new("test", "helpers", None)
            .with_unit_display_config("display_unit")
            .with_boolean_display_config("display_boolean")
            .with_integer_display_config("display_integer")
            .with_number_display_config("display_number")
            .with_string_display_config("display_string")
            .with_text_display_config("display_text")
            .with_object_display_config("display_object");

        let display_configs = def.display_configs.expect("display configs should exist");
        assert_eq!(display_configs.len(), 7);
        let config_map: std::collections::HashMap<_, _> = display_configs.into_iter().collect();

        assert_eq!(
            config_map.get("display_unit").unwrap().type_.as_deref(),
            Some("unit")
        );
        assert_eq!(
            config_map.get("display_boolean").unwrap().type_.as_deref(),
            Some("boolean")
        );
        assert_eq!(
            config_map.get("display_integer").unwrap().type_.as_deref(),
            Some("integer")
        );
        assert_eq!(
            config_map.get("display_number").unwrap().type_.as_deref(),
            Some("number")
        );
        assert_eq!(
            config_map.get("display_string").unwrap().type_.as_deref(),
            Some("string")
        );
        assert_eq!(
            config_map.get("display_text").unwrap().type_.as_deref(),
            Some("text")
        );
        assert_eq!(
            config_map.get("display_object").unwrap().type_.as_deref(),
            Some("object")
        );

        for entry in config_map.values() {
            assert!(!entry.hide_title);
        }
    }

    #[test]
    fn test_config_helper_customization() {
        let def = AgentDefinition::new("test", "custom", None)
            .with_integer_config_with("custom_default", 1, |entry| entry.with_title("Custom"))
            .with_text_global_config_with("custom_global", "value", |entry| {
                entry.with_description("Global Desc")
            })
            .with_text_display_config_with("custom_display", |entry| entry.with_title("Display"));

        let default_entry = def
            .default_configs
            .as_ref()
            .unwrap()
            .iter()
            .find(|(k, _)| k == "custom_default")
            .map(|(_, v)| v)
            .unwrap();
        assert_eq!(default_entry.title.as_deref(), Some("Custom"));

        let global_entry = def
            .global_configs
            .as_ref()
            .unwrap()
            .iter()
            .find(|(k, _)| k == "custom_global")
            .map(|(_, v)| v)
            .unwrap();
        assert_eq!(global_entry.description.as_deref(), Some("Global Desc"));

        let display_entry = def
            .display_configs
            .as_ref()
            .unwrap()
            .iter()
            .find(|(k, _)| k == "custom_display")
            .map(|(_, v)| v)
            .unwrap();
        assert_eq!(display_entry.title.as_deref(), Some("Display"));
    }

    fn echo_agent_definition() -> AgentDefinition {
        AgentDefinition::new(
            "test",
            "echo",
            Some(|_app, _id, _def_name, _configs| {
                Err(AgentError::NotImplemented("Echo agent".into()))
            }),
        )
        .with_title("Echo")
        .with_category("Test")
        .with_inputs(vec!["in"])
        .with_outputs(vec!["out"])
        .with_display_configs(vec![
            (
                "value",
                AgentDisplayConfigEntry::new("string")
                    .with_title("display_title")
                    .with_description("display_description"),
            ),
            (
                "hide_title_value",
                AgentDisplayConfigEntry::new("integer").with_hide_title(),
            ),
        ])
    }
}
