use crate::agent::Agent;
use crate::context::AgentContext;
use crate::error::AgentError;
use crate::value::AgentValue;
use std::future::Future;
use std::pin::Pin;

pub trait AgentOutput {
    fn output_raw(
        &self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Pin<Box<dyn Future<Output = Result<(), AgentError>> + Send + '_>>;

    fn output<S: Into<String>>(
        &self,
        ctx: AgentContext,
        port: S,
        value: AgentValue,
    ) -> Pin<Box<dyn Future<Output = Result<(), AgentError>> + Send + '_>> {
        self.output_raw(ctx, port.into(), value)
    }

    fn try_output_raw(
        &self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError>;

    fn try_output<S: Into<String>>(
        &self,
        ctx: AgentContext,
        port: S,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.try_output_raw(ctx, port.into(), value)
    }

    fn emit_config_updated_raw(&self, key: String, value: AgentValue);

    fn emit_config_updated<S: Into<String>>(&self, key: S, value: AgentValue) {
        self.emit_config_updated_raw(key.into(), value);
    }

    fn emit_agent_spec_updated_raw(&self);

    fn emit_agent_spec_updated(&self) {
        self.emit_agent_spec_updated_raw();
    }

    fn emit_error_raw(&self, message: String);

    #[allow(unused)]
    fn emit_error<S: Into<String>>(&self, message: S) {
        self.emit_error_raw(message.into());
    }
}

impl<T: Agent> AgentOutput for T {
    fn output_raw(
        &self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Pin<Box<dyn Future<Output = Result<(), AgentError>> + Send + '_>> {
        Box::pin(async move {
            self.ma()
                .send_agent_out(self.id().into(), ctx, port, value)
                .await
        })
    }

    fn try_output_raw(
        &self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.ma()
            .try_send_agent_out(self.id().into(), ctx, port, value)
    }

    fn emit_config_updated_raw(&self, key: String, value: AgentValue) {
        self.ma()
            .emit_agent_config_updated(self.id().to_string(), key, value);
    }

    fn emit_agent_spec_updated_raw(&self) {
        self.ma().emit_agent_spec_updated(self.id().to_string());
    }

    fn emit_error_raw(&self, message: String) {
        self.ma()
            .emit_agent_error(self.id().to_string(), message);
    }
}
