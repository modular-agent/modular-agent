extern crate modular_agent_core as ma;

use ma::ModularAgent;

use crate::common;

const COUNTER_DEF: &str = common::agents::CounterAgent::DEF_NAME;

#[test]
fn test_init() {
    let ma = ModularAgent::init().unwrap();

    let defs = ma.get_agent_definitions();
    assert_eq!(defs.len(), 15);
    let mut keys: Vec<_> = defs.keys().cloned().collect();
    keys.sort();
    let expected = vec![
        "main_test::common::agents::CancelWaitAgent",
        "main_test::common::agents::CounterAgent",
        "main_test::common::agents::DynSpecAgent",
        "main_test::common::agents::NumberedConfigAgent",
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

    // get_preset_spec exposes spec-only agents, so the editor can see and
    // address them like any other agent.
    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    let orphan_id = preset_spec.agents[0].id.clone();

    ma.remove_agent(&preset_id, &orphan_id).await.unwrap();
    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    assert!(preset_spec.agents.is_empty());

    // An agent in neither the runtime nor the spec is still an error.
    let err = ma.remove_agent(&preset_id, "missing").await.unwrap_err();
    assert!(matches!(err, ma::AgentError::AgentNotFound(_)));

    ma.quit();
}

#[tokio::test]
async fn test_spec_only_agent_survives_preset_spec_and_updates() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let mut orphan_configs = ma::AgentConfigs::default();
    orphan_configs.set("channel".into(), ma::AgentValue::string("general"));
    orphan_configs.set("token".into(), ma::AgentValue::string("secret"));

    let spec = ma::PresetSpec {
        agents: vec![
            ma::AgentSpec {
                id: "orphan".into(),
                def_name: "no_such::Definition".into(),
                outputs: Some(vec!["message".into()]),
                configs: Some(orphan_configs),
                extensions: [("x".to_string(), serde_json::json!(10))]
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
            ma::AgentSpec {
                id: "counter".into(),
                def_name: COUNTER_DEF.into(),
                ..Default::default()
            },
        ],
        connections: vec![ma::ConnectionSpec {
            source: "orphan".into(),
            source_handle: "message".into(),
            target: "counter".into(),
            target_handle: "in".into(),
        }],
        ..Default::default()
    };
    let preset_id = ma.add_preset(spec).unwrap();

    // The unknown definition has no live instance, but the stored entry must
    // still be reported - save_preset writes whatever this returns.
    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    assert_eq!(preset_spec.agents.len(), 2);
    assert_eq!(preset_spec.connections.len(), 1);
    let orphan_id = preset_spec.agents[0].id.clone();
    assert_eq!(preset_spec.agents[0].def_name, "no_such::Definition");

    ma.update_agent_spec(&orphan_id, &serde_json::json!({ "x": 42, "color": 3 }))
        .await
        .unwrap();

    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    let orphan = &preset_spec.agents[0];
    assert_eq!(orphan.extensions.get("x"), Some(&serde_json::json!(42)));
    assert_eq!(orphan.extensions.get("color"), Some(&serde_json::json!(3)));
    assert_eq!(
        orphan.outputs.as_deref(),
        Some(["message".to_string()].as_slice())
    );

    // The other half of the overlay: a live agent's patch lands only on the
    // instance, never on the stored entry, so get_preset_spec must reflect
    // the instance spec for live agents.
    let counter_id = preset_spec
        .agents
        .iter()
        .find(|a| a.def_name == COUNTER_DEF)
        .unwrap()
        .id
        .clone();
    ma.update_agent_spec(&counter_id, &serde_json::json!({ "x": 42 }))
        .await
        .unwrap();
    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    let counter = preset_spec
        .agents
        .iter()
        .find(|a| a.id == counter_id)
        .unwrap();
    assert_eq!(counter.extensions.get("x"), Some(&serde_json::json!(42)));

    let mut new_configs = ma::AgentConfigs::default();
    new_configs.set("channel".into(), ma::AgentValue::string("random"));
    ma.set_agent_configs(orphan_id.clone(), new_configs)
        .await
        .unwrap();

    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    let configs = preset_spec.agents[0].configs.as_ref().unwrap();
    assert_eq!(configs.get_string("channel").unwrap(), "random");
    // Setting one key must merge, not replace: the untouched key survives.
    assert_eq!(configs.get_string("token").unwrap(), "secret");

    // An id in neither the runtime nor any preset spec is still an error.
    let err = ma
        .update_agent_spec("missing", &serde_json::json!({ "x": 1 }))
        .await
        .unwrap_err();
    assert!(matches!(err, ma::AgentError::AgentNotFound(_)));

    ma.quit();
}

#[tokio::test]
async fn test_add_agent_registers_constructed_spec() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let def = ma
        .get_agent_definition(common::agents::DynSpecAgent::DEF_NAME)
        .unwrap();

    let agent_id = ma
        .add_agent(preset_id.clone(), def.to_spec())
        .await
        .unwrap();

    // The raw preset spec (not overlaid with live agent specs) must contain
    // the config and port that new() generated.
    let preset = ma.get_preset(&preset_id).unwrap();
    let registered = {
        let preset = preset.lock().await;
        preset
            .spec()
            .agents
            .iter()
            .find(|a| a.id == agent_id)
            .cloned()
            .unwrap()
    };
    let configs = registered.configs.expect("configs must be present");
    assert!(configs.contains_key(common::agents::CONFIG_DYN));
    let outputs = registered.outputs.expect("outputs must be present");
    assert!(outputs.iter().any(|p| p == common::agents::PORT_DYN_OUT));

    // get_preset_spec (save path) must expose them as well.
    let preset_spec = ma.get_preset_spec(&preset_id).await.unwrap();
    let saved = preset_spec
        .agents
        .iter()
        .find(|a| a.id == agent_id)
        .unwrap();
    let configs = saved.configs.as_ref().expect("configs must be present");
    assert!(configs.contains_key(common::agents::CONFIG_DYN));

    ma.quit();
}

#[tokio::test]
async fn test_add_agents_and_connections_returns_constructed_specs() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let def = ma
        .get_agent_definition(common::agents::DynSpecAgent::DEF_NAME)
        .unwrap();

    let (added, _) = ma
        .add_agents_and_connections(&preset_id, &vec![def.to_spec()], &vec![])
        .await
        .unwrap();
    assert_eq!(added.len(), 1);

    // The returned spec must be the constructed one, including the config
    // and port that new() generated.
    let configs = added[0].configs.as_ref().expect("configs must be present");
    assert!(configs.contains_key(common::agents::CONFIG_DYN));
    let outputs = added[0].outputs.as_ref().expect("outputs must be present");
    assert!(outputs.iter().any(|p| p == common::agents::PORT_DYN_OUT));

    // The preset must have registered the constructed spec as well.
    let preset = ma.get_preset(&preset_id).unwrap();
    let registered = {
        let preset = preset.lock().await;
        preset
            .spec()
            .agents
            .iter()
            .find(|a| a.id == added[0].id)
            .cloned()
            .unwrap()
    };
    let configs = registered.configs.expect("configs must be present");
    assert!(configs.contains_key(common::agents::CONFIG_DYN));

    ma.quit();
}

#[tokio::test]
async fn test_update_agent_spec_configs_regenerates_dynamic_spec() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let def = ma
        .get_agent_definition(common::agents::NumberedConfigAgent::DEF_NAME)
        .unwrap();
    let agent_id = ma
        .add_agent(preset_id.clone(), def.to_spec())
        .await
        .unwrap();

    ma.update_agent_spec(&agent_id, &serde_json::json!({ "configs": { "n": 3 } }))
        .await
        .unwrap();

    // A configs patch must run configs_changed(), which is what grows the
    // third condition and the third output port.
    let spec = ma.get_agent_spec(&agent_id).await.unwrap();
    let configs = spec.configs.expect("configs must be present");
    assert!(configs.contains_key("c2"));
    let outputs = spec.outputs.expect("outputs must be present");
    assert!(outputs.iter().any(|p| p == "2"));

    ma.quit();
}

#[tokio::test]
async fn test_set_agent_configs_accepts_generated_key() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let def = ma
        .get_agent_definition(common::agents::NumberedConfigAgent::DEF_NAME)
        .unwrap();

    let mut spec = def.to_spec();
    let mut configs = spec.configs.take().unwrap();
    configs.set(common::agents::CONFIG_N.into(), ma::AgentValue::integer(3));
    spec.configs = Some(configs);
    let agent_id = ma.add_agent(preset_id.clone(), spec).await.unwrap();

    let created = ma.get_agent_spec(&agent_id).await.unwrap();
    let mut configs = created.configs.expect("configs must be present");
    assert!(
        configs.contains_key("c2"),
        "new() must generate c2 from n=3"
    );

    configs.set("c2".into(), ma::AgentValue::string("hello"));
    ma.set_agent_configs(agent_id.clone(), configs)
        .await
        .unwrap();

    let spec = ma.get_agent_spec(&agent_id).await.unwrap();
    let configs = spec.configs.expect("configs must be present");
    assert_eq!(configs.get_string("c2").unwrap(), "hello");

    ma.quit();
}

#[tokio::test]
async fn test_numbered_config_restores_parked_stale_key() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let def = ma
        .get_agent_definition(common::agents::NumberedConfigAgent::DEF_NAME)
        .unwrap();

    // reconcile_spec parks configs the definition does not declare under a
    // "_" prefix when a preset is loaded; new() must pick the value back up.
    let mut spec = def.to_spec();
    let mut configs = spec.configs.take().unwrap();
    configs.set(common::agents::CONFIG_N.into(), ma::AgentValue::integer(3));
    configs.set("_c2".into(), ma::AgentValue::string("parked"));
    spec.configs = Some(configs);
    let agent_id = ma.add_agent(preset_id.clone(), spec).await.unwrap();

    let created = ma.get_agent_spec(&agent_id).await.unwrap();
    let configs = created.configs.expect("configs must be present");
    assert_eq!(configs.get_string("c2").unwrap(), "parked");
    assert!(!configs.contains_key("_c2"), "the parked key must be gone");

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
