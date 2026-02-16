use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::config::AgentConfigs;
use crate::context::AgentContext;
use crate::error::AgentError;
use crate::modular_agent::ModularAgent;
use crate::runtime::runtime;
use crate::spec::AgentSpec;
use crate::value::AgentValue;

/// The lifecycle status of an agent.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum AgentStatus {
    #[default]
    Init,
    Start,
    Stop,
}

/// Internal messages sent to agents.
pub(crate) enum AgentMessage {
    /// Input value received on a port.
    Input {
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    },

    /// Configuration value update.
    Config {
        key: String,
        value: AgentValue,
    },

    /// Full configuration update.
    Configs {
        configs: AgentConfigs,
    },

    /// Stop the agent.
    Stop,
}

/// The core trait for all agents.
///
/// All agents implement this trait. Defines lifecycle management,
/// configuration access, and message processing.
#[async_trait]
pub trait Agent: Send + Sync + 'static {
    /// Constructs a new agent instance.
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError>
    where
        Self: Sized;

    /// Returns the `ModularAgent`.
    fn ma(&self) -> &ModularAgent;

    /// Returns the unique agent ID.
    fn id(&self) -> &str;

    /// Returns the current lifecycle status.
    fn status(&self) -> &AgentStatus;

    /// Returns the agent specification.
    fn spec(&self) -> &AgentSpec;

    /// Updates the agent specification.
    fn update_spec(&mut self, spec_update: &Value) -> Result<(), AgentError>;

    /// Returns the agent definition name.
    fn def_name(&self) -> &str;

    /// Returns the agent's configuration.
    ///
    /// # Errors
    ///
    /// Returns `NoConfig` if no configuration is available.
    fn configs(&self) -> Result<&AgentConfigs, AgentError>;

    /// Sets a configuration value.
    fn set_config(&mut self, key: String, value: AgentValue) -> Result<(), AgentError>;

    /// Sets the entire configuration.
    fn set_configs(&mut self, configs: AgentConfigs) -> Result<(), AgentError>;

    /// Gets global configuration for this agent.
    fn get_global_configs(&self) -> Option<AgentConfigs> {
        self.ma().get_global_configs(self.def_name())
    }

    /// Returns the preset ID this agent belongs to.
    fn preset_id(&self) -> &str;

    /// Sets the preset ID.
    fn set_preset_id(&mut self, preset_id: String);

    /// Starts the agent.
    ///
    /// Called when the workflow starts. Use for initialization and initial output.
    async fn start(&mut self) -> Result<(), AgentError>;

    /// Stops the agent.
    async fn stop(&mut self) -> Result<(), AgentError>;

    /// Processes an input message.
    ///
    /// Called when the agent receives a value on an input port.
    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError>;

    /// Returns the tokio runtime.
    fn runtime(&self) -> &tokio::runtime::Runtime {
        runtime()
    }

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl dyn Agent {
    pub fn as_agent<T: Agent>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }

    pub fn as_agent_mut<T: Agent>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut::<T>()
    }
}

/// Core data structure for an agent.
///
/// Used by agents implementing `AsAgent` to store common state.
/// The `#[modular_agent]` macro generates a struct with this as a field.
pub struct AgentData {
    /// The ModularAgent instance.
    pub ma: ModularAgent,

    /// The unique identifier for this agent.
    pub id: String,

    /// The specification of the agent (definition, config, etc.).
    pub spec: AgentSpec,

    /// The preset identifier for the agent.
    /// Empty string when the agent does not belong to any preset.
    pub preset_id: String,

    /// The current lifecycle status of the agent.
    pub status: AgentStatus,
}

impl AgentData {
    /// Creates a new `AgentData` instance.
    ///
    /// Removes any `_`-prefixed config keys that were preserved by
    /// `AgentDefinition::reconcile_spec()` for lazy migration.
    /// Agents can read these keys from the `spec` parameter in `AsAgent::new()`
    /// before calling this method.
    pub fn new(ma: ModularAgent, id: String, mut spec: AgentSpec) -> Self {
        if let Some(ref mut configs) = spec.configs {
            configs.retain(|key, _| !key.starts_with('_'));
        }
        Self {
            ma,
            id,
            spec,
            preset_id: String::new(),
            status: AgentStatus::Init,
        }
    }
}

/// Trait for types that contain `AgentData`.
///
/// Required by `AsAgent`. Usually implemented automatically via `#[modular_agent]` macro.
pub trait HasAgentData {
    fn data(&self) -> &AgentData;

    fn mut_data(&mut self) -> &mut AgentData;
}

/// Simplified trait for implementing custom agents.
///
/// Implement this trait instead of `Agent` directly.
/// The `Agent` trait is automatically implemented for all types that implement `AsAgent`.
#[async_trait]
pub trait AsAgent: HasAgentData + Send + Sync + 'static {
    /// Constructs a new agent instance.
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError>
    where
        Self: Sized;

    /// Called when configuration values change.
    ///
    /// Override to react to configuration changes at runtime.
    fn configs_changed(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called when the agent starts.
    ///
    /// Override for initialization logic or to emit initial values.
    async fn start(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called when the agent stops.
    ///
    /// Override for cleanup logic.
    async fn stop(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    /// Processes an input message.
    ///
    /// Override to implement the agent's main logic.
    async fn process(
        &mut self,
        _ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[async_trait]
impl<T: AsAgent> Agent for T {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let mut agent = T::new(ma, id, spec)?;
        agent.mut_data().status = AgentStatus::Init;
        Ok(agent)
    }

    fn ma(&self) -> &ModularAgent {
        &self.data().ma
    }

    fn id(&self) -> &str {
        &self.data().id
    }

    fn spec(&self) -> &AgentSpec {
        &self.data().spec
    }

    fn update_spec(&mut self, value: &Value) -> Result<(), AgentError> {
        self.mut_data().spec.update(value)
    }

    fn status(&self) -> &AgentStatus {
        &self.data().status
    }

    fn def_name(&self) -> &str {
        self.data().spec.def_name.as_str()
    }

    fn configs(&self) -> Result<&AgentConfigs, AgentError> {
        self.data()
            .spec
            .configs
            .as_ref()
            .ok_or(AgentError::NoConfig)
    }

    fn set_config(&mut self, key: String, value: AgentValue) -> Result<(), AgentError> {
        if let Some(configs) = &mut self.mut_data().spec.configs {
            configs.set(key, value);
            self.configs_changed()?;
        }
        Ok(())
    }

    fn set_configs(&mut self, configs: AgentConfigs) -> Result<(), AgentError> {
        self.mut_data().spec.configs = Some(configs);
        self.configs_changed()
    }

    fn preset_id(&self) -> &str {
        &self.data().preset_id
    }

    fn set_preset_id(&mut self, preset_id: String) {
        self.mut_data().preset_id = preset_id;
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        self.mut_data().status = AgentStatus::Start;

        if let Err(e) = <T as AsAgent>::start(self).await {
            self.ma()
                .emit_agent_error(self.id().to_string(), e.to_string());
            return Err(e);
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.mut_data().status = AgentStatus::Stop;
        <T as AsAgent>::stop(self).await?;
        self.mut_data().status = AgentStatus::Init;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if let Err(e) = <T as AsAgent>::process(self, ctx.clone(), port, value).await {
            self.ma()
                .emit_agent_error(self.id().to_string(), e.to_string());
            self.ma()
                .send_agent_out(
                    self.id().to_string(),
                    ctx,
                    "err".to_string(),
                    AgentValue::Error(Arc::new(e.clone())),
                )
                .await
                .unwrap_or_else(|e| {
                    log::error!("Failed to send error message for {}: {}", self.id(), e);
                });
            return Err(e);
        }
        Ok(())
    }

    fn get_global_configs(&self) -> Option<AgentConfigs> {
        self.ma().get_global_configs(self.def_name())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Creates a boxed agent instance from a concrete type.
#[doc(hidden)]
pub fn new_agent_boxed<T: Agent>(
    ma: ModularAgent,
    id: String,
    spec: AgentSpec,
) -> Result<Box<dyn Agent>, AgentError> {
    Ok(Box::new(T::new(ma, id, spec)?))
}

/// Creates an agent based on its definition.
///
/// Looks up the agent definition by name and calls the appropriate constructor.
pub(crate) fn agent_new(
    ma: ModularAgent,
    agent_id: String,
    mut spec: AgentSpec,
) -> Result<Box<dyn Agent>, AgentError> {
    let def;
    {
        let def_name = &spec.def_name;
        let defs = ma.defs.lock().unwrap();
        def = defs
            .get(def_name)
            .ok_or_else(|| AgentError::UnknownDefName(def_name.to_string()))?
            .clone();
    }

    def.reconcile_spec(&mut spec);

    if let Some(new_boxed) = def.new_boxed {
        return new_boxed(ma, agent_id, spec);
    }

    match def.kind.as_str() {
        // "Command" => {
        //     return new_boxed::<super::builtins::CommandAgent>(
        //         ma,
        //         agent_id,
        //         def_name.to_string(),
        //         config,
        //     );
        // }
        _ => return Err(AgentError::UnknownDefKind(def.kind.to_string()).into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentConfigs;
    use crate::value::AgentValue;

    #[test]
    fn test_agent_data_new_strips_prefixed_keys() {
        let ma = ModularAgent::init().unwrap();
        let mut configs = AgentConfigs::new();
        configs.set("name".into(), AgentValue::string("hello"));
        configs.set("count".into(), AgentValue::integer(10));
        configs.set("_old_key".into(), AgentValue::string("stale"));
        configs.set("_removed".into(), AgentValue::integer(42));

        let spec = AgentSpec {
            configs: Some(configs),
            ..Default::default()
        };

        let data = AgentData::new(ma.clone(), "test_id".into(), spec);

        let c = data.spec.configs.as_ref().unwrap();
        assert_eq!(c.get_string_or_default("name"), "hello");
        assert_eq!(c.get_integer_or_default("count"), 10);
        assert!(c.get("_old_key").is_err());
        assert!(c.get("_removed").is_err());

        ma.quit();
    }
}
