use crate::{AgentDefinition, MAK};

/// Registration entry emitted by the `#[mak_agent]` macro.
pub struct AgentRegistration {
    pub build: fn() -> AgentDefinition,
}

inventory::collect!(AgentRegistration);

/// Register all agents collected via the `#[mak_agent]` macro.
pub fn register_inventory_agents(mak: &MAK) {
    for reg in inventory::iter::<AgentRegistration> {
        mak.register_agent_definiton((reg.build)());
    }
}
