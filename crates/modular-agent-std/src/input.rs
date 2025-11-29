use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentError, AgentOutput, AgentStatus,
    AgentValue, AsAgent, async_trait,
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
    data: AgentData,
}

impl AsAgent for UnitInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, configs),
        })
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
    inputs = [UNIT],
    outputs = [BOOLEAN],
    boolean_config(name = BOOLEAN),
)]
struct BooleanInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for BooleanInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, configs),
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get(BOOLEAN)?;
            self.try_output(AgentContext::new(), BOOLEAN, value.clone())?;
        }
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get(BOOLEAN)?;
        self.try_output(ctx, BOOLEAN, value.clone())
    }
}

// Integer Input
#[askit_agent(
    title = "Integer Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [INTEGER],
    integer_config(name = INTEGER)
)]
struct IntegerInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for IntegerInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, configs),
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get(INTEGER)?;
            self.try_output(AgentContext::new(), INTEGER, value.clone())?;
        }
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get(INTEGER)?;
        self.try_output(ctx, INTEGER, value.clone())
    }
}

// Number Input
#[askit_agent(
    title = "Number Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [NUMBER],
    number_config(name = NUMBER)
)]
struct NumberInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for NumberInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, configs),
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get(NUMBER)?;
            self.try_output(AgentContext::new(), NUMBER, value.clone())?;
        }
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get(NUMBER)?;
        self.try_output(ctx, NUMBER, value.clone())
    }
}

// String Input
#[askit_agent(
    title = "String Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [STRING],
    string_config(name = STRING)
)]
struct StringInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for StringInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, configs),
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get(STRING)?;
            self.try_output(AgentContext::new(), STRING, value.clone())?;
        }
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get(STRING)?;
        self.try_output(ctx, STRING, value.clone())
    }
}

// Text Input
#[askit_agent(
    title = "Text Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [TEXT],
    text_config(name = TEXT)
)]
struct TextInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for TextInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, configs),
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get(TEXT)?;
            self.try_output(AgentContext::new(), TEXT, value.clone())?;
        }
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get(TEXT)?;
        self.try_output(ctx, TEXT, value.clone())
    }
}

// Object Input
#[askit_agent(
    title = "Object Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [OBJECT],
    object_config(name = OBJECT)
)]
struct ObjectInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ObjectInputAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, configs),
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get(OBJECT)?;
            self.try_output(AgentContext::new(), OBJECT, value.clone())?;
        }
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get(OBJECT)?;
        self.try_output(ctx, OBJECT, value.clone())
    }
}
