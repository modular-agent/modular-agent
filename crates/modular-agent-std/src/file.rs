use std::fs;
use std::path::Path;

use modular_agent_kit::{
    MAK, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    mak_agent, async_trait,
};
use glob::glob;
use im::hashmap;

const CATEGORY: &str = "Std/File";

const PORT_DATA: &str = "data";
const PORT_DOC: &str = "doc";
const PORT_FILES: &str = "files";
const PORT_PATH: &str = "path";
const PORT_STRING: &str = "string";

// Glob Agent
#[mak_agent(
    title = "Glob",
    category = CATEGORY,
    inputs = [PORT_PATH],
    outputs = [PORT_FILES]
)]
struct GlobAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for GlobAgent {
    fn new(mak: MAK, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let pat = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("not a string".to_string()))?;

        let mut files = Vec::new();

        for entry in glob(pat).map_err(|e| {
            AgentError::InvalidValue(format!("Failed to read glob pattern {}: {}", pat, e))
        })? {
            match entry {
                Ok(path) => {
                    files.push(path.to_string_lossy().to_string().into());
                }
                Err(e) => {
                    return Err(AgentError::InvalidValue(format!(
                        "Failed to read glob entry: {}",
                        e
                    )));
                }
            }
        }

        let out_value = AgentValue::array(files.into());
        self.output(ctx, PORT_FILES, out_value).await
    }
}

// List Files Agent
#[mak_agent(
    title = "List Files",
    category = CATEGORY,
    inputs = [PORT_PATH],
    outputs = [PORT_FILES]
)]
struct ListFilesAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ListFilesAgent {
    fn new(mak: MAK, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let path = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("path is not a string".to_string()))?;
        let path = Path::new(path);

        if !path.exists() {
            return Err(AgentError::InvalidValue(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        if !path.is_dir() {
            return Err(AgentError::InvalidValue(format!(
                "Path is not a directory: {}",
                path.display()
            )));
        }

        let mut files = Vec::new();
        let entries = fs::read_dir(path).map_err(|e| {
            AgentError::InvalidValue(format!(
                "Failed to read directory {}: {}",
                path.display(),
                e
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                AgentError::InvalidValue(format!("Failed to read directory entry: {}", e))
            })?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            files.push(file_name.into());
        }

        let out_value = AgentValue::array(files.into());
        self.output(ctx, PORT_FILES, out_value).await
    }
}

// Read Text File Agent
#[mak_agent(
    title = "Read Text File",
    category = CATEGORY,
    inputs = [PORT_PATH],
    outputs = [PORT_STRING, PORT_DOC]
)]
struct ReadTextFileAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ReadTextFileAgent {
    fn new(mak: MAK, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let path = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("path is not a string".into()))?;
        let path = Path::new(path);

        if !path.exists() {
            return Err(AgentError::InvalidValue(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        if !path.is_file() {
            return Err(AgentError::InvalidValue(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        let content = fs::read_to_string(path).map_err(|e| {
            AgentError::InvalidValue(format!("Failed to read file {}: {}", path.display(), e))
        })?;

        let text = AgentValue::string(content);
        self.output(ctx.clone(), PORT_STRING, text.clone()).await?;

        let out_doc = AgentValue::object(hashmap! {
            "path".into() => AgentValue::string(path.to_string_lossy().to_string()),
            "text".into() => text,
        });
        self.output(ctx, PORT_DOC, out_doc).await
    }
}

// Write Text File Agent
#[mak_agent(
    title = "Write Text File",
    category = CATEGORY,
    inputs = [PORT_DATA],
    outputs = [PORT_DATA]
)]
struct WriteTextFileAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for WriteTextFileAgent {
    fn new(mak: MAK, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let input = value
            .as_object()
            .ok_or_else(|| AgentError::InvalidValue("Input is not an object".into()))?;

        let path = input
            .get("path")
            .ok_or_else(|| AgentError::InvalidValue("Missing 'path' in input".into()))?
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("'path' is not a string".into()))?;

        let text = input
            .get("text")
            .ok_or_else(|| AgentError::InvalidValue("Missing 'text' in input".into()))?
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("'text' is not a string".into()))?;

        let path = Path::new(path);

        // Ensure parent directories exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to create parent directories: {}", e))
                })?
            }
        }

        fs::write(path, text).map_err(|e| {
            AgentError::InvalidValue(format!("Failed to write file {}: {}", path.display(), e))
        })?;

        self.output(ctx, PORT_DATA, value).await
    }
}
