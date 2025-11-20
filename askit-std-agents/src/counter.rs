use std::vec;

use agent_stream_kit::{
    ASKit, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentDisplayConfigEntry,
    AgentError, AgentOutput, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};

/// Counter
struct CounterAgent {
    data: AsAgentData,
    count: i64,
}

#[async_trait]
impl AsAgent for CounterAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
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
        self.emit_display(DISPLAY_COUNT, AgentData::integer(0));
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        _data: AgentData,
    ) -> Result<(), AgentError> {
        if pin == PIN_RESET {
            self.count = 0;
        } else if pin == PIN_IN {
            self.count += 1;
        }
        self.try_output(ctx, PIN_COUNT, AgentData::integer(self.count))?;
        self.emit_display(DISPLAY_COUNT, AgentData::integer(self.count));

        Ok(())
    }
}

static CATEGORY: &str = "Core/Utils";

static PIN_IN: &str = "in";
static PIN_RESET: &str = "reset";
static PIN_COUNT: &str = "count";

static DISPLAY_COUNT: &str = "count";

pub fn register_agents(askit: &ASKit) {
    // Counter Agent
    askit.register_agent(
        AgentDefinition::new(
            "agent",
            "std_counter",
            Some(new_agent_boxed::<CounterAgent>),
        )
        .title("Counter")
        // .description("Display value on the node")
        .category(CATEGORY)
        .inputs(vec![PIN_IN, PIN_RESET])
        .outputs(vec![PIN_COUNT])
        .display_configs(vec![(
            DISPLAY_COUNT,
            AgentDisplayConfigEntry::new("integer").hide_title(),
        )]),
    );
}
