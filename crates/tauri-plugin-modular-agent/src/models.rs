use agent_stream_kit::{AgentData};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BoardMessage {
  pub name: String,
  pub data: AgentData,
}
