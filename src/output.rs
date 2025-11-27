use crate::error::AgentError;

use super::agent::Agent;
use super::context::AgentContext;
use super::value::AgentValue;

pub trait AgentOutput {
    fn try_output_raw(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError>;

    fn try_output<S: Into<String>>(
        &mut self,
        ctx: AgentContext,
        pin: S,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.try_output_raw(ctx, pin.into(), value)
    }

    fn emit_display_raw(&self, key: String, value: AgentValue);

    fn emit_display<S: Into<String>>(&self, key: S, value: AgentValue) {
        self.emit_display_raw(key.into(), value);
    }

    fn emit_error_raw(&self, message: String);

    #[allow(unused)]
    fn emit_error<S: Into<String>>(&self, message: S) {
        self.emit_error_raw(message.into());
    }
}

impl<T: Agent> AgentOutput for T {
    fn try_output_raw(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.set_out_pin(pin.clone(), value.clone());
        self.askit()
            .try_send_agent_out(self.id().into(), ctx, pin, value)
    }

    fn emit_display_raw(&self, key: String, value: AgentValue) {
        self.askit()
            .emit_agent_display(self.id().to_string(), key, value);
    }

    fn emit_error_raw(&self, message: String) {
        self.askit()
            .emit_agent_error(self.id().to_string(), message);
    }
}
