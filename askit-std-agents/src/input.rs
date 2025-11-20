use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentError, AgentOutput,
    AgentStatus, AsAgent, AsAgentData, new_agent_boxed,
};

/// Unit Input
struct UnitInputAgent {
    data: AsAgentData,
}

impl AsAgent for UnitInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, configs),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        // Since set_config is called even when the agent is not running,
        // we need to check the status before outputting the value.
        if *self.status() == AgentStatus::Start {
            self.try_output(AgentContext::new(), CONFIG_UNIT, AgentData::unit())?;
        }

        Ok(())
    }
}

// Boolean Input
struct BooleanInputAgent {
    data: AsAgentData,
}

impl AsAgent for BooleanInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, configs),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get_bool(CONFIG_BOOLEAN)?;
            self.try_output(
                AgentContext::new(),
                CONFIG_BOOLEAN,
                AgentData::boolean(value),
            )?;
        }
        Ok(())
    }
}

// Integer Input
struct IntegerInputAgent {
    data: AsAgentData,
}

impl AsAgent for IntegerInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, configs),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get_integer(CONFIG_INTEGER)?;
            self.try_output(
                AgentContext::new(),
                CONFIG_INTEGER,
                AgentData::integer(value),
            )?;
        }
        Ok(())
    }
}

// Number Input
struct NumberInputAgent {
    data: AsAgentData,
}

impl AsAgent for NumberInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, configs),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get_number(CONFIG_NUMBER)?;
            self.try_output(AgentContext::new(), CONFIG_NUMBER, AgentData::number(value))?;
        }
        Ok(())
    }
}

// String Input
struct StringInputAgent {
    data: AsAgentData,
}

impl AsAgent for StringInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, configs),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get_string(CONFIG_STRING)?;
            self.try_output(AgentContext::new(), CONFIG_STRING, AgentData::string(value))?;
        }
        Ok(())
    }
}

// Text Input
struct TextInputAgent {
    data: AsAgentData,
}

impl AsAgent for TextInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, configs),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get_string(CONFIG_TEXT)?;
            self.try_output(AgentContext::new(), CONFIG_TEXT, AgentData::string(value))?;
        }
        Ok(())
    }
}

// Object Input
struct ObjectInputAgent {
    data: AsAgentData,
}

impl AsAgent for ObjectInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, configs),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get(CONFIG_OBJECT)?;
            if let Some(obj) = value.as_object() {
                self.try_output(
                    AgentContext::new(),
                    CONFIG_OBJECT,
                    AgentData::object(obj.clone()),
                )?;
            } else if let Some(arr) = value.as_array() {
                self.try_output(
                    AgentContext::new(),
                    CONFIG_OBJECT,
                    AgentData::array("object", arr.clone()),
                )?;
            } else {
                return Err(AgentError::InvalidConfig(format!(
                    "Invalid object value for config '{}'",
                    CONFIG_OBJECT
                )));
            }
        }
        Ok(())
    }
}

// Register Agents

static KIND: &str = "agent";
static CATEGORY: &str = "Core/Input";

static CONFIG_UNIT: &str = "unit";
static CONFIG_BOOLEAN: &str = "boolean";
static CONFIG_INTEGER: &str = "integer";
static CONFIG_NUMBER: &str = "number";
static CONFIG_STRING: &str = "string";
static CONFIG_TEXT: &str = "text";
static CONFIG_OBJECT: &str = "object";

pub fn register_agents(askit: &ASKit) {
    // Unit Input Agent
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_unit_input",
            Some(new_agent_boxed::<UnitInputAgent>),
        )
        .title("Unit Input")
        .category(CATEGORY)
        .outputs(vec![CONFIG_UNIT])
        .unit_config(CONFIG_UNIT),
    );

    // Boolean Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_boolean_input",
            Some(new_agent_boxed::<BooleanInputAgent>),
        )
        .title("Boolean Input")
        .category(CATEGORY)
        .outputs(vec![CONFIG_BOOLEAN])
        .boolean_config_default(CONFIG_BOOLEAN),
    );

    // Integer Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_integer_input",
            Some(new_agent_boxed::<IntegerInputAgent>),
        )
        .title("Integer Input")
        .category(CATEGORY)
        .outputs(vec![CONFIG_INTEGER])
        .integer_config_default(CONFIG_INTEGER),
    );

    // Number Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_number_input",
            Some(new_agent_boxed::<NumberInputAgent>),
        )
        .title("Number Input")
        .category(CATEGORY)
        .outputs(vec![CONFIG_NUMBER])
        .number_config_default(CONFIG_NUMBER),
    );

    // String Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_string_input",
            Some(new_agent_boxed::<StringInputAgent>),
        )
        .title("String Input")
        .category(CATEGORY)
        .outputs(vec![CONFIG_STRING])
        .string_config_default(CONFIG_STRING),
    );

    // Text Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_text_input",
            Some(new_agent_boxed::<TextInputAgent>),
        )
        .title("Text Input")
        .category(CATEGORY)
        .outputs(vec![CONFIG_TEXT])
        .text_config_default(CONFIG_TEXT),
    );

    // Object Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_object_input",
            Some(new_agent_boxed::<ObjectInputAgent>),
        )
        .title("Object Input")
        .category(CATEGORY)
        .outputs(vec![CONFIG_OBJECT])
        .object_config_default(CONFIG_OBJECT),
    );
}
