extern crate agent_stream_kit as askit;

use async_trait::async_trait;

use askit::{
    ASKit, AgentConfig, AgentContext, AgentData, AgentDefinition, AgentDisplayConfigEntry,
    AgentError, AgentOutput, AsAgent, AsAgentData, new_boxed,
};
use std::vec;

/// Counter
pub struct CounterAgent {
    data: AsAgentData,
    count: i64,
}

#[async_trait]
impl AsAgent for CounterAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            count: 0,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn start(&mut self) -> Result<(), AgentError> {
        self.count = 0;
        // self.emit_display(DISPLAY_COUNT, AgentData::new_integer(0))?;
        Ok(())
    }

    async fn process(&mut self, ctx: AgentContext, _data: AgentData) -> Result<(), AgentError> {
        let ch = ctx.ch();
        if ch == CH_RESET {
            self.count = 0;
        } else if ch == CH_IN {
            self.count += 1;
        }
        self.try_output(ctx, CH_COUNT, AgentData::new_integer(self.count))?;
        // self.emit_display(DISPLAY_COUNT, AgentData::new_integer(self.count))?;

        Ok(())
    }
}

static CATEGORY: &str = "Core/Utils";

static CH_IN: &str = "in";
static CH_RESET: &str = "reset";
static CH_COUNT: &str = "count";

static DISPLAY_COUNT: &str = "count";

pub fn register_agents(askit: &ASKit) {
    // Counter Agent
    askit.register_agent(
        AgentDefinition::new(
            "Agent", // AGENT_KIND_BUILTIN,
            "$counter",
            Some(new_boxed::<CounterAgent>),
        )
        .with_title("Counter")
        // .with_description("Display value on the node")
        .with_category(CATEGORY)
        .with_inputs(vec![CH_IN, CH_RESET])
        .with_outputs(vec![CH_COUNT])
        .with_display_config(vec![(
            DISPLAY_COUNT.into(),
            AgentDisplayConfigEntry::new("integer").with_hide_title(),
        )]),
    );
}
