use std::vec;

use agent_stream_kit::prelude::*;

/// Unit Input
struct UnitInputAgent {
    data: AsAgentData,
}

impl AsAgent for UnitInputAgent {
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

    fn set_config(&mut self, _config: AgentConfig) -> Result<(), AgentError> {
        // Since set_config is called even when the agent is not running,
        // we need to check the status before outputting the value.
        if *self.status() == AgentStatus::Start {
            self.try_output(AgentContext::new(), CONFIG_UNIT, AgentData::new_unit())?;
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

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            if let Some(value) = config.get_bool(CONFIG_BOOLEAN) {
                self.try_output(
                    AgentContext::new(),
                    CONFIG_BOOLEAN,
                    AgentData::new_boolean(value),
                )?;
            }
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

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            if let Some(value) = config.get_integer(CONFIG_INTEGER) {
                self.try_output(
                    AgentContext::new(),
                    CONFIG_INTEGER,
                    AgentData::new_integer(value),
                )?;
            }
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

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            if let Some(value) = config.get_number(CONFIG_NUMBER) {
                self.try_output(
                    AgentContext::new(),
                    CONFIG_NUMBER,
                    AgentData::new_number(value),
                )?;
            }
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

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            if let Some(value) = config.get_string(CONFIG_STRING) {
                self.try_output(
                    AgentContext::new(),
                    CONFIG_STRING,
                    AgentData::new_string(value),
                )?;
            }
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

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            if let Some(value) = config.get_string(CONFIG_TEXT) {
                self.try_output(AgentContext::new(), CONFIG_TEXT, AgentData::new_text(value))?;
            }
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

    fn set_config(&mut self, config: AgentConfig) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            if let Some(value) = config.get(CONFIG_OBJECT) {
                if let Some(obj) = value.as_object() {
                    self.try_output(
                        AgentContext::new(),
                        CONFIG_OBJECT,
                        AgentData::new_object(obj.clone()),
                    )?;
                } else if let Some(arr) = value.as_array() {
                    self.try_output(
                        AgentContext::new(),
                        CONFIG_OBJECT,
                        AgentData::new_array("object", arr.clone()),
                    )?;
                } else {
                    return Err(AgentError::InvalidConfig(format!(
                        "Invalid object value for config '{}'",
                        CONFIG_OBJECT
                    )));
                }
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
        AgentDefinition::new(KIND, "std_unit_input", Some(new_boxed::<UnitInputAgent>))
            .with_title("Unit Input")
            .with_category(CATEGORY)
            .with_outputs(vec![CONFIG_UNIT])
            .with_default_config(vec![(
                CONFIG_UNIT.into(),
                AgentConfigEntry::new(AgentValue::new_unit(), "unit"),
            )]),
    );

    // Boolean Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_boolean_input",
            Some(new_boxed::<BooleanInputAgent>),
        )
        .with_title("Boolean Input")
        .with_category(CATEGORY)
        .with_outputs(vec![CONFIG_BOOLEAN])
        .with_default_config(vec![(
            CONFIG_BOOLEAN.into(),
            AgentConfigEntry::new(AgentValue::new_boolean(false), "boolean"),
        )]),
    );

    // Integer Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_integer_input",
            Some(new_boxed::<IntegerInputAgent>),
        )
        .with_title("Integer Input")
        .with_category(CATEGORY)
        .with_outputs(vec![CONFIG_INTEGER])
        .with_default_config(vec![(
            CONFIG_INTEGER.into(),
            AgentConfigEntry::new(AgentValue::new_integer(0), "integer"),
        )]),
    );

    // Number Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_number_input",
            Some(new_boxed::<NumberInputAgent>),
        )
        .with_title("Number Input")
        .with_category(CATEGORY)
        .with_outputs(vec![CONFIG_NUMBER])
        .with_default_config(vec![(
            CONFIG_NUMBER.into(),
            AgentConfigEntry::new(AgentValue::new_number(0.0), "number"),
        )]),
    );

    // String Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_string_input",
            Some(new_boxed::<StringInputAgent>),
        )
        .with_title("String Input")
        .with_category(CATEGORY)
        .with_outputs(vec![CONFIG_STRING])
        .with_default_config(vec![(
            CONFIG_STRING.into(),
            AgentConfigEntry::new(AgentValue::new_string(""), "string"),
        )]),
    );

    // Text Input
    askit.register_agent(
        AgentDefinition::new(KIND, "std_text_input", Some(new_boxed::<TextInputAgent>))
            .with_title("Text Input")
            .with_category(CATEGORY)
            .with_outputs(vec![CONFIG_TEXT])
            .with_default_config(vec![(
                CONFIG_TEXT.into(),
                AgentConfigEntry::new(AgentValue::new_string(""), "text"),
            )]),
    );

    // Object Input
    askit.register_agent(
        AgentDefinition::new(
            KIND,
            "std_object_input",
            Some(new_boxed::<ObjectInputAgent>),
        )
        .with_title("Object Input")
        .with_category(CATEGORY)
        .with_outputs(vec![CONFIG_OBJECT])
        .with_default_config(vec![(
            CONFIG_OBJECT.into(),
            AgentConfigEntry::new(AgentValue::default_object(), "object"),
        )]),
    );
}
