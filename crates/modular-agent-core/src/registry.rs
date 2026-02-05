use crate::{AgentDefinition, ModularAgent};

/// Registration entry emitted by the `#[modular_agent]` macro.
pub struct AgentRegistration {
    pub build: fn() -> AgentDefinition,
}

inventory::collect!(AgentRegistration);

/// Register all agents collected via the `#[modular_agent]` macro.
pub(crate) fn register_inventory_agents(ma: &ModularAgent) {
    for reg in inventory::iter::<AgentRegistration> {
        ma.register_agent_definiton((reg.build)());
    }
}
