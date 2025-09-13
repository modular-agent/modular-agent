use agent_stream_kit::{
    ASKit, Agent, AgentConfig, AgentConfigEntry, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AgentValue, AgentValueMap, AsAgent, AsAgentData, new_boxed,
};
use async_trait::async_trait;

// Task agent
struct TaskAgent {
    data: AsAgentData,
    last_id: i64,
}

#[async_trait]
impl AsAgent for TaskAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            last_id: 0,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let task_name = self
            .config()
            .ok_or(AgentError::NoConfig)?
            .get(CONFIG_TASK)
            .ok_or(AgentError::InvalidConfig("missing task".into()))?
            .as_str()
            .ok_or(AgentError::InvalidConfig("failed as_str".into()))?
            .to_string();
        if task_name.is_empty() {
            self.try_output(ctx, CH_DATA, data)?;
            return Ok(());
        }

        self.last_id += 1;
        let key = format!("{}:$task:{}", self.flow_name(), task_name);
        let new_ctx = ctx.with_var(key, AgentValue::new_integer(self.last_id));
        self.try_output(new_ctx, CH_DATA, data)?;

        Ok(())
    }
}

// Task Zip agent
struct TaskZipAgent {
    data: AsAgentData,
    n: usize,
    in_channels: Vec<String>,
    keys: Vec<String>,
    input_value: Vec<Option<AgentValue>>,
    current_id: i64,
}

#[async_trait]
impl AsAgent for TaskZipAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        let mut this = Self {
            data: AsAgentData::new(askit, id, def_name, config.clone()),
            n: 0,
            in_channels: Vec::new(),
            keys: Vec::new(),
            input_value: Vec::new(),
            current_id: -1,
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
        let n = config
            .get(CONFIG_N)
            .ok_or(AgentError::NoConfig)?
            .as_i64()
            .ok_or(AgentError::InvalidConfig("failed as_i64".into()))?;
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
            self.in_channels = (0..self.n).map(|i| format!("in{}", i + 1)).collect();
            self.keys = (0..self.n)
                .map(|i| config.get_string_or_default(&format!("key{}", i + 1)))
                .collect();
            self.input_value = vec![None; self.n];
            self.current_id = -1;
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

        let task_name = self
            .config()
            .ok_or(AgentError::NoConfig)?
            .get(CONFIG_TASK)
            .ok_or(AgentError::InvalidConfig("missing task".into()))?
            .as_str()
            .ok_or(AgentError::InvalidConfig("failed as_str".into()))?
            .to_string();

        if !task_name.is_empty() {
            let key = format!("{}:$task:{}", self.flow_name(), task_name);
            let Some(value) = ctx.get_var(key.as_str()) else {
                // value does not have the task key
                return Ok(());
            };
            let Some(task_id) = value.as_i64() else {
                // value is not a number
                return Ok(());
            };
            if task_id != self.current_id {
                self.current_id = task_id;
                for i in 0..self.n {
                    self.input_value[i] = None;
                }
            }
        }

        for i in 0..self.n {
            if ctx.ch() == self.in_channels[i] {
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
        let out_data = AgentData::new_object(map);

        self.try_output(ctx, CH_DATA, out_data)?;

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/Task";

static CH_DATA: &str = "data";
static CH_IN1: &str = "in1";
static CH_IN2: &str = "in2";
static CH_IN3: &str = "in3";
static CH_IN4: &str = "in4";

static CONFIG_TASK: &str = "task";
static CONFIG_KEY1: &str = "key1";
static CONFIG_KEY2: &str = "key2";
static CONFIG_KEY3: &str = "key3";
static CONFIG_KEY4: &str = "key4";
static CONFIG_N: &str = "n";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "std_task", Some(new_boxed::<TaskAgent>))
            .with_title("Task")
            .with_category(CATEGORY)
            .with_inputs(vec![CH_DATA])
            .with_outputs(vec![CH_DATA])
            .with_default_config(vec![(
                CONFIG_TASK.into(),
                AgentConfigEntry::new(AgentValue::new_string(""), "string"),
            )]),
    );

    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "std_task_zip2", Some(new_boxed::<TaskZipAgent>))
            .with_title("Zip2")
            .with_category(CATEGORY)
            .with_inputs(vec![CH_IN1, CH_IN2])
            .with_outputs(vec![CH_DATA])
            .with_default_config(vec![
                (
                    CONFIG_N.into(),
                    AgentConfigEntry::new(AgentValue::new_integer(2), "integer").with_hidden(),
                ),
                (
                    CONFIG_TASK.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
                (
                    CONFIG_KEY1.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
                (
                    CONFIG_KEY2.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
            ]),
    );

    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "std_task_zip3", Some(new_boxed::<TaskZipAgent>))
            .with_title("Zip3")
            .with_category(CATEGORY)
            .with_inputs(vec![CH_IN1, CH_IN2, CH_IN3])
            .with_outputs(vec![CH_DATA])
            .with_default_config(vec![
                (
                    CONFIG_N.into(),
                    AgentConfigEntry::new(AgentValue::new_integer(3), "integer").with_hidden(),
                ),
                (
                    CONFIG_TASK.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
                (
                    CONFIG_KEY1.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
                (
                    CONFIG_KEY2.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
                (
                    CONFIG_KEY3.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
            ]),
    );

    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "std_task_zip4", Some(new_boxed::<TaskZipAgent>))
            .with_title("Zip4")
            .with_category(CATEGORY)
            .with_inputs(vec![CH_IN1, CH_IN2, CH_IN3, CH_IN4])
            .with_outputs(vec![CH_DATA])
            .with_default_config(vec![
                (
                    CONFIG_N.into(),
                    AgentConfigEntry::new(AgentValue::new_integer(4), "integer").with_hidden(),
                ),
                (
                    CONFIG_TASK.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
                (
                    CONFIG_KEY1.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
                (
                    CONFIG_KEY2.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
                (
                    CONFIG_KEY3.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
                (
                    CONFIG_KEY4.into(),
                    AgentConfigEntry::new(AgentValue::new_string(""), "string"),
                ),
            ]),
    );
}
