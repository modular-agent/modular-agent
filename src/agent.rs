use super::askit::ASKit;
use super::config::AgentConfig;
use super::context::AgentContext;
use super::data::AgentData;
use super::error::AgentError;

#[derive(Debug, Default, Clone, PartialEq)]
pub enum AgentStatus {
    #[default]
    Init,
    Start,
    Stop,
}

pub enum AgentMessage {
    Input { ctx: AgentContext, data: AgentData },
    Config { config: AgentConfig },
    Stop,
}

pub trait Agent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError>
    where
        Self: Sized;

    fn askit(&self) -> &ASKit;

    fn id(&self) -> &str;

    fn status(&self) -> &AgentStatus;

    fn def_name(&self) -> &str;

    fn config(&self) -> Option<&AgentConfig>;

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError>;

    fn start(&mut self) -> Result<(), AgentError>;

    fn stop(&mut self) -> Result<(), AgentError>;

    fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError>;
}

pub struct AsAgentData {
    pub askit: ASKit,

    pub id: String,
    pub status: AgentStatus,
    pub def_name: String,
    pub config: Option<AgentConfig>,
}

impl AsAgentData {
    pub fn new(askit: ASKit, id: String, def_name: String, config: Option<AgentConfig>) -> Self {
        Self {
            askit,
            id,
            status: AgentStatus::Init,
            def_name,
            config,
        }
    }
}

pub trait AsAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError>
    where
        Self: Sized;

    fn data(&self) -> &AsAgentData;

    fn mut_data(&mut self) -> &mut AsAgentData;

    fn set_config(&mut self, _config: AgentConfig) -> Result<(), AgentError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    fn process(&mut self, _ctx: AgentContext, _data: AgentData) -> Result<(), AgentError> {
        Ok(())
    }
}

impl<T: AsAgent> Agent for T {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        let mut agent = T::new(askit, id, def_name, config)?;
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

    fn config(&self) -> Option<&AgentConfig> {
        self.data().config.as_ref()
    }

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        self.mut_data().config = Some(config.clone());
        self.set_config(config)
    }

    fn start(&mut self) -> Result<(), AgentError> {
        self.mut_data().status = AgentStatus::Start;

        if let Err(e) = self.start() {
            self.askit()
                .emit_error(self.id().to_string(), e.to_string());
            return Err(e);
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<(), AgentError> {
        self.mut_data().status = AgentStatus::Stop;
        self.stop()?;
        self.mut_data().status = AgentStatus::Init;
        Ok(())
    }

    fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        if let Err(e) = self.process(ctx, data) {
            self.askit()
                .emit_error(self.id().to_string(), e.to_string());
            return Err(e);
        }
        Ok(())
    }
}

pub trait AsyncAgent: Agent + Send + Sync + 'static {}
impl<T: Agent + Send + Sync + 'static> AsyncAgent for T {}

pub fn new_boxed<T: AsyncAgent>(
    askit: ASKit,
    id: String,
    def_name: String,
    config: Option<AgentConfig>,
) -> Result<Box<dyn AsyncAgent>, AgentError> {
    Ok(Box::new(T::new(askit, id, def_name, config)?))
}

pub fn agent_new(
    askit: ASKit,
    agent_id: String,
    def_name: &str,
    config: Option<AgentConfig>,
) -> Result<Box<dyn AsyncAgent>, AgentError> {
    let def;
    {
        let defs = askit.defs.lock().unwrap();
        def = defs
            .get(def_name)
            .ok_or_else(|| AgentError::UnknownDefName(def_name.to_string()))?
            .clone();
    }

    if let Some(new_boxed) = def.new_boxed {
        return new_boxed(askit, agent_id, def_name.to_string(), config);
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
