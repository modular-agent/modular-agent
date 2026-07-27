use crate::agent::Agent;
use crate::context::AgentContext;
use crate::error::AgentError;
use crate::value::AgentValue;
use std::future::Future;
use std::pin::Pin;

/// Trait for sending output values and emitting events from agents.
///
/// This trait is automatically implemented for all types that implement `Agent`.
/// It provides methods for sending values to output ports and notifying the
/// orchestrator about configuration changes and errors.
pub trait AgentOutput {
    /// Sends a value to an output port asynchronously (raw version with String port).
    ///
    /// This is the low-level method; prefer using `output()` which accepts
    /// any type that can be converted to String.
    fn output_raw(
        &self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Pin<Box<dyn Future<Output = Result<(), AgentError>> + Send + '_>>;

    /// Sends a value to an output port asynchronously.
    ///
    /// This method waits until the value is accepted by the channel.
    /// Use `try_output` if you want non-blocking behavior.
    fn output<S: Into<String>>(
        &self,
        ctx: AgentContext,
        port: S,
        value: AgentValue,
    ) -> Pin<Box<dyn Future<Output = Result<(), AgentError>> + Send + '_>> {
        self.output_raw(ctx, port.into(), value)
    }

    /// Tries to send a value to an output port without blocking (raw version).
    ///
    /// Returns immediately, even if the channel is full.
    fn try_output_raw(
        &self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError>;

    /// Tries to send a value to an output port without blocking.
    fn try_output<S: Into<String>>(
        &self,
        ctx: AgentContext,
        port: S,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        self.try_output_raw(ctx, port.into(), value)
    }

    /// Emits a configuration update event (raw version with String key).
    fn emit_config_updated_raw(&self, key: String, value: AgentValue);

    /// Emits a configuration update event.
    ///
    /// Notifies the orchestrator that a configuration value has changed,
    /// typically used when an agent updates its own configuration.
    fn emit_config_updated<S: Into<String>>(&self, key: S, value: AgentValue) {
        self.emit_config_updated_raw(key.into(), value);
    }

    /// Emits an agent spec update event (raw version).
    fn emit_agent_spec_updated_raw(&self);

    /// Emits an agent spec update event.
    ///
    /// Notifies the orchestrator that the agent's specification has changed
    /// (e.g., ports were added or removed dynamically).
    fn emit_agent_spec_updated(&self) {
        self.emit_agent_spec_updated_raw();
    }

    /// Emits an error event (raw version with String message).
    fn emit_error_raw(&self, message: String);

    /// Emits an error event.
    ///
    /// Notifies the orchestrator that an error occurred in this agent.
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
        self.ma().emit_agent_error(self.id().to_string(), message);
    }
}
