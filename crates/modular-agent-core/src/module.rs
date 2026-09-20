use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::config::ModuleConfigs;
use crate::context::ModuleContext;
use crate::error::{Error, Result};
use crate::modular_agent::ModularAgent;
use crate::runtime::runtime;
use crate::spec::ModuleSpec;
use crate::value::Value;

/// The lifecycle status of a module.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum ModuleStatus {
    #[default]
    Init,
    Start,
    Stop,
}

/// Internal messages sent to modules.
pub(crate) enum ModuleMessage {
    /// Input value received on a port.
    Input {
        ctx: ModuleContext,
        port: String,
        value: Value,
    },

    /// Configuration value update.
    Config { key: String, value: Value },

    /// Full configuration update.
    Configs { configs: ModuleConfigs },

    /// Stop the module.
    Stop,
}

/// The core trait for all modules.
///
/// All modules implement this trait. Defines lifecycle management,
/// configuration access, and message processing.
#[async_trait]
pub trait Module: Send + Sync + 'static {
    /// Constructs a new module instance.
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self>
    where
        Self: Sized;

    /// Returns the `ModularAgent`.
    fn ma(&self) -> &ModularAgent;

    /// Returns the unique module ID.
    fn id(&self) -> &str;

    /// Returns the current lifecycle status.
    fn status(&self) -> &ModuleStatus;

    /// Returns the module specification.
    fn spec(&self) -> &ModuleSpec;

    /// Updates the module specification.
    fn update_spec(&mut self, spec_update: &JsonValue) -> Result<()>;

    /// Returns the module definition name.
    fn def_name(&self) -> &str;

    /// Returns the module's configuration.
    ///
    /// # Errors
    ///
    /// Returns `NoConfig` if no configuration is available.
    fn configs(&self) -> Result<&ModuleConfigs>;

    /// Sets a configuration value.
    fn set_config(&mut self, key: String, value: Value) -> Result<()>;

    /// Sets the entire configuration.
    fn set_configs(&mut self, configs: ModuleConfigs) -> Result<()>;

    /// Gets global configuration for this module.
    fn get_global_configs(&self) -> Option<ModuleConfigs> {
        self.ma().get_global_configs(self.def_name())
    }

    /// Returns the patch ID this module belongs to.
    fn patch_id(&self) -> &str;

    /// Sets the patch ID.
    fn set_patch_id(&mut self, patch_id: String);

    /// Starts the module.
    ///
    /// Called when the workflow starts. Use for initialization and initial output.
    async fn start(&mut self) -> Result<()>;

    /// Stops the module.
    async fn stop(&mut self) -> Result<()>;

    /// Processes an input message.
    ///
    /// Called when the module receives a value on an input port.
    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()>;

    /// Returns the shared tokio runtime, or an error if it could not be created.
    fn runtime(&self) -> Result<&tokio::runtime::Runtime> {
        runtime()
    }

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl dyn Module {
    pub fn as_module<T: Module>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }

    pub fn as_module_mut<T: Module>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut::<T>()
    }
}

/// Core data structure for a module.
///
/// Used by modules implementing `AsModule` to store common state.
/// The `#[modular_agent]` macro generates a struct with this as a field.
pub struct ModuleData {
    /// The ModularAgent instance.
    pub ma: ModularAgent,

    /// The unique identifier for this module.
    pub id: String,

    /// The specification of the module (definition, config, etc.).
    pub spec: ModuleSpec,

    /// The patch identifier for the module.
    /// Empty string when the module does not belong to any patch.
    pub patch_id: String,

    /// The current lifecycle status of the module.
    pub status: ModuleStatus,
}

impl ModuleData {
    /// Creates a new `ModuleData` instance.
    ///
    /// Removes any `_`-prefixed config keys that were preserved by
    /// `ModuleDefinition::reconcile_spec()` for lazy migration.
    /// Modules can read these keys from the `spec` parameter in `AsModule::new()`
    /// before calling this method.
    pub fn new(ma: ModularAgent, id: String, mut spec: ModuleSpec) -> Self {
        if let Some(ref mut configs) = spec.configs {
            configs.retain(|key, _| !key.starts_with('_'));
        }
        Self {
            ma,
            id,
            spec,
            patch_id: String::new(),
            status: ModuleStatus::Init,
        }
    }
}

/// Trait for types that contain `ModuleData`.
///
/// Required by `AsModule`. Usually implemented automatically via `#[modular_agent]` macro.
pub trait HasModuleData {
    fn data(&self) -> &ModuleData;

    fn mut_data(&mut self) -> &mut ModuleData;
}

/// Simplified trait for implementing custom modules.
///
/// Implement this trait instead of `Module` directly.
/// The `Module` trait is automatically implemented for all types that implement `AsModule`.
///
/// # Cancellation safety
///
/// The module loop races [`process()`](Self::process) against the module's
/// cancellation token, which fires when the module (or its whole patch) is
/// stopped. On cancellation the in-flight `process()` future is **dropped at
/// whatever await point it has reached** — implementations must not rely on
/// running to completion. In particular, outputs emitted before the drop
/// stay emitted, and internal bookkeeping updated across await points (e.g.
/// entries in a pending map) may be left behind; keep such state consistent
/// at every await point or clean it up in [`stop()`](Self::stop).
///
/// Flow-level aborts ([`ModularAgent::abort_context`](crate::ModularAgent::abort_context))
/// are cooperative: the context's token fires, but messages carrying it are
/// still delivered so that wind-down outputs (e.g. an aborted final message
/// replacing a dangling partial in history) can traverse the graph.
/// Implementations that initiate external work (network requests, DB writes,
/// message posts) must therefore check
/// [`ctx.is_cancelled()`](crate::ModuleContext::is_cancelled) before starting
/// it, and may select on
/// [`ctx.cancel_token()`](crate::ModuleContext::cancel_token) at long awaits
/// to wind down gracefully.
#[async_trait]
pub trait AsModule: HasModuleData + Send + Sync + 'static {
    /// Constructs a new module instance.
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self>
    where
        Self: Sized;

    /// Called when configuration values change.
    ///
    /// Override to react to configuration changes at runtime.
    fn configs_changed(&mut self) -> Result<()> {
        Ok(())
    }

    /// Called when the module starts.
    ///
    /// Override for initialization logic or to emit initial values.
    ///
    /// A `start()` that returns `Err` must release whatever it acquired
    /// before failing: the module goes back to `Init` and `stop()` is not
    /// called for a failed start.
    async fn start(&mut self) -> Result<()> {
        Ok(())
    }

    /// Called when the module stops.
    ///
    /// Override for cleanup logic.
    async fn stop(&mut self) -> Result<()> {
        Ok(())
    }

    /// Processes an input message.
    ///
    /// Override to implement the module's main logic.
    ///
    /// This method may be cancelled by being dropped at any await point (see
    /// the [trait-level docs](AsModule#cancellation-safety)). Long-running
    /// implementations can additionally observe
    /// [`ModuleContext::cancel_token`] to abort gracefully when the flow is
    /// cancelled via [`ModularAgent::abort_context`], recording the
    /// interruption as [`Error::Cancelled`].
    async fn process(&mut self, _ctx: ModuleContext, _port: String, _value: Value) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl<T: AsModule> Module for T {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        let mut module = T::new(ma, id, spec)?;
        module.mut_data().status = ModuleStatus::Init;
        Ok(module)
    }

    fn ma(&self) -> &ModularAgent {
        &self.data().ma
    }

    fn id(&self) -> &str {
        &self.data().id
    }

    fn spec(&self) -> &ModuleSpec {
        &self.data().spec
    }

    fn update_spec(&mut self, value: &JsonValue) -> Result<()> {
        self.mut_data().spec.update(value)?;
        // A config patch must reach the module the same way `set_configs`
        // delivers one: modules that derive ports or further configs from
        // their config values only rebuild them in `configs_changed`.
        if value
            .as_object()
            .is_some_and(|map| map.contains_key("configs"))
        {
            self.configs_changed()?;
        }
        Ok(())
    }

    fn status(&self) -> &ModuleStatus {
        &self.data().status
    }

    fn def_name(&self) -> &str {
        self.data().spec.def_name.as_str()
    }

    fn configs(&self) -> Result<&ModuleConfigs> {
        self.data().spec.configs.as_ref().ok_or(Error::NoConfig)
    }

    fn set_config(&mut self, key: String, value: Value) -> Result<()> {
        if let Some(configs) = &mut self.mut_data().spec.configs {
            configs.set(key, value);
            self.configs_changed()?;
        }
        Ok(())
    }

    fn set_configs(&mut self, configs: ModuleConfigs) -> Result<()> {
        // Merge instead of replacing, for the same reason `ModuleSpec::update`
        // merges config patches: a partial update must not drop untouched
        // keys, or values a module only emitted (never persisted) would be
        // clobbered and dynamic configs rebuilt from defaults.
        let data = self.mut_data();
        match data.spec.configs.as_mut() {
            Some(existing) => {
                for (key, value) in configs {
                    existing.set(key, value);
                }
            }
            None => data.spec.configs = Some(configs),
        }
        self.configs_changed()
    }

    fn patch_id(&self) -> &str {
        &self.data().patch_id
    }

    fn set_patch_id(&mut self, patch_id: String) {
        self.mut_data().patch_id = patch_id;
    }

    async fn start(&mut self) -> Result<()> {
        self.mut_data().status = ModuleStatus::Start;

        if let Err(e) = <T as AsModule>::start(self).await {
            // A failed start leaves no running module behind; report it as
            // never started so stop_module skips stop() and configs are
            // written directly.
            self.mut_data().status = ModuleStatus::Init;
            self.ma()
                .emit_module_error(self.id().to_string(), e.to_string());
            return Err(e);
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.mut_data().status = ModuleStatus::Stop;
        <T as AsModule>::stop(self).await?;
        self.mut_data().status = ModuleStatus::Init;
        Ok(())
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        if let Err(e) = <T as AsModule>::process(self, ctx.clone(), port, value).await {
            self.ma()
                .emit_module_error(self.id().to_string(), e.to_string());
            self.ma()
                .send_module_out(
                    self.id().to_string(),
                    ctx,
                    "err".to_string(),
                    Value::Error(Arc::new(e.clone())),
                )
                .await
                .unwrap_or_else(|e| {
                    log::error!("Failed to send error message for {}: {}", self.id(), e);
                });
            return Err(e);
        }
        Ok(())
    }

    fn get_global_configs(&self) -> Option<ModuleConfigs> {
        self.ma().get_global_configs(self.def_name())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Creates a boxed module instance from a concrete type.
#[doc(hidden)]
pub fn new_module_boxed<T: Module>(
    ma: ModularAgent,
    id: String,
    spec: ModuleSpec,
) -> Result<Box<dyn Module>> {
    Ok(Box::new(T::new(ma, id, spec)?))
}

/// Creates a module based on its definition.
///
/// Looks up the module definition by name and calls the appropriate constructor.
pub(crate) fn module_new(
    ma: ModularAgent,
    module_id: String,
    mut spec: ModuleSpec,
) -> Result<Box<dyn Module>> {
    let def;
    {
        let def_name = &spec.def_name;
        let defs = ma.defs.lock();
        def = defs
            .get(def_name)
            .ok_or_else(|| Error::UnknownDefName(def_name.to_string()))?
            .clone();
    }

    def.reconcile_spec(&mut spec);

    if let Some(new_boxed) = def.new_boxed {
        return new_boxed(ma, module_id, spec);
    }

    Err(Error::UnknownDefKind(def.kind.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModuleConfigs;
    use crate::value::Value;

    #[test]
    fn test_module_data_new_strips_prefixed_keys() {
        let ma = ModularAgent::init().unwrap();
        let mut configs = ModuleConfigs::new();
        configs.set("name".into(), Value::string("hello"));
        configs.set("count".into(), Value::integer(10));
        configs.set("_old_key".into(), Value::string("stale"));
        configs.set("_removed".into(), Value::integer(42));

        let spec = ModuleSpec {
            configs: Some(configs),
            ..Default::default()
        };

        let data = ModuleData::new(ma.clone(), "test_id".into(), spec);

        let c = data.spec.configs.as_ref().unwrap();
        assert_eq!(c.get_string_or_default("name"), "hello");
        assert_eq!(c.get_integer_or_default("count"), 10);
        assert!(c.get("_old_key").is_err());
        assert!(c.get("_removed").is_err());

        ma.quit();
    }
}
