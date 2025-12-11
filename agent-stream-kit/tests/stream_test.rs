extern crate agent_stream_kit as askit;

mod common;

use askit::{ASKit, AgentSpec, AgentStream};

const COUNTER_DEF: &str = concat!(module_path!(), "::common::CounterAgent");

// AgentStreamNode

#[test]
fn test_agent_spec_from_def() {
    let askit = ASKit::init().unwrap();

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();

    let spec = AgentSpec::from_def(&def);

    assert_eq!(spec.def_name, COUNTER_DEF);
    assert!(!spec.enabled);

    let spec2 = AgentSpec::from_def(&def);
    assert_eq!(spec2.def_name, COUNTER_DEF);
    assert!(spec.id != spec2.id);
    assert!(!spec2.enabled);
}

// AgentStream

#[test]
fn test_agent_stream_new() {
    let stream = AgentStream::new("test_stream".into());

    assert_eq!(stream.name(), "test_stream");
}

#[test]
fn test_agent_stream_rename() {
    let mut stream = AgentStream::new("test_stream".into());

    stream.set_name("new_stream_name".into());
    assert_eq!(stream.name(), "new_stream_name");
}

#[test]
fn test_agent_stream_add_agent() {
    let askit = ASKit::init().unwrap();

    let mut stream = AgentStream::new("test_stream".into());
    assert_eq!(stream.agents().len(), 0);

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    let spec = AgentSpec::from_def(&def);

    stream.add_agent(spec);

    assert_eq!(stream.agents().len(), 1);
}

#[test]
fn test_agent_stream_remove_agent() {
    let askit = ASKit::init().unwrap();

    let mut stream = AgentStream::new("test_stream".into());
    assert_eq!(stream.agents().len(), 0);

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    let spec = AgentSpec::from_def(&def);

    let spec_id = spec.id.clone();

    stream.add_agent(spec);
    assert_eq!(stream.agents().len(), 1);

    stream.remove_agent(&spec_id);
    assert_eq!(stream.agents().len(), 0);
}
