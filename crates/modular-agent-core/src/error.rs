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
/// - **Retryable / provider errors**: `RateLimited`, `Overloaded`, `Timeout`, `ContextOverflow`, `Cancelled`
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

    /// Invalid patch name.
    #[error("Invalid patch name: {0}")]
    InvalidPatchName(String),

    /// Invalid value for the expected type.
    #[error("Invalid {0} value")]
    InvalidValue(String),

    /// Agent definition is missing a required field.
    #[error("{0}: Agent definition \"{1}\" is missing")]
    MissingDefinition(String, String),

    /// Failed to rename a patch.
    #[error("Failed to rename patch: {0}")]
    RenamePatchFailed(String),

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

    /// Patch with the specified name was not found.
    #[error("Patch {0} not found")]
    PatchNotFound(String),

    /// A patch with this name already exists.
    #[error("Patch name \"{0}\" already exists")]
    PatchNameExists(String),

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

    /// Request was rejected because the provider rate limit was exceeded.
    ///
    /// `retry_after` carries the provider's suggested wait duration when a
    /// `Retry-After` header is present.
    #[error("Rate limited: {message}")]
    RateLimited {
        message: String,
        retry_after: Option<std::time::Duration>,
    },

    /// Provider is temporarily overloaded (e.g. HTTP 529).
    #[error("Provider overloaded: {0}")]
    Overloaded(String),

    /// Request did not complete within the allotted time.
    #[error("Request timed out: {0}")]
    Timeout(String),

    /// Request exceeded the model's context window.
    #[error("Context overflow: {0}")]
    ContextOverflow(String),

    /// Operation was cancelled before completion.
    #[error("Cancelled")]
    Cancelled,

    /// [`ModularAgent::shutdown`](crate::ModularAgent::shutdown) did not
    /// finish within the given duration.
    #[error("Shutdown timed out after {0:?}")]
    ShutdownTimeout(std::time::Duration),

    /// Generic agent error.
    #[error("Agent error: {0}")]
    Other(String),
}

impl AgentError {
    /// Returns `true` for errors that are transient and may succeed on retry.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. } | Self::Overloaded(_) | Self::Timeout(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_variants_are_retryable() {
        assert!(
            AgentError::RateLimited {
                message: "slow down".into(),
                retry_after: None,
            }
            .is_retryable()
        );
        assert!(AgentError::Overloaded("busy".into()).is_retryable());
        assert!(AgentError::Timeout("deadline exceeded".into()).is_retryable());
    }

    #[test]
    fn non_retryable_variants_are_not_retryable() {
        assert!(!AgentError::ContextOverflow("too long".into()).is_retryable());
        assert!(!AgentError::Cancelled.is_retryable());
        assert!(!AgentError::InvalidValue("bad".into()).is_retryable());
        assert!(!AgentError::IoError("disk full".into()).is_retryable());
    }
}
