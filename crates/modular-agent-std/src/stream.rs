use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentError, AgentOutput, AgentValue, AgentValueMap,
    AsAgent, AsAgentData, async_trait,
};
use askit_macros::askit_agent;

struct ZipAgent {
    data: AsAgentData,
    n: usize,
    in_ports: Vec<String>,
    keys: Vec<String>,
    input_value: Vec<Option<AgentValue>>,
    current_id: usize,
}

impl ZipAgent {
    fn new_with_n(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
        n: usize,
    ) -> Result<Self, AgentError> {
        let mut agent = Self {
            data: AsAgentData::new(askit, id, def_name, config),
            n,
            in_ports: Vec::new(),
            keys: Vec::new(),
            input_value: Vec::new(),
            current_id: 0,
        };
        agent.reset_ports_and_keys();
        Ok(agent)
    }

    fn reset_ports_and_keys(&mut self) {
        self.in_ports = (0..self.n).map(|i| format!("in{}", i + 1)).collect();
        self.keys = (0..self.n)
            .map(|i| {
                self.data
                    .configs
                    .as_ref()
                    .map(|c| c.get_string_or_default(&format!("key{}", i + 1)))
                    .unwrap_or_default()
            })
            .collect();
        self.input_value = vec![None; self.n];
        self.current_id = 0;
    }

    fn configs_changed_impl(&mut self, configs: &AgentConfigs) -> Result<(), AgentError> {
        for (i, key_slot) in self.keys.iter_mut().enumerate() {
            *key_slot = configs.get_string_or_default(&format!("key{}", i + 1));
        }
        Ok(())
    }

    async fn process_impl(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<Option<AgentValue>, AgentError> {
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
            for slot in &mut self.input_value {
                *slot = None;
            }
        }

        // Store the input value
        for i in 0..self.n {
            if pin == self.in_ports[i] {
                self.input_value[i] = Some(value.clone());
            }
        }

        // Check if all inputs are present
        if self.input_value.iter().any(|v| v.is_none()) {
            return Ok(None);
        }

        // All inputs are present, create the output
        let mut map = AgentValueMap::new();
        for i in 0..self.n {
            let key = self.keys[i].clone();
            let value = self.input_value[i].take().unwrap();
            map.insert(key, value);
        }
        let out_value = AgentValue::object(map);

        Ok(Some(out_value))
    }
}

static CATEGORY: &str = "Std/Stream";

static PIN_DATA: &str = "data";
static PIN_IN1: &str = "in1";
static PIN_IN2: &str = "in2";
static PIN_IN3: &str = "in3";
static PIN_IN4: &str = "in4";

static CONFIG_KEY1: &str = "key1";
static CONFIG_KEY2: &str = "key2";
static CONFIG_KEY3: &str = "key3";
static CONFIG_KEY4: &str = "key4";

#[askit_agent(
    title = "Zip2",
    category = CATEGORY,
    inputs = [PIN_IN1, PIN_IN2],
    outputs = [PIN_DATA],
    string_config(name = CONFIG_KEY1),
    string_config(name = CONFIG_KEY2)
)]
struct Zip2Agent {
    inner: ZipAgent,
}

#[async_trait]
impl AsAgent for Zip2Agent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            inner: ZipAgent::new_with_n(askit, id, def_name, config, 2)?,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.inner.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.inner.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let configs = self.configs()?.clone();
        self.inner.configs_changed_impl(&configs)
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if let Some(out) = self.inner.process_impl(ctx.clone(), pin, value).await? {
            self.try_output(ctx, PIN_DATA, out)?;
        }
        Ok(())
    }
}

#[askit_agent(
    title = "Zip3",
    category = CATEGORY,
    inputs = [PIN_IN1, PIN_IN2, PIN_IN3],
    outputs = [PIN_DATA],
    string_config(name = CONFIG_KEY1),
    string_config(name = CONFIG_KEY2),
    string_config(name = CONFIG_KEY3)
)]
struct Zip3Agent {
    inner: ZipAgent,
}

#[async_trait]
impl AsAgent for Zip3Agent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            inner: ZipAgent::new_with_n(askit, id, def_name, config, 3)?,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.inner.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.inner.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let configs = self.configs()?.clone();
        self.inner.configs_changed_impl(&configs)
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if let Some(out) = self.inner.process_impl(ctx.clone(), pin, value).await? {
            self.try_output(ctx, PIN_DATA, out)?;
        }
        Ok(())
    }
}

#[askit_agent(
    title = "Zip4",
    category = CATEGORY,
    inputs = [PIN_IN1, PIN_IN2, PIN_IN3, PIN_IN4],
    outputs = [PIN_DATA],
    string_config(name = CONFIG_KEY1),
    string_config(name = CONFIG_KEY2),
    string_config(name = CONFIG_KEY3),
    string_config(name = CONFIG_KEY4)
)]
struct Zip4Agent {
    inner: ZipAgent,
}

#[async_trait]
impl AsAgent for Zip4Agent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            inner: ZipAgent::new_with_n(askit, id, def_name, config, 4)?,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.inner.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.inner.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let configs = self.configs()?.clone();
        self.inner.configs_changed_impl(&configs)
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if let Some(out) = self.inner.process_impl(ctx.clone(), pin, value).await? {
            self.try_output(ctx, PIN_DATA, out)?;
        }
        Ok(())
    }
}
