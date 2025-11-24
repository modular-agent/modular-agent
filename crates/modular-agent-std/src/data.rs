use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentError, AgentOutput,
    AgentValue, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};

// To JSON
struct ToJsonAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ToJsonAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        let json = serde_json::to_string_pretty(&data.value)
            .map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        self.try_output(ctx, PIN_JSON, AgentData::string(json))?;
        Ok(())
    }
}

// From JSON
struct FromJsonAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for FromJsonAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        let s = data
            .value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("not a string".to_string()))?;
        let json_value: serde_json::Value =
            serde_json::from_str(s).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let data = AgentData::from_json(json_value)?;
        self.try_output(ctx, PIN_DATA, data)?;
        Ok(())
    }
}

// Get Value
struct GetValueAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for GetValueAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        let key = self.configs()?.get_string(CONFIG_KEY)?;

        if key.is_empty() {
            return Ok(());
        }

        let keys = key.split('.').collect::<Vec<_>>();

        if data.is_array() {
            let mut out_arr = Vec::new();
            for v in data
                .as_array()
                .ok_or_else(|| AgentError::InvalidValue("failed as_array".to_string()))?
            {
                let mut value = v.clone();
                for key in &keys {
                    let Some(obj) = value.as_object() else {
                        value = AgentValue::unit();
                        break;
                    };
                    if let Some(v) = obj.get(*key) {
                        value = v.clone();
                    } else {
                        value = AgentValue::unit();
                        break;
                    }
                }
                out_arr.push(value);
            }
            let kind = if out_arr.is_empty() {
                "unit"
            } else {
                &out_arr[0].kind()
            };
            self.try_output(ctx, PIN_VALUE, AgentData::array(kind.to_string(), out_arr))?;
        } else if data.is_object() {
            let mut value = data.value;
            for key in keys {
                let Some(obj) = value.as_object() else {
                    value = AgentValue::unit();
                    break;
                };
                if let Some(v) = obj.get(key) {
                    value = v.clone();
                } else {
                    // TODO: Add a config to determine whether to output unit
                    value = AgentValue::unit();
                    break;
                }
            }

            self.try_output(ctx, PIN_VALUE, AgentData::from_value(value))?;
        }

        Ok(())
    }
}

// Set Value
struct SetValueAgent {
    data: AsAgentData,
    input_data: Option<AgentData>,
    input_value: Option<AgentValue>,
    current_id: usize,
}

#[async_trait]
impl AsAgent for SetValueAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            input_data: None,
            input_value: None,
            current_id: 0,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        // Reset input values if context ID changes
        let ctx_id = ctx.id();
        if ctx_id != self.current_id {
            self.current_id = ctx_id;
            self.input_data = None;
            self.input_value = None;
        }

        // Store input data or value
        if pin == PIN_DATA {
            if data.is_object() {
                self.input_data = Some(data);
            }
        } else if pin == PIN_VALUE {
            self.input_value = Some(data.value);
        }
        if self.input_data.is_none() || self.input_value.is_none() {
            return Ok(());
        }

        // parse key
        let key = self.configs()?.get_string(CONFIG_KEY)?;
        if key.is_empty() {
            return Ok(());
        }
        let keys = key.split('.').collect::<Vec<_>>();

        // set value
        if self.input_data.as_ref().unwrap().is_object() {
            let new_value = self.input_value.take().unwrap();
            let value = self.input_data.take().unwrap().value;
            let mut obj = value
                .as_object()
                .ok_or_else(|| AgentError::InvalidValue("failed as_object_mut".to_string()))?
                .clone();
            let last_key = keys.last().unwrap();
            let mut current = &mut obj;
            for key in &keys[..keys.len() - 1] {
                if !current.contains_key(*key) {
                    current.insert((*key).to_string(), AgentValue::object_default());
                }
                let next = current
                    .get_mut(*key)
                    .ok_or_else(|| AgentError::InvalidValue("failed get_mut".to_string()))?;
                let next_obj = next
                    .as_object_mut()
                    .ok_or_else(|| AgentError::InvalidValue("failed as_object_mut".to_string()))?;
                current = next_obj;
            }
            current.insert((*last_key).to_string(), new_value);

            self.try_output(
                ctx,
                PIN_DATA,
                AgentData::from_value(AgentValue::object(obj)),
            )?;
        }

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/Data";

static PIN_DATA: &str = "data";
static PIN_JSON: &str = "json";
static PIN_VALUE: &str = "value";

static CONFIG_KEY: &str = "key";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_data_to_json",
            Some(new_agent_boxed::<ToJsonAgent>),
        )
        .title("To JSON")
        .category(CATEGORY)
        .inputs(vec![PIN_DATA])
        .outputs(vec![PIN_JSON]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_data_from_json",
            Some(new_agent_boxed::<FromJsonAgent>),
        )
        .title("From JSON")
        .category(CATEGORY)
        .inputs(vec![PIN_JSON])
        .outputs(vec![PIN_DATA]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_data_get_value",
            Some(new_agent_boxed::<GetValueAgent>),
        )
        .title("Get Value")
        .category(CATEGORY)
        .inputs(vec![PIN_DATA])
        .outputs(vec![PIN_VALUE])
        .string_config_default(CONFIG_KEY),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_data_set_value",
            Some(new_agent_boxed::<SetValueAgent>),
        )
        .title("Set Value")
        .category(CATEGORY)
        .inputs(vec![PIN_DATA, PIN_VALUE])
        .outputs(vec![PIN_DATA])
        .string_config_default(CONFIG_KEY),
    );
}
