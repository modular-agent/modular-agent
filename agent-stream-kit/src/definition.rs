use std::collections::HashMap;
use std::ops::Not;

use serde::{Deserialize, Serialize};

use super::agent::Agent;
use super::askit::ASKit;
use super::config::AgentConfig;
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
    pub default_config: Option<AgentDefaultConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_config: Option<AgentGlobalConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_config: Option<AgentDisplayConfig>,

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub native_thread: bool,

    #[serde(skip)]
    pub new_boxed: Option<AgentNewBoxedFn>,
}

pub type AgentDefaultConfig = Vec<(String, AgentConfigEntry)>;
pub type AgentGlobalConfig = Vec<(String, AgentConfigEntry)>;

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

pub type AgentDisplayConfig = Vec<(String, AgentDisplayConfigEntry)>;

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
    config: Option<AgentConfig>,
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

    pub fn with_default_config(mut self, config: Vec<(&str, AgentConfigEntry)>) -> Self {
        self.default_config = Some(config.into_iter().map(|(k, v)| (k.into(), v)).collect());
        self
    }

    #[allow(unused)]
    pub fn with_global_config(mut self, config: Vec<(&str, AgentConfigEntry)>) -> Self {
        self.global_config = Some(config.into_iter().map(|(k, v)| (k.into(), v)).collect());
        self
    }

    pub fn with_display_config(mut self, config: Vec<(&str, AgentDisplayConfigEntry)>) -> Self {
        self.display_config = Some(config.into_iter().map(|(k, v)| (k.into(), v)).collect());
        self
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
            Some(|_app, _id, _def_name, _config| {
                Err(AgentError::NotImplemented("Echo agent".into()))
            }),
        );

        assert_eq!(def.kind, "test");
        assert_eq!(def.name, "echo");
        assert!(def.title.is_none());
        assert!(def.category.is_none());
        assert!(def.inputs.is_none());
        assert!(def.outputs.is_none());
        assert!(def.display_config.is_none());
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
        let display_config = def.display_config.unwrap();
        assert_eq!(display_config.len(), 2);
        let entry = &display_config[0];
        assert_eq!(entry.0, "value");
        assert_eq!(entry.1.type_.as_ref().unwrap(), "string");
        assert_eq!(entry.1.title.as_ref().unwrap(), "display_title");
        assert_eq!(entry.1.description.as_ref().unwrap(), "display_description");
        assert_eq!(entry.1.hide_title, false);
        let entry = &display_config[1];
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
            Some(|_app, _id, _def_name, _config| {
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
            r#"{"kind":"test","name":"echo","title":"Echo","category":"Test","inputs":["in"],"outputs":["out"],"display_config":[["value",{"type":"string","title":"display_title","description":"display_description"}],["hide_title_value",{"type":"integer","hide_title":true}]]}"#
        );
    }

    #[test]
    fn test_deserialize_echo_agent_definition() {
        let json = r#"{"kind":"test","name":"echo","title":"Echo","category":"Test","inputs":["in"],"outputs":["out"],"display_config":[["value",{"type":"string","title":"display_title","description":"display_description"}],["hide_title_value",{"type":"integer","hide_title":true}]]}"#;
        let def: AgentDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(def.kind, "test");
        assert_eq!(def.name, "echo");
        assert_eq!(def.title.unwrap(), "Echo");
        assert_eq!(def.category.unwrap(), "Test");
        assert_eq!(def.inputs.unwrap(), vec!["in"]);
        assert_eq!(def.outputs.unwrap(), vec!["out"]);
        let display_config = def.display_config.unwrap();
        assert_eq!(display_config.len(), 2);
        let entry = &display_config[0];
        assert_eq!(entry.0, "value");
        assert_eq!(entry.1.type_.as_ref().unwrap(), "string");
        assert_eq!(entry.1.title.as_ref().unwrap(), "display_title");
        assert_eq!(entry.1.description.as_ref().unwrap(), "display_description");
        assert_eq!(entry.1.hide_title, false);
        let entry = &display_config[1];
        assert_eq!(entry.0, "hide_title_value");
        assert_eq!(entry.1.type_.as_ref().unwrap(), "integer");
        assert_eq!(entry.1.title, None);
        assert_eq!(entry.1.description, None);
        assert_eq!(entry.1.hide_title, true);
    }

    fn echo_agent_definition() -> AgentDefinition {
        AgentDefinition::new(
            "test",
            "echo",
            Some(|_app, _id, _def_name, _config| {
                Err(AgentError::NotImplemented("Echo agent".into()))
            }),
        )
        .with_title("Echo")
        .with_category("Test")
        .with_inputs(vec!["in"])
        .with_outputs(vec!["out"])
        .with_display_config(vec![
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
