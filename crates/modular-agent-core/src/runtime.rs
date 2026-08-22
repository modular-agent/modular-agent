use std::sync::OnceLock;

use tokio::runtime::Runtime;

use crate::error::AgentError;

// The failure is cached as a Result rather than retried: propagating the error
// from a plain `OnceLock<Runtime>` would need a get()/set() pattern, and the
// `set()` loser's freshly built Runtime would be dropped inside an async
// context, which panics.
static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();

pub fn runtime() -> Result<&'static Runtime, AgentError> {
    RUNTIME
        .get_or_init(|| Runtime::new().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| AgentError::IoError(e.clone()))
}
