use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;

use crate::askit::ASKit;
use crate::config::AgentConfigs;
use crate::context::AgentContext;
use crate::error::AgentError;
use crate::runtime::runtime;
use crate::spec::AgentSpec;
use crate::value::AgentValue;

#[derive(Debug, Default, Clone, PartialEq)]
pub enum AgentStatus {
    #[default]
    Init,
    Start,
    Stop,
}

pub enum AgentMessage {
    Input {
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    },
    Config {
        key: String,
        value: AgentValue,
    },
    Configs {
        configs: AgentConfigs,
    },
    Stop,
}

/// The core trait for all agents.
#[async_trait]
pub trait Agent: Send + Sync + 'static {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError>
    where
        Self: Sized;

    fn askit(&self) -> &ASKit;

    fn id(&self) -> &str;

    fn status(&self) -> &AgentStatus;

    fn spec(&self) -> &AgentSpec;

    fn def_name(&self) -> &str;

    fn configs(&self) -> Result<&AgentConfigs, AgentError>;

    fn set_config(&mut self, key: String, value: AgentValue) -> Result<(), AgentError>;

    fn set_configs(&mut self, configs: AgentConfigs) -> Result<(), AgentError>;

    fn get_global_configs(&self) -> Option<AgentConfigs> {
        self.askit().get_global_configs(self.def_name())
    }

    fn stream_id(&self) -> &str;

    fn set_stream_id(&mut self, stream_id: String);

    async fn start(&mut self) -> Result<(), AgentError>;

    async fn stop(&mut self) -> Result<(), AgentError>;

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError>;

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

/// The core data structure for an agent.
pub struct AgentData {
    /// The ASKit instance.
    pub askit: ASKit,

    /// The unique identifier for the agent.
    pub id: String,

    /// The specification of the agent.
    pub spec: AgentSpec,

    /// The stream identifier for the agent.
    /// Empty string when the agent does not belong to any stream.
    pub stream_id: String,

    /// The current status of the agent.
    pub status: AgentStatus,
}

impl AgentData {
    pub fn new(askit: ASKit, id: String, spec: AgentSpec) -> Self {
        Self {
            askit,
            id,
            spec,
            stream_id: String::new(),
            status: AgentStatus::Init,
        }
    }
}

pub trait HasAgentData {
    fn data(&self) -> &AgentData;

    fn mut_data(&mut self) -> &mut AgentData;
}

#[async_trait]
pub trait AsAgent: HasAgentData + Send + Sync + 'static {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError>
    where
        Self: Sized;

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[async_trait]
impl<T: AsAgent> Agent for T {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let mut agent = T::new(askit, id, spec)?;
        agent.mut_data().status = AgentStatus::Init;
        Ok(agent)
    }

    fn askit(&self) -> &ASKit {
        &self.data().askit
    }

    fn id(&self) -> &str {
        &self.data().id
    }

    fn spec(&self) -> &AgentSpec {
        &self.data().spec
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
            configs.set(key.clone(), value.clone());
            self.configs_changed()?;
        }
        Ok(())
    }

    fn set_configs(&mut self, configs: AgentConfigs) -> Result<(), AgentError> {
        self.mut_data().spec.configs = Some(configs);
        self.configs_changed()
    }

    fn stream_id(&self) -> &str {
        &self.data().stream_id
    }

    fn set_stream_id(&mut self, stream_id: String) {
        self.mut_data().stream_id = stream_id.clone();
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        self.mut_data().status = AgentStatus::Start;

        if let Err(e) = self.start().await {
            self.askit()
                .emit_agent_error(self.id().to_string(), e.to_string());
            return Err(e);
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.mut_data().status = AgentStatus::Stop;
        self.stop().await?;
        self.mut_data().status = AgentStatus::Init;
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if let Err(e) = self.process(ctx.clone(), pin, value).await {
            self.askit()
                .emit_agent_error(self.id().to_string(), e.to_string());
            self.askit()
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
        self.askit().get_global_configs(self.def_name())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub fn new_agent_boxed<T: Agent>(
    askit: ASKit,
    id: String,
    spec: AgentSpec,
) -> Result<Box<dyn Agent>, AgentError> {
    Ok(Box::new(T::new(askit, id, spec)?))
}

pub fn agent_new(
    askit: ASKit,
    agent_id: String,
    spec: AgentSpec,
) -> Result<Box<dyn Agent>, AgentError> {
    let def;
    {
        let def_name = &spec.def_name;
        let defs = askit.defs.lock().unwrap();
        def = defs
            .get(def_name)
            .ok_or_else(|| AgentError::UnknownDefName(def_name.to_string()))?
            .clone();
    }

    if let Some(new_boxed) = def.new_boxed {
        return new_boxed(askit, agent_id, spec);
    }

    match def.kind.as_str() {
        // "Command" => {
        //     return new_boxed::<super::builtins::CommandAgent>(
        //         askit,
        //         agent_id,
        //         def_name.to_string(),
        //         config,
        //     );
        // }
        _ => return Err(AgentError::UnknownDefKind(def.kind.to_string()).into()),
    }
}
