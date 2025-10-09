use agent_stream_kit::{
    ASKit, AgentConfig, AgentConfigEntry, AgentContext, AgentData, AgentDefinition, AgentError,
    AgentOutput, AgentValue, AgentValueMap, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};

// Zip agent
struct ZipAgent {
    data: AsAgentData,
    n: usize,
    in_ports: Vec<String>,
    keys: Vec<String>,
    input_value: Vec<Option<AgentValue>>,
    current_id: usize,
}

#[async_trait]
impl AsAgent for ZipAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        let mut this = Self {
            data: AsAgentData::new(askit, id, def_name, config.clone()),
            n: 0,
            in_ports: Vec::new(),
            keys: Vec::new(),
            input_value: Vec::new(),
            current_id: 0,
        };
        if let Some(c) = config {
            AsAgent::set_config(&mut this, c)?;
        } else {
            return Err(AgentError::InvalidConfig("missing config".into()));
        }
        Ok(this)
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        let n = config.get_integer(CONFIG_N)?;
        if n <= 1 {
            return Err(AgentError::InvalidConfig("n must be greater than 1".into()));
        }
        let n = n as usize;
        if self.n == n {
            self.keys = (0..self.n)
                .map(|i| config.get_string_or_default(&format!("key{}", i + 1)))
                .collect();
        } else {
            self.n = n;
            self.in_ports = (0..self.n).map(|i| format!("in{}", i + 1)).collect();
            self.keys = (0..self.n)
                .map(|i| config.get_string_or_default(&format!("key{}", i + 1)))
                .collect();
            self.input_value = vec![None; self.n];
            self.current_id = 0;
        }
        Ok(())
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        for i in 0..self.n {
            if self.keys[i].is_empty() {
                return Err(AgentError::InvalidConfig(format!(
                    "key{} is not set",
                    i + 1
                )));
            }
        }

        // Reset input values if context ID changes
        let ctx_id = ctx.id();
        if ctx_id != self.current_id {
            self.current_id = ctx_id;
            for i in 0..self.n {
                self.input_value[i] = None;
            }
        }

        // Store the input value
        for i in 0..self.n {
            if ctx.port() == self.in_ports[i] {
                self.input_value[i] = Some(data.value.clone());
            }
        }

        // Check if all inputs are present
        for i in 0..self.n {
            if self.input_value[i].is_none() {
                return Ok(());
            }
        }

        // All inputs are present, create the output
        let mut map = AgentValueMap::new();
        for i in 0..self.n {
            let key = self.keys[i].clone();
            let value = self.input_value[i].take().unwrap();
            map.insert(key, value);
        }
        let out_data = AgentData::object(map);

        self.try_output(ctx, PORT_DATA, out_data)?;

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/Stream";

static PORT_DATA: &str = "data";
static PORT_IN1: &str = "in1";
static PORT_IN2: &str = "in2";
static PORT_IN3: &str = "in3";
static PORT_IN4: &str = "in4";

static CONFIG_KEY1: &str = "key1";
static CONFIG_KEY2: &str = "key2";
static CONFIG_KEY3: &str = "key3";
static CONFIG_KEY4: &str = "key4";
static CONFIG_N: &str = "n";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "std_zip2", Some(new_agent_boxed::<ZipAgent>))
            .with_title("Zip2")
            .with_category(CATEGORY)
            .with_inputs(vec![PORT_IN1, PORT_IN2])
            .with_outputs(vec![PORT_DATA])
            .with_default_config(vec![
                (CONFIG_N, AgentConfigEntry::new(2, "integer").with_hidden()),
                (CONFIG_KEY1, AgentConfigEntry::new("", "string")),
                (CONFIG_KEY2, AgentConfigEntry::new("", "string")),
            ]),
    );

    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "std_zip3", Some(new_agent_boxed::<ZipAgent>))
            .with_title("Zip3")
            .with_category(CATEGORY)
            .with_inputs(vec![PORT_IN1, PORT_IN2, PORT_IN3])
            .with_outputs(vec![PORT_DATA])
            .with_default_config(vec![
                (CONFIG_N, AgentConfigEntry::new(3, "integer").with_hidden()),
                (CONFIG_KEY1, AgentConfigEntry::new("", "string")),
                (CONFIG_KEY2, AgentConfigEntry::new("", "string")),
                (CONFIG_KEY3, AgentConfigEntry::new("", "string")),
            ]),
    );

    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "std_zip4", Some(new_agent_boxed::<ZipAgent>))
            .with_title("Zip4")
            .with_category(CATEGORY)
            .with_inputs(vec![PORT_IN1, PORT_IN2, PORT_IN3, PORT_IN4])
            .with_outputs(vec![PORT_DATA])
            .with_default_config(vec![
                (CONFIG_N, AgentConfigEntry::new(4, "integer").with_hidden()),
                (CONFIG_KEY1, AgentConfigEntry::new("", "string")),
                (CONFIG_KEY2, AgentConfigEntry::new("", "string")),
                (CONFIG_KEY3, AgentConfigEntry::new("", "string")),
                (CONFIG_KEY4, AgentConfigEntry::new("", "string")),
            ]),
    );
}
