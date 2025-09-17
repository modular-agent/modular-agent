use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::data::AgentValue;
use crate::error::AgentError;

pub type AgentConfigs = HashMap<String, AgentConfig>;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AgentConfig(BTreeMap<String, AgentValue>);

impl AgentConfig {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn set(&mut self, key: String, value: AgentValue) {
        self.0.insert(key, value);
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Result<&AgentValue, AgentError> {
        self.0
            .get(key)
            .ok_or_else(|| AgentError::UnknownConfig(key.to_string()))
    }

    pub fn get_bool(&self, key: &str) -> Result<bool, AgentError> {
        self.0
            .get(key)
            .and_then(|v| v.as_bool())
            .ok_or_else(|| AgentError::UnknownConfig(key.to_string()))
    }

    pub fn get_bool_or(&self, key: &str, default: bool) -> bool {
        self.get_bool(key).unwrap_or(default)
    }

    pub fn get_bool_or_default(&self, key: &str) -> bool {
        self.get_bool(key).unwrap_or_default()
    }

    pub fn get_integer(&self, key: &str) -> Result<i64, AgentError> {
        self.0
            .get(key)
            .and_then(|v| v.as_i64())
            .ok_or_else(|| AgentError::UnknownConfig(key.to_string()))
    }

    pub fn get_integer_or(&self, key: &str, default: i64) -> i64 {
        self.get_integer(key).unwrap_or(default)
    }

    pub fn get_integer_or_default(&self, key: &str) -> i64 {
        self.get_integer(key).unwrap_or_default()
    }

    pub fn get_number(&self, key: &str) -> Result<f64, AgentError> {
        self.0
            .get(key)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| AgentError::UnknownConfig(key.to_string()))
    }

    pub fn get_number_or(&self, key: &str, default: f64) -> f64 {
        self.get_number(key).unwrap_or(default)
    }

    pub fn get_number_or_default(&self, key: &str) -> f64 {
        self.get_number(key).unwrap_or_default()
    }

    pub fn get_string(&self, key: &str) -> Result<String, AgentError> {
        self.0
            .get(key)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .ok_or_else(|| AgentError::UnknownConfig(key.to_string()))
    }

    pub fn get_string_or(&self, key: &str, default: impl Into<String>) -> String {
        self.0
            .get(key)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .unwrap_or(default.into())
    }

    pub fn get_string_or_default(&self, key: &str) -> String {
        self.0
            .get(key)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .unwrap_or_default()
    }

    pub fn get_array(&self, key: &str) -> Result<&Vec<AgentValue>, AgentError> {
        self.0
            .get(key)
            .and_then(|v| v.as_array())
            .ok_or_else(|| AgentError::UnknownConfig(key.to_string()))
    }

    pub fn get_array_or<'a>(
        &'a self,
        key: &str,
        default: &'a Vec<AgentValue>,
    ) -> &'a Vec<AgentValue> {
        self.0
            .get(key)
            .and_then(|v| v.as_array())
            .unwrap_or(default)
    }

    pub fn get_array_or_default(&self, key: &str) -> Vec<AgentValue> {
        self.0
            .get(key)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_object(&self, key: &str) -> Result<&BTreeMap<String, AgentValue>, AgentError> {
        self.0
            .get(key)
            .and_then(|v| v.as_object())
            .ok_or_else(|| AgentError::UnknownConfig(key.to_string()))
    }

    pub fn get_object_or<'a>(
        &'a self,
        key: &str,
        default: &'a BTreeMap<String, AgentValue>,
    ) -> &'a BTreeMap<String, AgentValue> {
        self.0
            .get(key)
            .and_then(|v| v.as_object())
            .unwrap_or(default)
    }

    pub fn get_object_or_default(&self, key: &str) -> BTreeMap<String, AgentValue> {
        self.0
            .get(key)
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default()
    }
}

impl IntoIterator for AgentConfig {
    type Item = (String, AgentValue);
    type IntoIter = std::collections::btree_map::IntoIter<String, AgentValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a AgentConfig {
    type Item = (&'a String, &'a AgentValue);
    type IntoIter = std::collections::btree_map::Iter<'a, String, AgentValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
