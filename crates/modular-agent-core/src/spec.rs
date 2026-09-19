use std::ops::Not;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::FnvIndexMap;
use crate::config::ModuleConfigs;
use crate::definition::ModuleConfigSpecs;
use crate::error::{Error, Result};

/// A map of patch names to their specifications.
pub type PatchSpecs = FnvIndexMap<String, PatchSpec>;

/// The serializable specification of a patch (workflow).
///
/// A patch defines a complete workflow configuration including all modules
/// and their connections. This struct is designed for JSON serialization
/// and can be loaded from or saved to patch files.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PatchSpec {
    /// List of module specifications in this patch.
    pub modules: Vec<ModuleSpec>,

    /// List of connections between modules.
    pub connections: Vec<ConnectionSpec>,

    /// Extension fields for custom data.
    ///
    /// Any JSON fields not matching defined fields are captured here.
    #[serde(flatten)]
    pub extensions: FnvIndexMap<String, JsonValue>,
}

impl PatchSpec {
    /// Adds a module to this patch.
    pub fn add_module(&mut self, module: ModuleSpec) {
        self.modules.push(module);
    }

    /// Removes a module from this patch by its ID.
    pub fn remove_module(&mut self, module_id: &str) {
        self.modules.retain(|module| module.id != module_id);
    }

    /// Adds a connection to this patch.
    pub fn add_connection(&mut self, connection: ConnectionSpec) {
        self.connections.push(connection);
    }

    /// Removes a connection from this patch.
    ///
    /// Returns `Some(ConnectionSpec)` if the connection was found and removed,
    /// or `None` if it was not found.
    pub fn remove_connection(&mut self, connection: &ConnectionSpec) -> Option<ConnectionSpec> {
        let index = self.connections.iter().position(|c| c == connection)?;
        Some(self.connections.remove(index))
    }

    /// Serializes this patch to a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| Error::SerializationError(e.to_string()))?;
        Ok(json)
    }

    /// Deserializes a patch from a JSON string.
    pub fn from_json(json_str: &str) -> Result<Self> {
        let patch: PatchSpec =
            serde_json::from_str(json_str).map_err(|e| Error::SerializationError(e.to_string()))?;
        Ok(patch)
    }
}

/// The runtime specification of a module instance.
///
/// Contains all the information needed to instantiate and configure a module,
/// including its ID, definition reference, ports, and configuration values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModuleSpec {
    /// Unique identifier for this module instance.
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub id: String,

    /// Name of the ModuleDefinition this module is based on.
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub def_name: String,

    /// List of input port names (overrides definition if set).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inputs: Option<Vec<String>>,

    /// List of output port names (overrides definition if set).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub outputs: Option<Vec<String>>,

    /// Configuration values for this module instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configs: Option<ModuleConfigs>,

    /// Configuration specifications (metadata about configs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_specs: Option<ModuleConfigSpecs>,

    /// Whether this module is disabled (will not be started).
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub disabled: bool,

    /// Extension fields for custom data.
    #[serde(flatten)]
    pub extensions: FnvIndexMap<String, serde_json::Value>,
}

impl ModuleSpec {
    /// Updates this module spec from a JSON value.
    ///
    /// Known fields (id, def_name, inputs, outputs, configs, disabled) are parsed
    /// and applied. `configs` is merged key by key into the current values;
    /// the other fields are replaced. Unknown fields are stored in the
    /// extensions map.
    pub fn update(&mut self, value: &JsonValue) -> Result<()> {
        let update_map = value
            .as_object()
            .ok_or_else(|| Error::SerializationError("Expected JSON object".to_string()))?;

        for (k, v) in update_map {
            match k.as_str() {
                "id" => {
                    if let Some(id_str) = v.as_str() {
                        self.id = id_str.to_string();
                    }
                }
                "def_name" => {
                    if let Some(def_name_str) = v.as_str() {
                        self.def_name = def_name_str.to_string();
                    }
                }
                "inputs" => {
                    if let Some(inputs_array) = v.as_array() {
                        self.inputs = Some(
                            inputs_array
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect(),
                        );
                    }
                }
                "outputs" => {
                    if let Some(outputs_array) = v.as_array() {
                        self.outputs = Some(
                            outputs_array
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect(),
                        );
                    }
                }
                "configs" => {
                    let patch: ModuleConfigs = serde_json::from_value(v.clone())
                        .map_err(|e| Error::SerializationError(e.to_string()))?;
                    // Merge instead of replacing: callers patch single keys,
                    // and replacing would drop the untouched configs - which
                    // modules that regenerate configs/ports in configs_changed
                    // (e.g. from an "n" config) would then rebuild from
                    // defaults, destroying ports that still have connections.
                    match self.configs.as_mut() {
                        Some(configs) => {
                            for (key, value) in patch {
                                configs.set(key, value);
                            }
                        }
                        None => self.configs = Some(patch),
                    }
                }
                "disabled" => {
                    if let Some(disabled_bool) = v.as_bool() {
                        self.disabled = disabled_bool;
                    }
                }
                _ => {
                    // Update extensions: null removes the key
                    if v.is_null() {
                        self.extensions.shift_remove(k);
                    } else {
                        self.extensions.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        Ok(())
    }
}

/// A connection between two module ports.
///
/// Defines a directed edge in the module graph, connecting an output port
/// of a source module to an input port of a target module.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConnectionSpec {
    /// ID of the source module.
    pub source: String,

    /// Output port name on the source module.
    pub source_handle: String,

    /// ID of the target module.
    pub target: String,

    /// Input port name on the target module.
    pub target_handle: String,
}
