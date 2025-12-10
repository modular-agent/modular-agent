extern crate agent_stream_kit as askit;

use askit::ASKit;

mod common;

const COUNTER_DEF: &str = common::CounterAgent::DEF_NAME;

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
        "askit_test::common::CounterAgent",
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

    let configs = askit.get_agent_default_configs(COUNTER_DEF).unwrap();
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
