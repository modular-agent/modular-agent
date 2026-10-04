//! A workspace: the directory a host runs patches from.
//!
//! ```text
//! <workspace>/
//!   settings.json   # global configs and the patches to start automatically
//!   patches/        # patch JSON files; '/' in a patch name is a subdirectory
//! ```
//!
//! The desktop app and the `ma` CLI open the same layout, so a workspace built
//! in the editor runs unchanged under `ma serve`.

use std::fmt;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::config::ModuleConfigs;
use crate::error::{Error, Result};
use crate::modular_agent::ModularAgent;
use crate::patch::validate_patch_name;
use crate::value::Value;

const SETTINGS_FILE: &str = "settings.json";
const PATCHES_DIR: &str = "patches";

/// The contents of `settings.json`.
///
/// Values are kept as written, `${VAR}` references included, so saving never
/// writes a secret resolved from the environment back to the file.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSettings {
    /// Global configs: `{def_name: {key: value}}`.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub modules: IndexMap<String, IndexMap<String, JsonValue>>,

    /// Names of the patches to open and start when the workspace is loaded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto_start_patches: Vec<String>,
}

/// A global config entry that [`Workspace::apply_global_configs`] skipped.
///
/// Whether these are fatal is up to the host: a headless runner should refuse
/// to start, since nobody would notice a setting that silently does nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceWarning {
    /// No registered module definition has this name.
    UnknownModule { def_name: String },
    /// The module definition has no global config with this key.
    UnknownConfig { def_name: String, key: String },
    /// The value references an environment variable that is not set.
    UndefinedVariable {
        def_name: String,
        key: String,
        var: String,
    },
    /// The value could not be converted to a config value.
    InvalidValue {
        def_name: String,
        key: String,
        message: String,
    },
}

impl fmt::Display for WorkspaceWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownModule { def_name } => {
                write!(f, "unknown module definition \"{def_name}\"")
            }
            Self::UnknownConfig { def_name, key } => {
                write!(f, "{def_name}: unknown global config \"{key}\"")
            }
            Self::UndefinedVariable { def_name, key, var } => {
                write!(f, "{def_name}.{key}: environment variable {var} is not set")
            }
            Self::InvalidValue {
                def_name,
                key,
                message,
            } => write!(f, "{def_name}.{key}: {message}"),
        }
    }
}

/// A workspace directory and its settings.
#[derive(Debug, Clone)]
pub struct Workspace {
    dir: PathBuf,
    pub settings: WorkspaceSettings,
}

impl Workspace {
    /// Opens the workspace at `dir`.
    ///
    /// A missing `settings.json` means default settings. Fails when `dir` is
    /// not a directory, or when `settings.json` is not valid JSON or has keys
    /// other than `modules` and `auto_start_patches`.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        if !dir.is_dir() {
            return Err(Error::IoError(format!(
                "workspace {} is not a directory",
                dir.display()
            )));
        }
        let path = dir.join(SETTINGS_FILE);
        let settings = match std::fs::read_to_string(&path) {
            Ok(json) => serde_json::from_str(&json)
                .map_err(|e| Error::JsonParseError(format!("{}: {e}", path.display())))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => WorkspaceSettings::default(),
            Err(e) => return Err(Error::IoError(format!("{}: {e}", path.display()))),
        };
        Ok(Self { dir, settings })
    }

    /// Writes the settings back to `settings.json`.
    pub fn save(&self) -> Result<()> {
        let path = self.dir.join(SETTINGS_FILE);
        let json = serde_json::to_string_pretty(&self.settings)
            .map_err(|e| Error::SerializationError(e.to_string()))?;
        std::fs::write(&path, json).map_err(|e| Error::IoError(format!("{}: {e}", path.display())))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn patches_dir(&self) -> PathBuf {
        self.dir.join(PATCHES_DIR)
    }

    /// The file a patch name maps to: `/` separates folders, and `.json` is
    /// appended (not substituted, so `v1.2` maps to `v1.2.json`).
    pub fn patch_path(&self, name: &str) -> Result<PathBuf> {
        validate_patch_name(name)?;
        let mut path = self.patches_dir();
        path.extend(name.split('/'));
        path.as_mut_os_string().push(".json");
        Ok(path)
    }

    /// Opens the named patch, or returns its id if a patch with that name is
    /// already loaded.
    pub async fn open_patch(&self, ma: &ModularAgent, name: &str) -> Result<String> {
        if let Some(id) = ma.find_patch_id_by_name(name) {
            return Ok(id);
        }
        let path = self.patch_path(name)?;
        ma.open_patch_from_file(&path.to_string_lossy(), Some(name.to_string()))
            .await
    }

    /// Applies `modules` as global configs, expanding `${VAR}` from the
    /// environment. Entries that cannot be applied are skipped and returned.
    pub fn apply_global_configs(&self, ma: &ModularAgent) -> Vec<WorkspaceWarning> {
        self.apply_global_configs_with(ma, |var| std::env::var(var).ok())
    }

    fn apply_global_configs_with(
        &self,
        ma: &ModularAgent,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Vec<WorkspaceWarning> {
        let known = ma.get_global_configs_map();
        let mut warnings = Vec::new();
        for (def_name, entries) in &self.settings.modules {
            let Some(known_configs) = known.get(def_name) else {
                warnings.push(WorkspaceWarning::UnknownModule {
                    def_name: def_name.clone(),
                });
                continue;
            };
            let mut configs = ModuleConfigs::default();
            for (key, raw) in entries {
                if !known_configs.contains_key(key) {
                    warnings.push(WorkspaceWarning::UnknownConfig {
                        def_name: def_name.clone(),
                        key: key.clone(),
                    });
                    continue;
                }
                let expanded = match expand_json(raw, &lookup) {
                    Ok(v) => v,
                    Err(var) => {
                        warnings.push(WorkspaceWarning::UndefinedVariable {
                            def_name: def_name.clone(),
                            key: key.clone(),
                            var,
                        });
                        continue;
                    }
                };
                match Value::from_json(expanded) {
                    Ok(value) => configs.set(key.clone(), value),
                    Err(e) => warnings.push(WorkspaceWarning::InvalidValue {
                        def_name: def_name.clone(),
                        key: key.clone(),
                        message: e.to_string(),
                    }),
                }
            }
            ma.set_global_configs(def_name.clone(), configs);
        }
        warnings
    }

    /// Opens and starts every patch in `auto_start_patches`. Keeps going past
    /// failures and returns them with the patch name.
    pub async fn auto_start(&self, ma: &ModularAgent) -> Vec<(String, Error)> {
        let mut failures = Vec::new();
        for name in &self.settings.auto_start_patches {
            let result = match self.open_patch(ma, name).await {
                Ok(id) => ma.start_patch(&id).await,
                Err(e) => Err(e),
            };
            if let Err(e) = result {
                failures.push((name.clone(), e));
            }
        }
        failures
    }
}

/// Expands `${VAR}` in every string inside `value`. Returns the name of the
/// first undefined variable on failure.
fn expand_json(
    value: &JsonValue,
    lookup: &impl Fn(&str) -> Option<String>,
) -> std::result::Result<JsonValue, String> {
    Ok(match value {
        JsonValue::String(s) => JsonValue::String(expand_str(s, lookup)?),
        JsonValue::Array(items) => JsonValue::Array(
            items
                .iter()
                .map(|v| expand_json(v, lookup))
                .collect::<std::result::Result<_, _>>()?,
        ),
        JsonValue::Object(map) => JsonValue::Object(
            map.iter()
                .map(|(k, v)| Ok((k.clone(), expand_json(v, lookup)?)))
                .collect::<std::result::Result<_, String>>()?,
        ),
        other => other.clone(),
    })
}

/// Replaces `${NAME}` (NAME = `[A-Za-z_][A-Za-z0-9_]*`) with its value; `$${`
/// yields a literal `${`. Anything else, such as `${}` or an unclosed `${`,
/// is kept as written.
fn expand_str(
    s: &str,
    lookup: &impl Fn(&str) -> Option<String>,
) -> std::result::Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(after) = tail.strip_prefix("$${") {
            out.push_str("${");
            rest = after;
        } else if let Some((name, after)) = tail
            .strip_prefix("${")
            .and_then(|t| t.split_once('}'))
            .filter(|(name, _)| is_var_name(name))
        {
            out.push_str(&lookup(name).ok_or_else(|| name.to_string())?);
            rest = after;
        } else {
            out.push('$');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    Ok(out)
}

fn is_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::ModuleDefinition;
    use serde_json::json;

    fn lookup(var: &str) -> Option<String> {
        match var {
            "TOKEN" => Some("secret".into()),
            "EMPTY" => Some(String::new()),
            _ => None,
        }
    }

    #[test]
    fn expands_variables() {
        assert_eq!(expand_str("${TOKEN}", &lookup), Ok("secret".into()));
        assert_eq!(
            expand_str("a-${TOKEN}-${EMPTY}b", &lookup),
            Ok("a-secret-b".into())
        );
        assert_eq!(expand_str("$${TOKEN}", &lookup), Ok("${TOKEN}".into()));
        assert_eq!(
            expand_str("$5 ${} ${1X} ${open", &lookup),
            Ok("$5 ${} ${1X} ${open".into())
        );
        assert_eq!(expand_str("${MISSING}", &lookup), Err("MISSING".into()));
        assert_eq!(
            expand_json(&json!({"a": ["${TOKEN}", 1], "b": null}), &lookup),
            Ok(json!({"a": ["secret", 1], "b": null}))
        );
    }

    #[test]
    fn patch_path_maps_folders_and_appends_extension() {
        let ws = Workspace {
            dir: PathBuf::from("ws"),
            settings: WorkspaceSettings::default(),
        };
        assert_eq!(
            ws.patch_path("a/b").unwrap(),
            Path::new("ws").join("patches").join("a").join("b.json")
        );
        assert_eq!(
            ws.patch_path("v1.2").unwrap(),
            Path::new("ws").join("patches").join("v1.2.json")
        );
        assert!(ws.patch_path("../x").is_err());
    }

    #[test]
    fn open_and_save_settings() {
        let dir = tempfile::tempdir().unwrap();

        let ws = Workspace::open(dir.path()).unwrap();
        assert_eq!(ws.settings, WorkspaceSettings::default());

        let mut ws = ws;
        ws.settings
            .modules
            .entry("def".into())
            .or_default()
            .insert("token".into(), json!("${TOKEN}"));
        ws.settings.auto_start_patches.push("a/b".into());
        ws.save().unwrap();
        assert_eq!(Workspace::open(dir.path()).unwrap().settings, ws.settings);

        std::fs::write(dir.path().join(SETTINGS_FILE), r#"{"auto_start": []}"#).unwrap();
        assert!(Workspace::open(dir.path()).is_err());

        assert!(Workspace::open(dir.path().join("missing")).is_err());
    }

    #[test]
    fn apply_global_configs_reports_skipped_entries() {
        let ma = ModularAgent::new();
        ma.register_module_definition(
            ModuleDefinition::new("test", "ws_def", None)
                .string_global_config("token", "")
                .string_global_config("other", "keep"),
        );
        let ws = Workspace {
            dir: PathBuf::from("ws"),
            // Parsed from text like settings.json: json! would sort the keys.
            settings: serde_json::from_str(
                r#"{
                    "modules": {
                        "ws_def": { "token": "${TOKEN}", "other": "${MISSING}", "nope": 1 },
                        "no_such_def": { "k": "v" }
                    }
                }"#,
            )
            .unwrap(),
        };

        let warnings = ws.apply_global_configs_with(&ma, lookup);

        let configs = ma.get_global_configs("ws_def").unwrap();
        assert_eq!(configs.get_string("token").unwrap(), "secret");
        assert_eq!(configs.get_string("other").unwrap(), "keep");
        assert_eq!(
            warnings,
            vec![
                WorkspaceWarning::UndefinedVariable {
                    def_name: "ws_def".into(),
                    key: "other".into(),
                    var: "MISSING".into(),
                },
                WorkspaceWarning::UnknownConfig {
                    def_name: "ws_def".into(),
                    key: "nope".into(),
                },
                WorkspaceWarning::UnknownModule {
                    def_name: "no_such_def".into(),
                },
            ]
        );
    }
}
