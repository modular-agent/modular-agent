use im::Vector;
use serde::{Deserialize, Serialize};

use crate::FnvIndexMap;
use crate::error::{Error, Result};
use crate::value::{Value, ValueMap};

/// Type alias for a map of module configurations.
pub type ModuleConfigsMap = FnvIndexMap<String, ModuleConfigs>;

/// Configuration container for a module.
///
/// Holds configuration values in key-value format and provides type-safe accessor methods.
/// Configuration parameters defined with `#[modular_agent]` macro are stored in this struct.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ModuleConfigs(FnvIndexMap<String, Value>);

impl ModuleConfigs {
    /// Creates a new empty configuration container.
    pub fn new() -> Self {
        Self(FnvIndexMap::default())
    }

    /// Sets a configuration value.
    pub fn set(&mut self, key: String, value: Value) {
        self.0.insert(key, value);
    }

    /// Returns `true` if the configuration contains the specified key.
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Gets a configuration value by key.
    ///
    /// # Errors
    ///
    /// Returns `UnknownConfig` if the key does not exist.
    pub fn get(&self, key: &str) -> Result<&Value> {
        self.0
            .get(key)
            .ok_or_else(|| Error::UnknownConfig(key.to_string()))
    }

    /// Gets a boolean configuration value.
    ///
    /// # Errors
    ///
    /// Returns `UnknownConfig` if the key does not exist or cannot be converted to boolean.
    pub fn get_bool(&self, key: &str) -> Result<bool> {
        self.0
            .get(key)
            .and_then(|v| v.as_bool())
            .ok_or_else(|| Error::UnknownConfig(key.to_string()))
    }

    /// Gets a boolean configuration value, or the specified default if not found.
    pub fn get_bool_or(&self, key: &str, default: bool) -> bool {
        self.get_bool(key).unwrap_or(default)
    }

    /// Gets a boolean configuration value, or `false` if not found.
    pub fn get_bool_or_default(&self, key: &str) -> bool {
        self.get_bool(key).unwrap_or_default()
    }

    /// Gets an integer configuration value.
    ///
    /// # Errors
    ///
    /// Returns `UnknownConfig` if the key does not exist or cannot be converted to integer.
    pub fn get_integer(&self, key: &str) -> Result<i64> {
        self.0
            .get(key)
            .and_then(|v| v.as_i64())
            .ok_or_else(|| Error::UnknownConfig(key.to_string()))
    }

    /// Gets an integer configuration value, or the specified default if not found.
    pub fn get_integer_or(&self, key: &str, default: i64) -> i64 {
        self.get_integer(key).unwrap_or(default)
    }

    /// Gets an integer configuration value, or `0` if not found.
    pub fn get_integer_or_default(&self, key: &str) -> i64 {
        self.get_integer(key).unwrap_or_default()
    }

    /// Gets a number (f64) configuration value.
    ///
    /// # Errors
    ///
    /// Returns `UnknownConfig` if the key does not exist or cannot be converted to number.
    pub fn get_number(&self, key: &str) -> Result<f64> {
        self.0
            .get(key)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| Error::UnknownConfig(key.to_string()))
    }

    /// Gets a number configuration value, or the specified default if not found.
    pub fn get_number_or(&self, key: &str, default: f64) -> f64 {
        self.get_number(key).unwrap_or(default)
    }

    /// Gets a number configuration value, or `0.0` if not found.
    pub fn get_number_or_default(&self, key: &str) -> f64 {
        self.get_number(key).unwrap_or_default()
    }

    /// Gets a string configuration value.
    ///
    /// # Errors
    ///
    /// Returns `UnknownConfig` if the key does not exist or cannot be converted to string.
    pub fn get_string(&self, key: &str) -> Result<String> {
        self.0
            .get(key)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .ok_or_else(|| Error::UnknownConfig(key.to_string()))
    }

    /// Gets a string configuration value, or the specified default if not found.
    pub fn get_string_or(&self, key: &str, default: impl Into<String>) -> String {
        self.0
            .get(key)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .unwrap_or(default.into())
    }

    /// Gets a string configuration value, or an empty string if not found.
    pub fn get_string_or_default(&self, key: &str) -> String {
        self.0
            .get(key)
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .unwrap_or_default()
    }

    /// Gets an array configuration value.
    ///
    /// # Errors
    ///
    /// Returns `UnknownConfig` if the key does not exist or cannot be converted to array.
    pub fn get_array(&self, key: &str) -> Result<&Vector<Value>> {
        self.0
            .get(key)
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::UnknownConfig(key.to_string()))
    }

    /// Gets an array configuration value, or the specified default if not found.
    pub fn get_array_or<'a>(&'a self, key: &str, default: &'a Vector<Value>) -> &'a Vector<Value> {
        self.0
            .get(key)
            .and_then(|v| v.as_array())
            .unwrap_or(default)
    }

    /// Gets an array configuration value, or an empty array if not found.
    pub fn get_array_or_default(&self, key: &str) -> Vector<Value> {
        self.0
            .get(key)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    }

    /// Gets an object configuration value.
    ///
    /// # Errors
    ///
    /// Returns `UnknownConfig` if the key does not exist or cannot be converted to object.
    pub fn get_object(&self, key: &str) -> Result<&ValueMap<String, Value>> {
        self.0
            .get(key)
            .and_then(|v| v.as_object())
            .ok_or_else(|| Error::UnknownConfig(key.to_string()))
    }

    /// Gets an object configuration value, or the specified default if not found.
    pub fn get_object_or<'a>(
        &'a self,
        key: &str,
        default: &'a ValueMap<String, Value>,
    ) -> &'a ValueMap<String, Value> {
        self.0
            .get(key)
            .and_then(|v| v.as_object())
            .unwrap_or(default)
    }

    /// Gets an object configuration value, or an empty object if not found.
    pub fn get_object_or_default(&self, key: &str) -> ValueMap<String, Value> {
        self.0
            .get(key)
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default()
    }

    /// Returns an iterator over the configuration keys.
    pub fn keys(&self) -> indexmap::map::Keys<'_, String, Value> {
        self.0.keys()
    }

    /// Removes a configuration value by key, returning the value if it existed.
    ///
    /// Uses `shift_remove` to preserve insertion order.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.0.shift_remove(key)
    }

    /// Retains only the configuration entries for which the predicate returns `true`.
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&String, &mut Value) -> bool,
    {
        self.0.retain(f);
    }
}

impl IntoIterator for ModuleConfigs {
    type Item = (String, Value);
    type IntoIter = indexmap::map::IntoIter<String, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a ModuleConfigs {
    type Item = (&'a String, &'a Value);
    type IntoIter = indexmap::map::Iter<'a, String, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<(String, Value)> for ModuleConfigs {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        let mut configs = ModuleConfigs::new();
        for (key, value) in iter {
            configs.set(key, value);
        }
        configs
    }
}

impl<'a> FromIterator<(&'a String, &'a Value)> for ModuleConfigs {
    fn from_iter<T: IntoIterator<Item = (&'a String, &'a Value)>>(iter: T) -> Self {
        let mut configs = ModuleConfigs::new();
        for (key, value) in iter {
            configs.set(key.clone(), value.clone());
        }
        configs
    }
}
