use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfig, AgentConfigEntry, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AgentValue, AsAgent, AsAgentData, async_trait, new_agent_boxed,
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
        config: Option<AgentConfig>,
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

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let json = serde_json::to_string_pretty(&data.value)
            .map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        self.try_output(ctx, PORT_JSON, AgentData::new_text(json))?;
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
        config: Option<AgentConfig>,
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

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let s = data
            .value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("not a string".to_string()))?;
        let json_value: serde_json::Value =
            serde_json::from_str(s).map_err(|e| AgentError::InvalidValue(e.to_string()))?;
        let data = AgentData::from_json_value(json_value)?;
        self.try_output(ctx, PORT_DATA, data)?;
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
        config: Option<AgentConfig>,
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

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let property = self.config()?.get_string(CONFIG_PROPERTY)?;

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
                        value = AgentValue::new_unit();
                        break;
                    };
                    if let Some(v) = obj.get(*prop) {
                        value = v.clone();
                    } else {
                        value = AgentValue::new_unit();
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
            self.try_output(
                ctx,
                PORT_DATA,
                AgentData::new_array(kind.to_string(), out_arr),
            )?;
        } else if data.is_object() {
            let mut value = data.value;
            for prop in props {
                let Some(obj) = value.as_object() else {
                    value = AgentValue::new_unit();
                    break;
                };
                if let Some(v) = obj.get(prop) {
                    value = v.clone();
                } else {
                    // TODO: Add a config to determine whether to output unit
                    value = AgentValue::new_unit();
                    break;
                }
            }

            self.try_output(ctx, PORT_DATA, AgentData::from_value(value))?;
        }

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/Data";

static PORT_DATA: &str = "data";
static PORT_JSON: &str = "json";

static CONFIG_PROPERTY: &str = "property";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_to_json",
            Some(new_agent_boxed::<ToJsonAgent>),
        )
        .with_title("To JSON")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_DATA])
        .with_outputs(vec![PORT_JSON]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_from_json",
            Some(new_agent_boxed::<FromJsonAgent>),
        )
        .with_title("From JSON")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_JSON])
        .with_outputs(vec![PORT_DATA]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "std_get_property",
            Some(new_agent_boxed::<GetPropertyAgent>),
        )
        .with_title("Get Property")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_DATA])
        .with_outputs(vec![PORT_DATA])
        .with_default_config(vec![(
            CONFIG_PROPERTY.into(),
            AgentConfigEntry::new(AgentValue::new_string(""), "string"),
        )]),
    );
}
