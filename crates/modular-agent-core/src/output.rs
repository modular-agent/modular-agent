use crate::context::ModuleContext;
use crate::error::Result;
use crate::module::Module;
use crate::value::Value;
use std::future::Future;
use std::pin::Pin;

/// Trait for sending output values and emitting events from modules.
///
/// This trait is automatically implemented for all types that implement `Module`.
/// It provides methods for sending values to output ports and notifying the
/// orchestrator about configuration changes and errors.
pub trait ModuleOutput {
    /// Sends a value to an output port asynchronously (raw version with String port).
    ///
    /// This is the low-level method; prefer using `output()` which accepts
    /// any type that can be converted to String.
    fn output_raw(
        &self,
        ctx: ModuleContext,
        port: String,
        value: Value,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Sends a value to an output port asynchronously.
    ///
    /// The queue is unbounded, so enqueueing always succeeds immediately;
    /// this fails only when the orchestrator has shut down. Use
    /// `try_output` from synchronous contexts.
    fn output<S: Into<String>>(
        &self,
        ctx: ModuleContext,
        port: S,
        value: Value,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        self.output_raw(ctx, port.into(), value)
    }

    /// Sends a value to an output port from a synchronous context (raw version).
    fn try_output_raw(&self, ctx: ModuleContext, port: String, value: Value) -> Result<()>;

    /// Sends a value to an output port from a synchronous context.
    fn try_output<S: Into<String>>(&self, ctx: ModuleContext, port: S, value: Value) -> Result<()> {
        self.try_output_raw(ctx, port.into(), value)
    }

    /// Emits a configuration update event (raw version with String key).
    fn emit_config_updated_raw(&self, key: String, value: Value);

    /// Emits a configuration update event.
    ///
    /// Notifies the orchestrator that a configuration value has changed,
    /// typically used when a module updates its own configuration.
    fn emit_config_updated<S: Into<String>>(&self, key: S, value: Value) {
        self.emit_config_updated_raw(key.into(), value);
    }

    /// Emits a module spec update event (raw version).
    fn emit_module_spec_updated_raw(&self);

    /// Emits a module spec update event.
    ///
    /// Notifies the orchestrator that the module's specification has changed
    /// (e.g., ports were added or removed dynamically).
    fn emit_module_spec_updated(&self) {
        self.emit_module_spec_updated_raw();
    }

    /// Emits an error event (raw version with String message).
    fn emit_error_raw(&self, message: String);

    /// Emits an error event.
    ///
    /// Notifies the orchestrator that an error occurred in this module.
    #[allow(unused)]
    fn emit_error<S: Into<String>>(&self, message: S) {
        self.emit_error_raw(message.into());
    }
}

impl<T: Module> ModuleOutput for T {
    fn output_raw(
        &self,
        ctx: ModuleContext,
        port: String,
        value: Value,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            self.ma()
                .send_module_out(self.id().into(), ctx, port, value)
                .await
        })
    }

    fn try_output_raw(&self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        self.ma()
            .try_send_module_out(self.id().into(), ctx, port, value)
    }

    fn emit_config_updated_raw(&self, key: String, value: Value) {
        self.ma()
            .emit_module_config_updated(self.id().to_string(), key, value);
    }

    fn emit_module_spec_updated_raw(&self) {
        self.ma().emit_module_spec_updated(self.id().to_string());
    }

    fn emit_error_raw(&self, message: String) {
        self.ma().emit_module_error(self.id().to_string(), message);
    }
}
