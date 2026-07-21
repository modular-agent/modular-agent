extern crate modular_agent_core as ma;

use ma::ModularAgent;

use crate::common;

const COUNTER_DEF: &str = common::agents::CounterAgent::DEF_NAME;

#[test]
fn test_init() {
    let ma = ModularAgent::init().unwrap();

    let defs = ma.get_agent_definitions();
    assert_eq!(defs.len(), 13);
    let mut keys: Vec<_> = defs.keys().cloned().collect();
    keys.sort();
    let expected = vec![
        "main_test::common::agents::CancelWaitAgent",
        "main_test::common::agents::CounterAgent",
        "main_test::common::agents::StuckSleepAgent",
        "modular_agent_core::external_agent::ExternalInputAgent",
        "modular_agent_core::external_agent::ExternalOutputAgent",
        "modular_agent_core::external_agent::LocalInputAgent",
        "modular_agent_core::external_agent::LocalOutputAgent",
        "modular_agent_core::test_utils::TestProbeAgent",
        "modular_agent_core::tool::CallToolAgent",
        "modular_agent_core::tool::CallToolMessageAgent",
        "modular_agent_core::tool::ListToolsAgent",
        "modular_agent_core::tool::LoopControlAgent",
        "modular_agent_core::tool::PresetToolAgent",
    ];
    assert_eq!(keys, expected);

    ma.quit();
}

#[test]
fn test_agent_definition() {
    let ma = ModularAgent::init().unwrap();

    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    assert_eq!(def.name, COUNTER_DEF);

    ma.quit();
}

#[test]
fn test_agent_default_configs() {
    let ma = ModularAgent::init().unwrap();

    let configs = ma.get_agent_config_specs(COUNTER_DEF).unwrap();
    assert_eq!(configs.len(), 1);
    assert!(configs.contains_key("initial_count"));

    ma.quit();
}

#[test]
fn test_global_configs() {
    let ma = ModularAgent::init().unwrap();

    let gc = ma.get_global_configs(COUNTER_DEF).unwrap();
    assert_eq!(gc.get_string("global_string").unwrap(), "gs");

    ma.quit();
}

#[tokio::test]
async fn test_ready() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();
    ma.quit();
}

#[tokio::test]
async fn test_add_agent() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    let spec = def.to_spec();

    let agent_id = ma.add_agent(preset_id.clone(), spec).await.unwrap();
    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    assert!(preset_spec.agents.iter().any(|a| a.id == agent_id));

    ma.quit();
}

#[tokio::test]
async fn test_remove_agent() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();

    let spec = def.to_spec();
    let agent_id = ma.add_agent(preset_id.clone(), spec).await.unwrap();

    ma.remove_agent(&preset_id, &agent_id).await.unwrap();
    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    assert!(!preset_spec.agents.iter().any(|a| a.id == agent_id));

    ma.quit();
}

#[tokio::test]
async fn test_remove_after_connect_agent() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();

    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();

    let spec = def.to_spec();
    let agent1_id = ma.add_agent(preset_id.clone(), spec).await.unwrap();

    let spec = def.to_spec();
    let agent2_id = ma.add_agent(preset_id.clone(), spec).await.unwrap();

    let connection_spec = ma::ConnectionSpec {
        source: agent1_id.clone(),
        source_handle: "count".into(),
        target: agent2_id.clone(),
        target_handle: "in".into(),
    };

    ma.add_connection(&preset_id, connection_spec)
        .await
        .unwrap();

    ma.remove_agent(&preset_id, &agent1_id).await.unwrap();
    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    assert!(!preset_spec.agents.iter().any(|a| a.id == agent1_id));

    ma.quit();
}

#[tokio::test]
async fn test_duplicate_connection_leaves_spec_unchanged() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    let agent1_id = ma
        .add_agent(preset_id.clone(), def.to_spec())
        .await
        .unwrap();
    let agent2_id = ma
        .add_agent(preset_id.clone(), def.to_spec())
        .await
        .unwrap();

    let connection = ma::ConnectionSpec {
        source: agent1_id.clone(),
        source_handle: "count".into(),
        target: agent2_id.clone(),
        target_handle: "in".into(),
    };
    ma.add_connection(&preset_id, connection.clone())
        .await
        .unwrap();

    let err = ma.add_connection(&preset_id, connection).await.unwrap_err();
    assert!(matches!(err, ma::AgentError::ConnectionAlreadyExists));

    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    assert_eq!(preset_spec.connections.len(), 1);

    ma.quit();
}

#[tokio::test]
async fn test_remove_spec_only_agent() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    // An agent whose definition is unknown ends up in the spec without a
    // runtime instance; it must still be removable.
    let spec = ma::PresetSpec {
        agents: vec![ma::AgentSpec {
            id: "orphan".into(),
            def_name: "no_such::Definition".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let preset_id = ma.add_preset(spec).unwrap();

    // get_preset_spec hides spec-only agents (it keeps live instances
    // only), so read the raw spec through the preset itself.
    let preset = ma.get_preset(&preset_id).unwrap();
    let orphan_id = preset.lock().await.spec().agents[0].id.clone();

    ma.remove_agent(&preset_id, &orphan_id).await.unwrap();
    assert!(preset.lock().await.spec().agents.is_empty());

    // An agent in neither the runtime nor the spec is still an error.
    let err = ma.remove_agent(&preset_id, "missing").await.unwrap_err();
    assert!(matches!(err, ma::AgentError::AgentNotFound(_)));

    ma.quit();
}

#[tokio::test]
async fn test_failed_batch_add_rolls_back() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();

    let agents = vec![
        def.to_spec(),
        ma::AgentSpec {
            def_name: "no_such::Definition".into(),
            ..Default::default()
        },
    ];
    let err = ma
        .add_agents_and_connections(&preset_id, &agents, &vec![])
        .await
        .unwrap_err();
    assert!(matches!(err, ma::AgentError::UnknownDefName(_)));

    // The valid first agent must not survive the failed batch.
    let preset = ma.get_preset(&preset_id).unwrap();
    assert!(preset.lock().await.spec().agents.is_empty());

    ma.quit();
}
