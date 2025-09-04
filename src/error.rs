use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Agent flow {0} already exists")]
    DuplicateFlowName(String),

    #[error("Invalid {0} value in array")]
    InvalidArrayValue(String),

    #[error("{0}: Agent definition \"{1}\" is invalid")]
    InvalidDefinition(String, String),

    #[error("Invalid agent flow name: {0}")]
    InvalidFlowName(String),

    #[error("Invalid {0} value")]
    InvalidValue(String),

    #[error("{0}: Agent definition \"{1}\" is missing")]
    MissingDefinition(String, String),

    #[error("Failed to rename agent flow: {0}")]
    RenameFlowFailed(String),

    #[error("Unknown agent def kind: {0}")]
    UnknownDefKind(String),

    #[error("Unknown agent def name: {0}")]
    UnknownDefName(String),

    #[error("Agent definition \"{0}\" is not implemented")]
    NotImplemented(String),

    #[error("Agent {0} already exists")]
    AgentAlreadyExists(String),

    #[error("Failed to create agent {0}")]
    AgentCreationFailed(String),

    #[error("Agent {0} not found")]
    AgentNotFound(String),

    #[error("Source agent {0} not found")]
    SourceAgentNotFound(String),

    #[error("Source handle is empty")]
    EmptySourceHandle,

    #[error("Target handle is empty")]
    EmptyTargetHandle,

    #[error("Edge already exists")]
    EdgeAlreadyExists,

    #[error("Edge {0} not found")]
    EdgeNotFound(String),

    #[error("Agent flow {0} not found")]
    FlowNotFound(String),

    #[error("Agent {0} definition not found")]
    AgentDefinitionNotFound(String),

    #[error("Agent tx for {0} not found")]
    AgentTxNotFound(String),

    #[error("Failed to send message: {0}")]
    SendMessageFailed(String),

    #[error("Failed to serialize/deserialize: {0}")]
    SerializationError(String),

    #[error("Message sender not initialized")]
    TxNotInitialized,

    #[error("IO error: {0}")]
    IoError(String),

    #[error("JSON parsing error: {0}")]
    JsonParseError(String),

    #[error("Invalid file extension: expected JSON")]
    InvalidFileExtension,

    #[error("Empty file name")]
    EmptyFileName,

    #[error("Failed to get file stem from path")]
    FileSystemError,

    #[error("Configuration error: {0}")]
    InvalidConfig(String),
}
