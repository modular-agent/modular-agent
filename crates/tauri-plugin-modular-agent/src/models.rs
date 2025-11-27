use agent_stream_kit::AgentValue;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BoardMessage {
  pub name: String,
  pub value: AgentValue,
}
