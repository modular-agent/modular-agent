extern crate modular_agent_kit as mak;

use mak::MAK;

use crate::common;

const COUNTER_DEF: &str = common::agents::CounterAgent::DEF_NAME;

#[test]
fn test_init() {
    let mak = MAK::init().unwrap();

    let defs = mak.get_agent_definitions();
    assert_eq!(defs.len(), 10);
    let mut keys: Vec<_> = defs.keys().cloned().collect();
    keys.sort();
    let expected = vec![
        "main_test::common::agents::CounterAgent",
        "modular_agent_kit::board_agent::BoardInAgent",
        "modular_agent_kit::board_agent::BoardOutAgent",
        "modular_agent_kit::board_agent::VarInAgent",
        "modular_agent_kit::board_agent::VarOutAgent",
        "modular_agent_kit::test_utils::TestProbeAgent",
        "modular_agent_kit::tool::CallToolAgent",
        "modular_agent_kit::tool::CallToolMessageAgent",
        "modular_agent_kit::tool::ListToolsAgent",
        "modular_agent_kit::tool::PresetToolAgent",
    ];
    assert_eq!(keys, expected);

    mak.quit();
}

#[test]
fn test_agent_definition() {
    let mak = MAK::init().unwrap();

    let def = mak.get_agent_definition(COUNTER_DEF).unwrap();
    assert_eq!(def.name, COUNTER_DEF);

    mak.quit();
}

#[test]
fn test_agent_default_configs() {
    let mak = MAK::init().unwrap();

    let configs = mak.get_agent_config_specs(COUNTER_DEF).unwrap();
    assert_eq!(configs.len(), 1);
    assert!(configs.contains_key("initial_count"));

    mak.quit();
}

#[test]
fn test_global_configs() {
    let mak = MAK::init().unwrap();

    let gc = mak.get_global_configs(COUNTER_DEF).unwrap();
    assert_eq!(gc.get_string("global_string").unwrap(), "gs");

    mak.quit();
}

#[tokio::test]
async fn test_ready() {
    let mak = MAK::init().unwrap();
    mak.ready().await.unwrap();
    mak.quit();
}

#[tokio::test]
async fn test_add_agent() {
    let mak = MAK::init().unwrap();
    mak.ready().await.unwrap();

    let preset_id = mak.new_preset().unwrap();
    let def = mak.get_agent_definition(COUNTER_DEF).unwrap();
    let spec = def.to_spec();

    let agent_id = mak.add_agent(preset_id.clone(), spec).await.unwrap();
    let preset_spec = mak.get_preset_spec(&preset_id).await.unwrap();
    assert!(preset_spec.agents.iter().any(|a| a.id == agent_id));

    mak.quit();
}

#[tokio::test]
async fn test_remove_agent() {
    let mak = MAK::init().unwrap();
    mak.ready().await.unwrap();

    let preset_id = mak.new_preset().unwrap();
    let def = mak.get_agent_definition(COUNTER_DEF).unwrap();

    let spec = def.to_spec();
    let agent_id = mak.add_agent(preset_id.clone(), spec).await.unwrap();

    mak.remove_agent(&preset_id, &agent_id).await.unwrap();
    let preset_spec = mak.get_preset_spec(&preset_id).await.unwrap();
    assert!(!preset_spec.agents.iter().any(|a| a.id == agent_id));

    mak.quit();
}

#[tokio::test]
async fn test_remove_after_connect_agent() {
    let mak = MAK::init().unwrap();
    mak.ready().await.unwrap();

    let preset_id = mak.new_preset().unwrap();

    let def = mak.get_agent_definition(COUNTER_DEF).unwrap();

    let spec = def.to_spec();
    let agent1_id = mak.add_agent(preset_id.clone(), spec).await.unwrap();

    let spec = def.to_spec();
    let agent2_id = mak.add_agent(preset_id.clone(), spec).await.unwrap();

    let connection_spec = mak::ConnectionSpec {
        source: agent1_id.clone(),
        source_handle: "count".into(),
        target: agent2_id.clone(),
        target_handle: "in".into(),
    };

    mak.add_connection(&preset_id, connection_spec)
        .await
        .unwrap();

    mak.remove_agent(&preset_id, &agent1_id).await.unwrap();
    let preset_spec = mak.get_preset_spec(&preset_id).await.unwrap();
    assert!(!preset_spec.agents.iter().any(|a| a.id == agent1_id));

    mak.quit();
}
