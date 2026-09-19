extern crate modular_agent_core as ma;

use ma::{ModularAgent, PatchSpec};

use crate::common;

const COUNTER_DEF: &str = common::modules::CounterModule::DEF_NAME;

// PatchNode

#[test]
fn test_module_spec_from_def() {
    let ma = ModularAgent::init().unwrap();

    let def = ma.get_module_definition(COUNTER_DEF).unwrap();

    let spec = def.to_spec();

    assert_eq!(spec.def_name, COUNTER_DEF);

    let spec2 = def.to_spec();
    assert_eq!(spec2.def_name, COUNTER_DEF);
    assert!(spec.id != spec2.id);
}

// Patch

#[test]
fn test_patch_add_module() {
    let ma = ModularAgent::init().unwrap();

    let mut spec = PatchSpec::default();
    assert_eq!(spec.modules.len(), 0);

    let def = ma.get_module_definition(COUNTER_DEF).unwrap();
    let module_spec = def.to_spec();

    spec.add_module(module_spec);

    assert_eq!(spec.modules.len(), 1);
}

#[test]
fn test_patch_remove_module() {
    let ma = ModularAgent::init().unwrap();

    let mut spec = PatchSpec::default();
    assert_eq!(spec.modules.len(), 0);

    let def = ma.get_module_definition(COUNTER_DEF).unwrap();
    let module_spec = def.to_spec();
    let module_id = module_spec.id.clone();

    spec.add_module(module_spec);
    assert_eq!(spec.modules.len(), 1);

    spec.remove_module(&module_id);
    assert_eq!(spec.modules.len(), 0);
}
