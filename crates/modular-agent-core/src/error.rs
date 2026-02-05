use thiserror::Error;

/// Errors that occur during agent operations.
///
/// Errors are categorized into:
///
/// - **Configuration errors**: `InvalidConfig`, `UnknownConfig`, `NoConfig`
/// - **Value errors**: `InvalidValue`, `InvalidArrayValue`
/// - **Agent management errors**: `AgentNotFound`, `AgentAlreadyExists`
/// - **Connection errors**: `ConnectionNotFound`, `ConnectionAlreadyExists`
/// - **I/O errors**: `IoError`, `SerializationError`, `JsonParseError`
///
/// - **設定エラー**: `InvalidConfig`, `UnknownConfig`, `NoConfig`
/// - **値エラー**: `InvalidValue`, `InvalidArrayValue`
/// - **エージェント管理エラー**: `AgentNotFound`, `AgentAlreadyExists`
/// - **接続エラー**: `ConnectionNotFound`, `ConnectionAlreadyExists`
/// - **I/Oエラー**: `IoError`, `SerializationError`, `JsonParseError`
#[derive(Clone, Debug, Error)]
pub enum AgentError {
    /// Invalid value in an array element.
    #[error("Invalid {0} value in array")]
    InvalidArrayValue(String),

    /// Agent definition is invalid.
    #[error("{0}: Agent definition \"{1}\" is invalid")]
    InvalidDefinition(String, String),

    /// Invalid port name.
    #[error("Invalid port: {0}")]
    InvalidPin(String),

    /// Invalid preset name.
    #[error("Invalid preset name: {0}")]
    InvalidPresetName(String),

    /// Invalid value for the expected type.
    #[error("Invalid {0} value")]
    InvalidValue(String),

    /// Agent definition is missing a required field.
    #[error("{0}: Agent definition \"{1}\" is missing")]
    MissingDefinition(String, String),

    /// Failed to rename a preset.
    #[error("Failed to rename preset: {0}")]
    RenamePresetFailed(String),

    /// Unknown agent definition kind.
    #[error("Unknown agent def kind: {0}")]
    UnknownDefKind(String),

    /// Unknown agent definition name.
    #[error("Unknown agent def name: {0}")]
    UnknownDefName(String),

    /// Agent definition is not implemented.
    #[error("Agent definition \"{0}\" is not implemented")]
    NotImplemented(String),

    /// An agent with this ID already exists.
    #[error("Agent {0} already exists")]
    AgentAlreadyExists(String),

    /// Failed to create an agent.
    #[error("Failed to create agent {0}")]
    AgentCreationFailed(String),

    /// Agent with the specified ID was not found.
    #[error("Agent {0} not found")]
    AgentNotFound(String),

    /// Source agent in a connection was not found.
    #[error("Source agent {0} not found")]
    SourceAgentNotFound(String),

    /// Duplicate ID detected.
    #[error("Duplicate id: {0}")]
    DuplicateId(String),

    /// Connection source handle is empty.
    #[error("Source handle is empty")]
    EmptySourceHandle,

    /// Connection target handle is empty.
    #[error("Target handle is empty")]
    EmptyTargetHandle,

    /// A connection between these ports already exists.
    #[error("Connection already exists")]
    ConnectionAlreadyExists,

    /// Connection with the specified ID was not found.
    #[error("Connection {0} not found")]
    ConnectionNotFound(String),

    /// Preset with the specified name was not found.
    #[error("Preset {0} not found")]
    PresetNotFound(String),

    /// Agent definition was not found.
    #[error("Agent {0} definition not found")]
    AgentDefinitionNotFound(String),

    /// Agent message sender was not found.
    #[error("Agent tx for {0} not found")]
    AgentTxNotFound(String),

    /// Failed to send a message to an agent.
    #[error("Failed to send message: {0}")]
    SendMessageFailed(String),

    /// Serialization or deserialization error.
    #[error("Failed to serialize/deserialize: {0}")]
    SerializationError(String),

    /// Message sender is not initialized.
    #[error("Message sender not initialized")]
    TxNotInitialized,

    /// I/O error.
    #[error("IO error: {0}")]
    IoError(String),

    /// JSON parsing error.
    #[error("JSON parsing error: {0}")]
    JsonParseError(String),

    /// Invalid file extension (expected JSON).
    #[error("Invalid file extension: expected JSON")]
    InvalidFileExtension,

    /// File name is empty.
    #[error("Empty file name")]
    EmptyFileName,

    /// Failed to get file stem from path.
    #[error("Failed to get file stem from path")]
    FileSystemError,

    /// Invalid configuration value.
    #[error("Configuration error: {0}")]
    InvalidConfig(String),

    /// No configuration is available for this agent.
    #[error("No configuration available")]
    NoConfig,

    /// Configuration key does not exist.
    #[error("Unknown configuration: {0}")]
    UnknownConfig(String),

    /// No global configuration is available.
    #[error("No global configuration available")]
    NoGlobalConfig,

    /// Port (pin) was not found.
    #[error("Pin not found: {0}")]
    PinNotFound(String),

    /// Generic agent error.
    #[error("Agent error: {0}")]
    Other(String),
}
