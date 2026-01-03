extern crate agent_stream_kit as askit;

use askit::{ASKit, AgentStreamSpec};

use crate::common;

const COUNTER_DEF: &str = common::agents::CounterAgent::DEF_NAME;

// AgentStreamNode

#[test]
fn test_agent_spec_from_def() {
    let askit = ASKit::init().unwrap();

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();

    let spec = def.to_spec();

    assert_eq!(spec.def_name, COUNTER_DEF);

    let spec2 = def.to_spec();
    assert_eq!(spec2.def_name, COUNTER_DEF);
    assert!(spec.id != spec2.id);
}

// AgentStream

#[test]
fn test_agent_stream_add_agent() {
    let askit = ASKit::init().unwrap();

    let mut spec = AgentStreamSpec::default();
    assert_eq!(spec.agents.len(), 0);

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    let agent_spec = def.to_spec();

    spec.add_agent(agent_spec);

    assert_eq!(spec.agents.len(), 1);
}

#[test]
fn test_agent_stream_remove_agent() {
    let askit = ASKit::init().unwrap();

    let mut spec = AgentStreamSpec::default();
    assert_eq!(spec.agents.len(), 0);

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    let agent_spec = def.to_spec();
    let agent_id = agent_spec.id.clone();

    spec.add_agent(agent_spec);
    assert_eq!(spec.agents.len(), 1);

    spec.remove_agent(&agent_id);
    assert_eq!(spec.agents.len(), 0);
}
