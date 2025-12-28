extern crate agent_stream_kit as askit;

use askit::ASKit;

mod common;

const COUNTER_DEF: &str = common::agents::CounterAgent::DEF_NAME;

#[test]
fn test_init() {
    let askit = ASKit::init().unwrap();

    let defs = askit.get_agent_definitions();
    assert_eq!(defs.len(), 6);
    let mut keys: Vec<_> = defs.keys().cloned().collect();
    keys.sort();
    let expected = vec![
        "agent_stream_kit::board_agent::BoardInAgent",
        "agent_stream_kit::board_agent::BoardOutAgent",
        "agent_stream_kit::board_agent::VarInAgent",
        "agent_stream_kit::board_agent::VarOutAgent",
        "agent_stream_kit::test_utils::TestProbeAgent",
        "askit_test::common::agents::CounterAgent",
    ];
    assert_eq!(keys, expected);

    askit.quit();
}

#[test]
fn test_agent_definition() {
    let askit = ASKit::init().unwrap();

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    assert_eq!(def.name, COUNTER_DEF);

    askit.quit();
}

#[test]
fn test_agent_default_configs() {
    let askit = ASKit::init().unwrap();

    let configs = askit.get_agent_config_specs(COUNTER_DEF).unwrap();
    assert_eq!(configs.len(), 1);
    assert!(configs.contains_key("initial_count"));

    askit.quit();
}

#[test]
fn test_global_configs() {
    let askit = ASKit::init().unwrap();

    let gc = askit.get_global_configs(COUNTER_DEF).unwrap();
    assert_eq!(gc.get_string("global_string").unwrap(), "gs");

    askit.quit();
}

#[tokio::test]
async fn test_ready() {
    let askit = ASKit::init().unwrap();
    askit.ready().await.unwrap();
    askit.quit();
}

#[tokio::test]
async fn test_add_agent() {
    let askit = ASKit::init().unwrap();
    askit.ready().await.unwrap();

    let stream_id = askit.new_agent_stream("s1").unwrap();
    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    let spec = askit::AgentSpec::from_def(&def);
    let agent_id = spec.id.clone();

    askit.add_agent(stream_id.clone(), spec).unwrap();
    let stream_spec = askit.get_agent_stream_spec(&stream_id).unwrap();
    assert!(stream_spec.agents.iter().any(|a| a.id == agent_id));

    askit.quit();
}

#[tokio::test]
async fn test_remove_agent() {
    let askit = ASKit::init().unwrap();
    askit.ready().await.unwrap();

    let stream_id = askit.new_agent_stream("s1").unwrap();
    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();

    let spec = askit::AgentSpec::from_def(&def);
    let agent_id = spec.id.clone();
    askit.add_agent(stream_id.clone(), spec).unwrap();

    askit.remove_agent(&stream_id, &agent_id).await.unwrap();
    let stream_spec = askit.get_agent_stream_spec(&stream_id).unwrap();
    assert!(!stream_spec.agents.iter().any(|a| a.id == agent_id));

    askit.quit();
}

#[tokio::test]
async fn test_remove_after_connect_agent() {
    let askit = ASKit::init().unwrap();
    askit.ready().await.unwrap();

    let stream_id = askit.new_agent_stream("s1").unwrap();

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();

    let spec = askit::AgentSpec::from_def(&def);
    let agent1_id = spec.id.clone();
    askit.add_agent(stream_id.clone(), spec).unwrap();

    let spec = askit::AgentSpec::from_def(&def);
    let agent2_id = spec.id.clone();
    askit.add_agent(stream_id.clone(), spec).unwrap();

    let channel_spec = askit::ChannelSpec {
        source: agent1_id.clone(),
        source_handle: "count".into(),
        target: agent2_id.clone(),
        target_handle: "in".into(),
    };

    askit.add_channel(&stream_id, channel_spec).unwrap();

    askit.remove_agent(&stream_id, &agent1_id).await.unwrap();
    let stream_spec = askit.get_agent_stream_spec(&stream_id).unwrap();
    assert!(!stream_spec.agents.iter().any(|a| a.id == agent1_id));

    askit.quit();
}
