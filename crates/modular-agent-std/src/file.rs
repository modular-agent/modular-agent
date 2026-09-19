use std::fs;
use std::io::Write;
use std::path::Path;

use glob::glob;
use im::hashmap;
use modular_agent_core::{
    AsModule, Error, ModularAgent, Module, ModuleContext, ModuleData, ModuleOutput, ModuleSpec,
    Result, Value, async_trait, modular_agent,
};

const CATEGORY: &str = "Std/File";

const CONFIG_PATH: &str = "path";

const PORT_ARRAY: &str = "array";
const PORT_DOC: &str = "doc";
const PORT_FILES: &str = "files";
const PORT_PATH: &str = "path";
const PORT_STRING: &str = "string";
const PORT_UNIT: &str = "unit";
const PORT_VALUE: &str = "value";

// Glob Module
#[modular_agent(
    title = "Glob",
    category = CATEGORY,
    inputs = [PORT_PATH],
    outputs = [PORT_FILES]
)]
struct GlobModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for GlobModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let pat = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("not a string".to_string()))?;

        let mut files = Vec::new();

        for entry in glob(pat).map_err(|e| {
            Error::InvalidValue(format!("Failed to read glob pattern {}: {}", pat, e))
        })? {
            match entry {
                Ok(path) => {
                    files.push(path.to_string_lossy().to_string().into());
                }
                Err(e) => {
                    return Err(Error::InvalidValue(format!(
                        "Failed to read glob entry: {}",
                        e
                    )));
                }
            }
        }

        let out_value = Value::array(files.into());
        self.output(ctx, PORT_FILES, out_value).await
    }
}

// List Files Module
#[modular_agent(
    title = "List Files",
    category = CATEGORY,
    inputs = [PORT_PATH],
    outputs = [PORT_FILES]
)]
struct ListFilesModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for ListFilesModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let path = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("path is not a string".to_string()))?;
        let path = Path::new(path);

        if !path.exists() {
            return Err(Error::InvalidValue(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        if !path.is_dir() {
            return Err(Error::InvalidValue(format!(
                "Path is not a directory: {}",
                path.display()
            )));
        }

        let mut files = Vec::new();
        let entries = fs::read_dir(path).map_err(|e| {
            Error::InvalidValue(format!(
                "Failed to read directory {}: {}",
                path.display(),
                e
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                Error::InvalidValue(format!("Failed to read directory entry: {}", e))
            })?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            files.push(file_name.into());
        }

        let out_value = Value::array(files.into());
        self.output(ctx, PORT_FILES, out_value).await
    }
}

// Read Text File Module
#[modular_agent(
    title = "Read Text File",
    category = CATEGORY,
    inputs = [PORT_PATH],
    outputs = [PORT_STRING, PORT_DOC]
)]
struct ReadTextFileModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for ReadTextFileModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let path = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("path is not a string".into()))?;
        let path = Path::new(path);

        if !path.exists() {
            return Err(Error::InvalidValue(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        if !path.is_file() {
            return Err(Error::InvalidValue(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        let content = fs::read_to_string(path).map_err(|e| {
            Error::InvalidValue(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        let text = Value::string(content);
        self.output(ctx.clone(), PORT_STRING, text.clone()).await?;

        let out_doc = Value::object(hashmap! {
            "path".into() => Value::string(path.to_string_lossy().to_string()),
            "text".into() => text,
        });
        self.output(ctx, PORT_DOC, out_doc).await
    }
}

// Write Text File Module
#[modular_agent(
    title = "Write Text File",
    category = CATEGORY,
    inputs = [PORT_STRING, PORT_DOC],
    outputs = [PORT_UNIT],
    string_config(name = CONFIG_PATH),
)]
struct WriteTextFileModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for WriteTextFileModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        let (path, text) = if port == PORT_STRING {
            let path = self.configs()?.get_string(CONFIG_PATH)?;
            let text = value
                .to_string()
                .ok_or_else(|| Error::InvalidValue("Input value is not a string".into()))?;
            (path, text)
        } else if port == PORT_DOC {
            let path = if let Some(path) = value.get_str("path") {
                path.to_string()
            } else {
                self.configs()?.get_string(CONFIG_PATH)?
            };
            let text = value
                .get_str("text")
                .ok_or_else(|| Error::InvalidValue("Input value is not an object".into()))?
                .to_string();
            (path, text)
        } else {
            return Err(Error::InvalidPin(port));
        };

        let path = Path::new(&path);

        // Ensure parent directories exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    Error::InvalidValue(format!("Failed to create parent directories: {}", e))
                })?
            }
        }

        fs::write(path, text).map_err(|e| {
            Error::InvalidValue(format!("Failed to write file {}: {}", path.display(), e))
        })?;

        self.output(ctx, PORT_UNIT, Value::unit()).await
    }
}

// Read JSON File Module
#[modular_agent(
    title = "Read JSON File",
    category = CATEGORY,
    inputs = [PORT_PATH],
    outputs = [PORT_VALUE, PORT_DOC]
)]
struct ReadJsonFileModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for ReadJsonFileModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let path = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("path is not a string".into()))?;
        let path = Path::new(path);

        if !path.exists() {
            return Err(Error::InvalidValue(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        if !path.is_file() {
            return Err(Error::InvalidValue(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        let content = fs::read_to_string(path).map_err(|e| {
            Error::InvalidValue(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        let json = serde_json::from_str::<serde_json::Value>(&content).map_err(|e| {
            Error::InvalidValue(format!(
                "Failed to parse JSON from file {}: {}",
                path.display(),
                e
            ))
        })?;

        let value = Value::from_json(json)?;
        self.output(ctx.clone(), PORT_VALUE, value.clone()).await?;

        let out_doc = Value::object(hashmap! {
            "path".into() => Value::string(path.to_string_lossy().to_string()),
            "value".into() => value,
        });
        self.output(ctx, PORT_DOC, out_doc).await
    }
}

// Write JSON File Module
#[modular_agent(
    title = "Write JSON File",
    category = CATEGORY,
    inputs = [PORT_VALUE, PORT_DOC],
    outputs = [PORT_UNIT],
    string_config(name = CONFIG_PATH),
)]
struct WriteJsonFileModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for WriteJsonFileModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        let (path, value) = if port == PORT_VALUE {
            let path = self.configs()?.get_string(CONFIG_PATH)?;
            (path, value)
        } else if port == PORT_DOC {
            let path = if let Some(path) = value.get_str("path") {
                path.to_string()
            } else {
                self.configs()?.get_string(CONFIG_PATH)?
            };
            let value = value
                .get("value")
                .ok_or_else(|| Error::InvalidValue("Input doc is missing 'value' field".into()))?;
            (path, value.clone())
        } else {
            return Err(Error::InvalidPin(port));
        };

        let path = Path::new(&path);

        // Ensure parent directories exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    Error::InvalidValue(format!("Failed to create parent directories: {}", e))
                })?
            }
        }

        fs::write(path, value.to_json().to_string()).map_err(|e| {
            Error::InvalidValue(format!("Failed to write file {}: {}", path.display(), e))
        })?;

        self.output(ctx, PORT_UNIT, Value::unit()).await
    }
}

// Read JSONL File Module
#[modular_agent(
    title = "Read JSONL File",
    category = CATEGORY,
    inputs = [PORT_PATH],
    outputs = [PORT_ARRAY, PORT_DOC]
)]
struct ReadJsonlFileModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for ReadJsonlFileModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let path = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("path is not a string".into()))?;
        let path = Path::new(path);

        if !path.exists() {
            return Err(Error::InvalidValue(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        if !path.is_file() {
            return Err(Error::InvalidValue(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        let content = fs::read_to_string(path).map_err(|e| {
            Error::InvalidValue(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        let mut values = Vec::new();
        for line in content.lines() {
            let json = serde_json::from_str::<serde_json::Value>(line).map_err(|e| {
                Error::InvalidValue(format!("Failed to parse JSON from line {}: {}", line, e))
            })?;
            let value = Value::from_json(json)?;
            values.push(value);
        }

        let array_value = Value::array(values.into());
        self.output(ctx.clone(), PORT_ARRAY, array_value.clone())
            .await?;

        let out_doc = Value::object(hashmap! {
            "path".into() => Value::string(path.to_string_lossy().to_string()),
            "value".into() => array_value,
        });
        self.output(ctx, PORT_DOC, out_doc).await
    }
}

// Write JSONL File Module
#[modular_agent(
    title = "Write JSONL File",
    category = CATEGORY,
    inputs = [PORT_VALUE, PORT_DOC],
    outputs = [PORT_UNIT],
    string_config(name = CONFIG_PATH),
)]
struct WriteJsonlFileModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for WriteJsonlFileModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        let (path, value) = if port == PORT_VALUE {
            let path = self.configs()?.get_string(CONFIG_PATH)?;
            (path, value)
        } else if port == PORT_DOC {
            let path = if let Some(path) = value.get_str("path") {
                path.to_string()
            } else {
                self.configs()?.get_string(CONFIG_PATH)?
            };
            let value = value
                .get("value")
                .ok_or_else(|| Error::InvalidValue("Input doc is missing 'value' field".into()))?;
            (path, value.clone())
        } else {
            return Err(Error::InvalidPin(port));
        };

        let path = Path::new(&path);

        // Ensure parent directories exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    Error::InvalidValue(format!("Failed to create parent directories: {}", e))
                })?
            }
        }

        let mut json_lines = Vec::new();
        if let Some(array) = value.as_array() {
            for item in array.iter() {
                json_lines.push(item.to_json().to_string());
            }
        } else {
            json_lines.push(value.to_json().to_string());
        }

        let mut f = fs::File::options()
            .write(true)
            .create(true)
            .open(path)
            .map_err(|e| {
                Error::InvalidValue(format!("Failed to open file {}: {}", path.display(), e))
            })?;
        for line in json_lines {
            writeln!(f, "{}", line).map_err(|e| {
                Error::InvalidValue(format!("Failed to write to file {}: {}", path.display(), e))
            })?;
        }

        self.output(ctx, PORT_UNIT, Value::unit()).await
    }
}

// Append JSONL File Module
#[modular_agent(
    title = "Append JSONL File",
    category = CATEGORY,
    inputs = [PORT_VALUE, PORT_DOC],
    outputs = [PORT_UNIT],
    string_config(name = CONFIG_PATH),
)]
struct AppendJsonlFileModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for AppendJsonlFileModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        let (path, value) = if port == PORT_VALUE {
            let path = self.configs()?.get_string(CONFIG_PATH)?;
            (path, value)
        } else if port == PORT_DOC {
            let path = if let Some(path) = value.get_str("path") {
                path.to_string()
            } else {
                self.configs()?.get_string(CONFIG_PATH)?
            };
            let value = value
                .get("value")
                .ok_or_else(|| Error::InvalidValue("Input doc is missing 'value' field".into()))?;
            (path, value.clone())
        } else {
            return Err(Error::InvalidPin(port));
        };

        let path = Path::new(&path);

        // Ensure parent directories exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    Error::InvalidValue(format!("Failed to create parent directories: {}", e))
                })?
            }
        }

        let mut json_lines = Vec::new();
        if let Some(array) = value.as_array() {
            for item in array.iter() {
                json_lines.push(item.to_json().to_string());
            }
        } else {
            json_lines.push(value.to_json().to_string());
        }

        let mut f = fs::File::options()
            .append(true)
            .create(true)
            .open(path)
            .map_err(|e| {
                Error::InvalidValue(format!("Failed to open file {}: {}", path.display(), e))
            })?;
        for line in json_lines {
            writeln!(f, "{}", line).map_err(|e| {
                Error::InvalidValue(format!("Failed to write to file {}: {}", path.display(), e))
            })?;
        }

        self.output(ctx, PORT_UNIT, Value::unit()).await
    }
}
