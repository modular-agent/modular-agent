extern crate modular_agent_kit as mak;

use mak::{MAK, PresetSpec};

use crate::common;

const COUNTER_DEF: &str = common::agents::CounterAgent::DEF_NAME;

// PresetNode

#[test]
fn test_agent_spec_from_def() {
    let mak = MAK::init().unwrap();

    let def = mak.get_agent_definition(COUNTER_DEF).unwrap();

    let spec = def.to_spec();

    assert_eq!(spec.def_name, COUNTER_DEF);

    let spec2 = def.to_spec();
    assert_eq!(spec2.def_name, COUNTER_DEF);
    assert!(spec.id != spec2.id);
}

// Preset

#[test]
fn test_preset_add_agent() {
    let mak = MAK::init().unwrap();

    let mut spec = PresetSpec::default();
    assert_eq!(spec.agents.len(), 0);

    let def = mak.get_agent_definition(COUNTER_DEF).unwrap();
    let agent_spec = def.to_spec();

    spec.add_agent(agent_spec);

    assert_eq!(spec.agents.len(), 1);
}

#[test]
fn test_preset_remove_agent() {
    let mak = MAK::init().unwrap();

    let mut spec = PresetSpec::default();
    assert_eq!(spec.agents.len(), 0);

    let def = mak.get_agent_definition(COUNTER_DEF).unwrap();
    let agent_spec = def.to_spec();
    let agent_id = agent_spec.id.clone();

    spec.add_agent(agent_spec);
    assert_eq!(spec.agents.len(), 1);

    spec.remove_agent(&agent_id);
    assert_eq!(spec.agents.len(), 0);
}
