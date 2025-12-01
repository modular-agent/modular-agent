use std::collections::BTreeMap;

use async_trait::async_trait;

use super::askit::ASKit;
use super::config::AgentConfigs;
use super::context::AgentContext;
use super::error::AgentError;
use super::runtime::runtime;
use super::value::AgentValue;

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
        configs: AgentConfigs,
    },
    Stop,
}

pub struct Pin {
    pub name: String,
    pub value: AgentValue,
}

#[async_trait]
pub trait Agent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError>
    where
        Self: Sized;

    fn askit(&self) -> &ASKit;

    fn id(&self) -> &str;

    fn status(&self) -> &AgentStatus;

    fn def_name(&self) -> &str;

    fn out_pin(&self, name: &str) -> Option<&Pin>;

    fn set_out_pin(&mut self, name: String, value: AgentValue);

    fn configs(&self) -> Result<&AgentConfigs, AgentError>;

    fn set_config(&mut self, key: String, value: AgentValue) -> Result<(), AgentError>;

    fn set_configs(&mut self, configs: AgentConfigs) -> Result<(), AgentError>;

    fn get_global_configs(&self) -> Option<AgentConfigs> {
        self.askit().get_global_configs(self.def_name())
    }

    fn flow_id(&self) -> &str;

    fn set_flow_id(&mut self, flow_id: String);

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
}

pub struct AgentData {
    pub askit: ASKit,

    pub id: String,
    pub status: AgentStatus,
    pub def_name: String,
    pub flow_id: String,
    pub out_pins: Option<BTreeMap<String, Pin>>,
    pub configs: Option<AgentConfigs>,
}

impl AgentData {
    pub fn new(askit: ASKit, id: String, def_name: String, configs: Option<AgentConfigs>) -> Self {
        Self {
            askit,
            id,
            status: AgentStatus::Init,
            def_name,
            flow_id: String::new(),
            out_pins: None,
            configs,
        }
    }
}

pub trait HasAgentData {
    fn data(&self) -> &AgentData;

    fn mut_data(&mut self) -> &mut AgentData;
}

#[async_trait]
pub trait AsAgent: HasAgentData {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError>
    where
        Self: Sized + Send + Sync;

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
impl<T: AsAgent + Send + Sync> Agent for T {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        let mut agent = T::new(askit, id, def_name, configs)?;
        agent.mut_data().status = AgentStatus::Init;
        Ok(agent)
    }

    fn askit(&self) -> &ASKit {
        &self.data().askit
    }

    fn id(&self) -> &str {
        &self.data().id
    }

    fn status(&self) -> &AgentStatus {
        &self.data().status
    }

    fn def_name(&self) -> &str {
        self.data().def_name.as_str()
    }

    fn out_pin(&self, name: &str) -> Option<&Pin> {
        if let Some(out_pins) = &self.data().out_pins {
            return out_pins.get(name);
        }
        None
    }

    fn set_out_pin(&mut self, name: String, value: AgentValue) {
        if let Some(out_pins) = &mut self.mut_data().out_pins {
            out_pins.insert(name.clone(), Pin { name, value });
        } else {
            let mut out_pins = BTreeMap::new();
            out_pins.insert(name.clone(), Pin { name, value });
            self.mut_data().out_pins = Some(out_pins);
        }
    }

    fn configs(&self) -> Result<&AgentConfigs, AgentError> {
        self.data().configs.as_ref().ok_or(AgentError::NoConfig)
    }

    fn set_config(&mut self, key: String, value: AgentValue) -> Result<(), AgentError> {
        if let Some(configs) = &mut self.mut_data().configs {
            configs.set(key.clone(), value.clone());
            self.configs_changed()?;
        }
        Ok(())
    }

    fn set_configs(&mut self, configs: AgentConfigs) -> Result<(), AgentError> {
        self.mut_data().configs = Some(configs);
        self.configs_changed()
    }

    fn flow_id(&self) -> &str {
        &self.data().flow_id
    }

    fn set_flow_id(&mut self, flow_id: String) {
        self.mut_data().flow_id = flow_id.clone();
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
        if let Err(e) = self.process(ctx, pin, value).await {
            self.askit()
                .emit_agent_error(self.id().to_string(), e.to_string());
            return Err(e);
        }
        Ok(())
    }

    fn get_global_configs(&self) -> Option<AgentConfigs> {
        self.askit().get_global_configs(self.def_name())
    }
}

pub fn new_agent_boxed<T: Agent + Send + Sync + 'static>(
    askit: ASKit,
    id: String,
    def_name: String,
    configs: Option<AgentConfigs>,
) -> Result<Box<dyn Agent + Send + Sync>, AgentError> {
    Ok(Box::new(T::new(askit, id, def_name, configs)?))
}

pub fn agent_new(
    askit: ASKit,
    agent_id: String,
    def_name: &str,
    configs: Option<AgentConfigs>,
) -> Result<Box<dyn Agent + Send + Sync>, AgentError> {
    let def;
    {
        let defs = askit.defs.lock().unwrap();
        def = defs
            .get(def_name)
            .ok_or_else(|| AgentError::UnknownDefName(def_name.to_string()))?
            .clone();
    }

    let default_config = def.default_configs.clone();
    let configs = match (default_config, configs) {
        (Some(def_cfg), Some(mut cfg)) => {
            for (k, v) in def_cfg.iter() {
                if !cfg.contains_key(k) {
                    cfg.set(k.clone(), v.value.clone());
                }
            }
            Some(cfg)
        }
        (Some(def_cfg), None) => {
            let mut cfg = AgentConfigs::default();
            for (k, v) in def_cfg.iter() {
                cfg.set(k.clone(), v.value.clone());
            }
            Some(cfg)
        }
        (None, Some(cfg)) => Some(cfg),
        (None, None) => None,
    };

    if let Some(new_boxed) = def.new_boxed {
        return new_boxed(askit, agent_id, def_name.to_string(), configs);
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
