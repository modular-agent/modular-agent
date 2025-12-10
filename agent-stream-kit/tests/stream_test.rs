extern crate agent_stream_kit as askit;

mod common;

use askit::{ASKit, AgentStream, AgentStreamNode};

const COUNTER_DEF: &str = concat!(module_path!(), "::common::CounterAgent");

// AgentStreamNode

#[test]
fn test_agent_stream_node_new() {
    let askit = ASKit::init().unwrap();

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();

    let node = AgentStreamNode::new(&def).unwrap();

    assert_eq!(node.spec.def_name, COUNTER_DEF);
    assert!(!node.enabled);

    let node2 = AgentStreamNode::new(&def).unwrap();
    assert_eq!(node2.spec.def_name, COUNTER_DEF);
    assert!(node.id != node2.id);
    assert!(!node2.enabled);
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
    assert_eq!(stream.nodes().len(), 0);

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    let node = AgentStreamNode::new(&def).unwrap();

    stream.add_node(node);

    assert_eq!(stream.nodes().len(), 1);
}

#[test]
fn test_agent_stream_remove_agent() {
    let askit = ASKit::init().unwrap();

    let mut stream = AgentStream::new("test_stream".into());
    assert_eq!(stream.nodes().len(), 0);

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    let node = AgentStreamNode::new(&def).unwrap();

    let node_id = node.id.clone();

    stream.add_node(node);
    assert_eq!(stream.nodes().len(), 1);

    stream.remove_node(&node_id);
    assert_eq!(stream.nodes().len(), 0);
}
