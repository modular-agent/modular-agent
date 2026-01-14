use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentStatus,
    AgentValue, AsAgent, askit_agent, async_trait,
};

const CATEGORY: &str = "Std/Input";

const UNIT: &str = "unit";
const BOOLEAN: &str = "boolean";
const INTEGER: &str = "integer";
const NUMBER: &str = "number";
const STRING: &str = "string";
const TEXT: &str = "text";
const OBJECT: &str = "object";

/// Unit Input
#[askit_agent(
    kind = "Input",
    title = "Unit Input",
    category = CATEGORY,
    outputs = [UNIT],
    unit_config(name = UNIT, hide_title)
)]
struct UnitInputAgent {
    data: AgentData,
}

impl AsAgent for UnitInputAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
    kind = "Input",
    title = "Boolean Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [BOOLEAN],
    boolean_config(name = BOOLEAN, hide_title),
)]
struct BooleanInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for BooleanInputAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        self.output(ctx, BOOLEAN, value.clone()).await
    }
}

// Integer Input
#[askit_agent(
    kind = "Input",
    title = "Integer Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [INTEGER],
    integer_config(name = INTEGER, hide_title)
)]
struct IntegerInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for IntegerInputAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        self.output(ctx, INTEGER, value.clone()).await
    }
}

// Number Input
#[askit_agent(
    kind = "Input",
    title = "Number Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [NUMBER],
    number_config(name = NUMBER, hide_title)
)]
struct NumberInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for NumberInputAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        if *self.status() == AgentStatus::Start {
            let value = self.configs()?.get_number(NUMBER)?; // Should we use to_number here?
            self.try_output(AgentContext::new(), NUMBER, AgentValue::number(value))?;
        }
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get_number(NUMBER)?;
        self.output(ctx, NUMBER, AgentValue::number(value)).await
    }
}

// String Input
#[askit_agent(
    kind = "Input",
    title = "String Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [STRING],
    string_config(name = STRING, hide_title)
)]
struct StringInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for StringInputAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        self.output(ctx, STRING, value.clone()).await
    }
}

// Text Input
#[askit_agent(
    kind = "Input",
    title = "Text Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [TEXT],
    text_config(name = TEXT, hide_title)
)]
struct TextInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for TextInputAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        self.output(ctx, TEXT, value.clone()).await
    }
}

// Object Input
#[askit_agent(
    kind = "Input",
    title = "Object Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [OBJECT],
    object_config(name = OBJECT, hide_title)
)]
struct ObjectInputAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ObjectInputAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
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
        self.output(ctx, OBJECT, value.clone()).await
    }
}
