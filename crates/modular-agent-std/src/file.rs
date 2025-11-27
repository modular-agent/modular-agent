use std::fs;
use std::path::Path;

use agent_stream_kit::{
    ASKit, AgentConfigs, AgentContext, AgentDefinition, AgentError, AgentOutput, AgentValue,
    AsAgent, AsAgentData, async_trait, new_agent_boxed,
};

// List Files Agent
struct ListFilesAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ListFilesAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
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

        let out_value = AgentValue::array(files);
        self.try_output(ctx, PIN_FILES, out_value)
    }
}

// Read Text File Agent
struct ReadTextFileAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ReadTextFileAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
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
        let out_value = AgentValue::string(content);
        self.try_output(ctx, PIN_TEXT, out_value)
    }
}

// Write Text File Agent
struct WriteTextFileAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for WriteTextFileAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
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

        self.try_output(ctx, PIN_DATA, value)
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/File";

static PIN_PATH: &str = "path";
static PIN_FILES: &str = "files";
static PIN_TEXT: &str = "text";
static PIN_DATA: &str = "data";

pub fn register_agents(askit: &ASKit) {
    // List Files Agent
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_list_files",
            Some(new_agent_boxed::<ListFilesAgent>),
        )
        .title("List Files")
        .category(CATEGORY)
        .inputs(vec![PIN_PATH])
        .outputs(vec![PIN_FILES]),
    );

    // Read Text File Agent
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_read_text_file",
            Some(new_agent_boxed::<ReadTextFileAgent>),
        )
        .title("Read Text File")
        .category(CATEGORY)
        .inputs(vec![PIN_PATH])
        .outputs(vec![PIN_TEXT]),
    );

    // Write Text File Agent
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_write_text_file",
            Some(new_agent_boxed::<WriteTextFileAgent>),
        )
        .title("Write Text File")
        .category(CATEGORY)
        .inputs(vec![PIN_DATA])
        .outputs(vec![PIN_DATA]),
    );
}
