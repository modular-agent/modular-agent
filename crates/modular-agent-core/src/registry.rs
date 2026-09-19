use crate::{ModularAgent, ModuleDefinition};

/// Registration entry emitted by the `#[modular_agent]` macro.
pub struct ModuleRegistration {
    pub build: fn() -> ModuleDefinition,
}

inventory::collect!(ModuleRegistration);

/// Register all modules collected via the `#[modular_agent]` macro.
pub(crate) fn register_inventory_modules(ma: &ModularAgent) {
    for reg in inventory::iter::<ModuleRegistration> {
        ma.register_module_definition((reg.build)());
    }
}
