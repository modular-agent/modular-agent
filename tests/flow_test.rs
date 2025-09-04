extern crate agent_stream_kit as askit;

mod common;

use askit::{ASKit, AgentFlow, AgentFlowNode};
use common::register_agents;

// AgentFlowNode

#[test]
fn test_agent_flow_node_new() {
    let askit = ASKit::init().unwrap();
    register_agents(&askit);

    let def = askit.get_agent_definition("$counter").unwrap();

    let node = AgentFlowNode::new(&def).unwrap();

    assert_eq!(node.def_name, "$counter");
    assert!(!node.enabled);

    let node2 = AgentFlowNode::new(&def).unwrap();
    assert_eq!(node2.def_name, "$counter");
    assert!(node.id != node2.id);
    assert!(!node2.enabled);
}

// AgentFlow

#[test]
fn test_agent_flow_new() {
    let flow = AgentFlow::new("test_flow".into());

    assert_eq!(flow.name(), "test_flow");
}

#[test]
fn test_agent_flow_rename() {
    let mut flow = AgentFlow::new("test_flow".into());

    flow.set_name("new_flow_name".into());
    assert_eq!(flow.name(), "new_flow_name");
}

#[test]
fn test_agent_flow_add_agent() {
    let askit = ASKit::init().unwrap();
    register_agents(&askit);

    let mut flow = AgentFlow::new("test_flow".into());
    assert_eq!(flow.nodes().len(), 0);

    let def = askit.get_agent_definition("$counter").unwrap();
    let node = AgentFlowNode::new(&def).unwrap();

    flow.add_node(node);

    assert_eq!(flow.nodes().len(), 1);
}

#[test]
fn test_agent_flow_remove_agent() {
    let askit = ASKit::init().unwrap();
    register_agents(&askit);

    let mut flow = AgentFlow::new("test_flow".into());
    assert_eq!(flow.nodes().len(), 0);

    let def = askit.get_agent_definition("$counter").unwrap();
    let node = AgentFlowNode::new(&def).unwrap();

    let node_id = node.id.clone();

    flow.add_node(node);
    assert_eq!(flow.nodes().len(), 1);

    flow.remove_node(&node_id);
    assert_eq!(flow.nodes().len(), 0);
}
