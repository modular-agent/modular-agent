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

// Get Property
struct GetPropertyAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for GetPropertyAgent {
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
        let property = self.configs()?.get_string(CONFIG_PROPERTY)?;

        if property.is_empty() {
            return Ok(());
        }

        let props = property.split('.').collect::<Vec<_>>();

        if data.is_array() {
            let mut out_arr = Vec::new();
            for v in data
                .as_array()
                .ok_or_else(|| AgentError::InvalidValue("failed as_array".to_string()))?
            {
                let mut value = v.clone();
                for prop in &props {
                    let Some(obj) = value.as_object() else {
                        value = AgentValue::unit();
                        break;
                    };
                    if let Some(v) = obj.get(*prop) {
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
            self.try_output(ctx, PIN_DATA, AgentData::array(kind.to_string(), out_arr))?;
        } else if data.is_object() {
            let mut value = data.value;
            for prop in props {
                let Some(obj) = value.as_object() else {
                    value = AgentValue::unit();
                    break;
                };
                if let Some(v) = obj.get(prop) {
                    value = v.clone();
                } else {
                    // TODO: Add a config to determine whether to output unit
                    value = AgentValue::unit();
                    break;
                }
            }

            self.try_output(ctx, PIN_DATA, AgentData::from_value(value))?;
        }

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/Data";

static PIN_DATA: &str = "data";
static PIN_JSON: &str = "json";

static CONFIG_PROPERTY: &str = "property";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_to_json",
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
            "std_from_json",
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
            "std_get_property",
            Some(new_agent_boxed::<GetPropertyAgent>),
        )
        .title("Get Property")
        .category(CATEGORY)
        .inputs(vec![PIN_DATA])
        .outputs(vec![PIN_DATA])
        .string_config_default(CONFIG_PROPERTY),
    );
}
