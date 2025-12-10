use agent_stream_kit::{
    ASKit, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};

static CATEGORY: &str = "Std/Array";

static PIN_IN: &str = "in";
static PIN_OUT: &str = "out";

#[askit_agent(
    title = "Map",
    category = CATEGORY,
    inputs = [PIN_IN],
    outputs = [PIN_OUT],
)]
struct MapAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for MapAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let data = AgentData::new(askit, id, spec);
        Ok(Self { data })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if value.is_array() {
            let arr = value
                .as_array()
                .ok_or_else(|| AgentError::InvalidValue("Failed to get array".into()))?;
            let n = arr.len();
            for (i, item) in arr.iter().cloned().enumerate() {
                let c = ctx
                    .with_var("map_i".into(), AgentValue::integer(i as i64))
                    .with_var("map_n".into(), AgentValue::integer(n as i64));
                self.try_output(c, PIN_OUT, item.clone())?;
            }
        } else {
            return Err(AgentError::InvalidValue(
                "Input value is not an array".into(),
            ));
        }
        Ok(())
    }
}
