use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentError, AgentOutput, AgentStatus, AgentValue,
    AsAgent, AsAgentData,
};
use askit_macros::askit_agent;

static CATEGORY: &str = "Std/Input";

static UNIT: &str = "unit";
static BOOLEAN: &str = "boolean";
static INTEGER: &str = "integer";
static NUMBER: &str = "number";
static STRING: &str = "string";
static TEXT: &str = "text";
static OBJECT: &str = "object";

/// Unit Input
#[askit_agent(
    title = "Unit Input",
    category = CATEGORY,
    outputs = [UNIT],
    unit_config(name = UNIT)
)]
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
            self.try_output(AgentContext::new(), UNIT, AgentValue::unit())?;
        }

        Ok(())
    }
}

// Boolean Input
#[askit_agent(
    title = "Boolean Input",
    category = CATEGORY,
    outputs = [BOOLEAN],
    boolean_config(name = BOOLEAN)
)]
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
            let value = self.configs()?.get_bool(BOOLEAN)?;
            self.try_output(AgentContext::new(), BOOLEAN, AgentValue::boolean(value))?;
        }
        Ok(())
    }
}

// Integer Input
#[askit_agent(
    title = "Integer Input",
    category = CATEGORY,
    outputs = [INTEGER],
    integer_config(name = INTEGER)
)]
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
            let value = self.configs()?.get_integer(INTEGER)?;
            self.try_output(AgentContext::new(), INTEGER, AgentValue::integer(value))?;
        }
        Ok(())
    }
}

// Number Input
#[askit_agent(
    title = "Number Input",
    category = CATEGORY,
    outputs = [NUMBER],
    number_config(name = NUMBER)
)]
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
            let value = self.configs()?.get_number(NUMBER)?;
            self.try_output(AgentContext::new(), NUMBER, AgentValue::number(value))?;
        }
        Ok(())
    }
}

// String Input
#[askit_agent(
    title = "String Input",
    category = CATEGORY,
    outputs = [STRING],
    string_config(name = STRING)
)]
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
            let value = self.configs()?.get_string(STRING)?;
            self.try_output(AgentContext::new(), STRING, AgentValue::string(value))?;
        }
        Ok(())
    }
}

// Text Input
#[askit_agent(
    title = "Text Input",
    category = CATEGORY,
    outputs = [TEXT],
    text_config(name = TEXT)
)]
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
            let value = self.configs()?.get_string(TEXT)?;
            self.try_output(AgentContext::new(), TEXT, AgentValue::string(value))?;
        }
        Ok(())
    }
}

// Object Input
#[askit_agent(
    title = "Object Input",
    category = CATEGORY,
    outputs = [OBJECT],
    object_config(name = OBJECT)
)]
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
            let value = self.configs()?.get(OBJECT)?;
            if let Some(obj) = value.as_object() {
                self.try_output(AgentContext::new(), OBJECT, AgentValue::object(obj.clone()))?;
            } else if let Some(arr) = value.as_array() {
                self.try_output(AgentContext::new(), OBJECT, AgentValue::array(arr.clone()))?;
            } else {
                return Err(AgentError::InvalidConfig(format!(
                    "Invalid object value for config '{}'",
                    OBJECT
                )));
            }
        }
        Ok(())
    }
}
