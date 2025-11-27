use crate::{AgentDefinition, ASKit};

/// Registration entry emitted by the `#[askit_agent]` macro.
pub struct AgentRegistration {
    pub build: fn() -> AgentDefinition,
}

inventory::collect!(AgentRegistration);

/// Register all agents collected via the `#[askit_agent]` macro.
pub fn register_inventory_agents(askit: &ASKit) {
    for reg in inventory::iter::<AgentRegistration> {
        askit.register_agent((reg.build)());
    }
}
