use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentDefinition, AgentError, AgentOutput, AgentValue,
    AgentValueMap, AsAgent, AsAgentData, async_trait, new_agent_boxed,
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
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config.clone()),
            n: 0,
            in_ports: Vec::new(),
            keys: Vec::new(),
            input_value: Vec::new(),
            current_id: 0,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let n = self.configs()?.get_integer(CONFIG_N)?;
        if n <= 1 {
            return Err(AgentError::InvalidConfig("n must be greater than 1".into()));
        }
        let n = n as usize;
        if self.n == n {
            self.keys = (0..self.n)
                .map(|i| {
                    self.configs()
                        .and_then(|c| c.get_string(&format!("key{}", i + 1)))
                        .unwrap_or_default()
                })
                .collect();
        } else {
            self.n = n;
            self.in_ports = (0..self.n).map(|i| format!("in{}", i + 1)).collect();
            self.keys = (0..self.n)
                .map(|i| {
                    self.configs()
                        .and_then(|c| c.get_string(&format!("key{}", i + 1)))
                        .unwrap_or_default()
                })
                .collect();
            self.input_value = vec![None; self.n];
            self.current_id = 0;
        }
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
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
            if pin == self.in_ports[i] {
                self.input_value[i] = Some(value.clone());
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
        let out_value = AgentValue::object(map);

        self.try_output(ctx, PIN_DATA, out_value)?;

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/Stream";

static PIN_DATA: &str = "data";
static PIN_IN1: &str = "in1";
static PIN_IN2: &str = "in2";
static PIN_IN3: &str = "in3";
static PIN_IN4: &str = "in4";

static CONFIG_KEY1: &str = "key1";
static CONFIG_KEY2: &str = "key2";
static CONFIG_KEY3: &str = "key3";
static CONFIG_KEY4: &str = "key4";
static CONFIG_N: &str = "n";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "std_zip2", Some(new_agent_boxed::<ZipAgent>))
            .title("Zip2")
            .category(CATEGORY)
            .inputs(vec![PIN_IN1, PIN_IN2])
            .outputs(vec![PIN_DATA])
            .integer_config_with(CONFIG_N, 2, |entry| entry.hidden())
            .string_config_default(CONFIG_KEY1)
            .string_config_default(CONFIG_KEY2),
    );

    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "std_zip3", Some(new_agent_boxed::<ZipAgent>))
            .title("Zip3")
            .category(CATEGORY)
            .inputs(vec![PIN_IN1, PIN_IN2, PIN_IN3])
            .outputs(vec![PIN_DATA])
            .integer_config_with(CONFIG_N, 3, |entry| entry.hidden())
            .string_config_default(CONFIG_KEY1)
            .string_config_default(CONFIG_KEY2)
            .string_config_default(CONFIG_KEY3),
    );

    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "std_zip4", Some(new_agent_boxed::<ZipAgent>))
            .title("Zip4")
            .category(CATEGORY)
            .inputs(vec![PIN_IN1, PIN_IN2, PIN_IN3, PIN_IN4])
            .outputs(vec![PIN_DATA])
            .integer_config_with(CONFIG_N, 4, |entry| entry.hidden())
            .string_config_default(CONFIG_KEY1)
            .string_config_default(CONFIG_KEY2)
            .string_config_default(CONFIG_KEY3)
            .string_config_default(CONFIG_KEY4),
    );
}
