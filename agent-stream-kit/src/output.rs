use crate::error::AgentError;

use super::agent::Agent;
use super::context::AgentContext;
use super::data::AgentData;

pub trait AgentOutput {
    fn try_output_raw(
        &self,
        ctx: AgentContext,
        port: String,
        data: AgentData,
    ) -> Result<(), AgentError>;

    fn try_output<S: Into<String>>(
        &self,
        ctx: AgentContext,
        port: S,
        data: AgentData,
    ) -> Result<(), AgentError> {
        self.try_output_raw(ctx, port.into(), data)
    }

    fn emit_display_raw(&self, key: String, data: AgentData);

    fn emit_display<S: Into<String>>(&self, key: S, data: AgentData) {
        self.emit_display_raw(key.into(), data);
    }

    fn emit_error_raw(&self, message: String);

    #[allow(unused)]
    fn emit_error<S: Into<String>>(&self, message: S) {
        self.emit_error_raw(message.into());
    }
}

impl<T: Agent> AgentOutput for T {
    fn try_output_raw(
        &self,
        ctx: AgentContext,
        port: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        let new_ctx = ctx.with_port(port);
        self.askit()
            .try_send_agent_out(self.id().into(), new_ctx, data)
    }

    fn emit_display_raw(&self, key: String, data: AgentData) {
        self.askit().emit_display(self.id().to_string(), key, data);
    }

    fn emit_error_raw(&self, message: String) {
        self.askit().emit_error(self.id().to_string(), message);
    }
}
