extern crate modular_agent_core as ma;

use ma::{ModularAgent, PatchSpec};

use crate::common;

const COUNTER_DEF: &str = common::agents::CounterAgent::DEF_NAME;

// PatchNode

#[test]
fn test_agent_spec_from_def() {
    let ma = ModularAgent::init().unwrap();

    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();

    let spec = def.to_spec();

    assert_eq!(spec.def_name, COUNTER_DEF);

    let spec2 = def.to_spec();
    assert_eq!(spec2.def_name, COUNTER_DEF);
    assert!(spec.id != spec2.id);
}

// Patch

#[test]
fn test_patch_add_agent() {
    let ma = ModularAgent::init().unwrap();

    let mut spec = PatchSpec::default();
    assert_eq!(spec.agents.len(), 0);

    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    let agent_spec = def.to_spec();

    spec.add_agent(agent_spec);

    assert_eq!(spec.agents.len(), 1);
}

#[test]
fn test_patch_remove_agent() {
    let ma = ModularAgent::init().unwrap();

    let mut spec = PatchSpec::default();
    assert_eq!(spec.agents.len(), 0);

    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    let agent_spec = def.to_spec();
    let agent_id = agent_spec.id.clone();

    spec.add_agent(agent_spec);
    assert_eq!(spec.agents.len(), 1);

    spec.remove_agent(&agent_id);
    assert_eq!(spec.agents.len(), 0);
}
