use std::vec;

use modular_agent_core::{
    AsModule, ModularAgent, Module, ModuleContext, ModuleData, ModuleOutput, ModuleSpec,
    ModuleStatus, Result, Value, async_trait, modular_agent,
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
#[modular_agent(
    kind = "Input",
    title = "Unit Input",
    hide_title,
    category = CATEGORY,
    outputs = [UNIT],
    unit_config(name = UNIT, hide_title),
    hint(color=2),
)]
struct UnitInputModule {
    data: ModuleData,
}

impl AsModule for UnitInputModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        // Since set_config is called even when the module is not running,
        // we need to check the status before outputting the value.
        if *self.status() == ModuleStatus::Start {
            self.try_output(ModuleContext::new(), UNIT, Value::unit())?;
        }

        Ok(())
    }
}

// Boolean Input
#[modular_agent(
    kind = "Input",
    title = "Boolean Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [BOOLEAN],
    boolean_config(name = BOOLEAN, hide_title),
    hint(color=3),
)]
struct BooleanInputModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for BooleanInputModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        if *self.status() == ModuleStatus::Start {
            let value = self.configs()?.get(BOOLEAN)?;
            self.try_output(ModuleContext::new(), BOOLEAN, value.clone())?;
        }
        Ok(())
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, _value: Value) -> Result<()> {
        let value = self.configs()?.get(BOOLEAN)?;
        self.output(ctx, BOOLEAN, value.clone()).await
    }
}

// Integer Input
#[modular_agent(
    kind = "Input",
    title = "Integer Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [INTEGER],
    integer_config(name = INTEGER, hide_title),
    hint(color=6),
)]
struct IntegerInputModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for IntegerInputModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        if *self.status() == ModuleStatus::Start {
            let value = self.configs()?.get(INTEGER)?;
            self.try_output(ModuleContext::new(), INTEGER, value.clone())?;
        }
        Ok(())
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, _value: Value) -> Result<()> {
        let value = self.configs()?.get(INTEGER)?;
        self.output(ctx, INTEGER, value.clone()).await
    }
}

// Number Input
#[modular_agent(
    kind = "Input",
    title = "Number Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [NUMBER],
    number_config(name = NUMBER, hide_title),
    hint(color=6),
)]
struct NumberInputModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for NumberInputModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        if *self.status() == ModuleStatus::Start {
            let value = self.configs()?.get_number(NUMBER)?; // Should we use to_number here?
            self.try_output(ModuleContext::new(), NUMBER, Value::number(value))?;
        }
        Ok(())
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, _value: Value) -> Result<()> {
        let value = self.configs()?.get_number(NUMBER)?;
        self.output(ctx, NUMBER, Value::number(value)).await
    }
}

// String Input
#[modular_agent(
    kind = "Input",
    title = "String Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [STRING],
    string_config(name = STRING, hide_title),
    hint(color=5),
)]
struct StringInputModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for StringInputModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        if *self.status() == ModuleStatus::Start {
            let value = self.configs()?.get(STRING)?;
            self.try_output(ModuleContext::new(), STRING, value.clone())?;
        }
        Ok(())
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, _value: Value) -> Result<()> {
        let value = self.configs()?.get(STRING)?;
        self.output(ctx, STRING, value.clone()).await
    }
}

// Text Input
#[modular_agent(
    kind = "Input",
    title = "Text Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [TEXT],
    text_config(name = TEXT, hide_title),
    hint(color=5),
)]
struct TextInputModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for TextInputModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        if *self.status() == ModuleStatus::Start {
            let value = self.configs()?.get(TEXT)?;
            self.try_output(ModuleContext::new(), TEXT, value.clone())?;
        }
        Ok(())
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, _value: Value) -> Result<()> {
        let value = self.configs()?.get(TEXT)?;
        self.output(ctx, TEXT, value.clone()).await
    }
}

// Object Input
#[modular_agent(
    kind = "Input",
    title = "Object Input",
    category = CATEGORY,
    inputs = [UNIT],
    outputs = [OBJECT],
    object_config(name = OBJECT, hide_title),
    hint(color=4),
)]
struct ObjectInputModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for ObjectInputModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    fn configs_changed(&mut self) -> Result<()> {
        if *self.status() == ModuleStatus::Start {
            let value = self.configs()?.get(OBJECT)?;
            self.try_output(ModuleContext::new(), OBJECT, value.clone())?;
        }
        Ok(())
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, _value: Value) -> Result<()> {
        let value = self.configs()?.get(OBJECT)?;
        self.output(ctx, OBJECT, value.clone()).await
    }
}
