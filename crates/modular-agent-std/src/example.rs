use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    ModularAgent, async_trait, modular_agent,
};

const CATEGORY: &str = "Example";

const PORT_VALUE: &str = "value";
const PORT_UNIT: &str = "unit";
const PORT_COLOR: &str = "color";

const CONFIG_DATA: &str = "data";
const CONFIG_VALUE: &str = "value";
const CONFIG_COLOR: &str = "color";

const CHART_BUFFER_CAP: usize = 100;

/// Collects numeric inputs into a rolling buffer for chart rendering.
///
/// Demo agent for the custom NodeView mechanism. A numeric input (integer or
/// number) is appended to a rolling buffer capped at 100 entries; a numeric
/// array input replaces the whole buffer. After each update the buffer is
/// pushed to the frontend through the `data` config, so a companion chart
/// NodeView re-renders on every input; the buffer itself is kept in memory
/// and not saved into the patch. Non-numeric inputs are rejected with an
/// error.
///
/// # Ports
/// - Input `value`: A number to append, or a numeric array to replace the buffer
///
/// # Configuration
/// - `data`: The current numeric buffer rendered by the chart (default: `[]`)
///
/// # Example
/// With buffer `[1, 2]`, input `3` yields `[1, 2, 3]`; input `[9, 8]` yields `[9, 8]`.
#[modular_agent(
    title = "Chart Demo",
    category = CATEGORY,
    inputs = [PORT_VALUE],
    array_config(name = CONFIG_DATA),
    hint(width = 2),
)]
struct ChartDemoAgent {
    data: AgentData,
    buf: im::Vector<AgentValue>,
}

#[async_trait]
impl AsAgent for ChartDemoAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            buf: im::Vector::new(),
        })
    }

    async fn start(&mut self) -> Result<(), AgentError> {
        self.buf = im::Vector::new();
        // The buffer is only emitted, never persisted; this write clears a
        // buffer an older version saved into the patch.
        self.set_config(CONFIG_DATA.to_string(), AgentValue::array_default())?;
        self.emit_config_updated(CONFIG_DATA, AgentValue::array_default());
        Ok(())
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if let Some(arr) = value.as_array() {
            if arr.iter().any(|v| v.as_f64().is_none()) {
                return Err(AgentError::InvalidValue(
                    "Array elements must be numeric".into(),
                ));
            }
            self.buf = arr.clone();
        } else if value.as_f64().is_some() {
            self.buf.push_back(value);
            while self.buf.len() > CHART_BUFFER_CAP {
                self.buf.pop_front();
            }
        } else {
            return Err(AgentError::InvalidValue(
                "Expected a number or a numeric array".into(),
            ));
        }
        self.emit_config_updated(CONFIG_DATA, AgentValue::array(self.buf.clone()));
        Ok(())
    }
}

/// Emits the current slider value when triggered.
///
/// Demo agent for the custom NodeView mechanism. The `value` config is meant
/// to be edited through a companion slider NodeView; any input on the `unit`
/// port emits the current value downstream.
///
/// # Ports
/// - Input `unit`: Any value; triggers emitting the current `value`
/// - Output `value`: The current integer value
///
/// # Configuration
/// - `value`: The integer value controlled by the slider (default: 50)
#[modular_agent(
    title = "Slider Demo",
    category = CATEGORY,
    inputs = [PORT_UNIT],
    outputs = [PORT_VALUE],
    integer_config(name = CONFIG_VALUE, default = 50),
)]
struct SliderDemoAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for SliderDemoAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let value = self.configs()?.get_integer_or(CONFIG_VALUE, 50);
        self.output(ctx, PORT_VALUE, AgentValue::integer(value))
            .await
    }
}

/// Emits the current color when triggered.
///
/// Demo agent for the custom ConfigWidget mechanism. The `color` config uses
/// the custom `color` value type (a `#rrggbb` string) so a registered color
/// widget renders it as a color picker; any input on the `unit` port emits
/// the current color string downstream.
///
/// # Ports
/// - Input `unit`: Any value; triggers emitting the current `color`
/// - Output `color`: The current color as a `#rrggbb` string
///
/// # Configuration
/// - `color`: The color value as a `#rrggbb` string (default: "#ff8800")
#[modular_agent(
    title = "Color Demo",
    category = CATEGORY,
    inputs = [PORT_UNIT],
    outputs = [PORT_COLOR],
    custom_config(name = CONFIG_COLOR, type_ = "color", default = "#ff8800"),
)]
struct ColorDemoAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for ColorDemoAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let color = self.configs()?.get_string_or(CONFIG_COLOR, "#ff8800");
        self.output(ctx, PORT_COLOR, AgentValue::string(color))
            .await
    }
}
