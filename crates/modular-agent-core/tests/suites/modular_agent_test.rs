extern crate modular_agent_core as ma;

use ma::ModularAgent;

use crate::common;

const COUNTER_DEF: &str = common::modules::CounterModule::DEF_NAME;

#[test]
fn test_init() {
    let ma = ModularAgent::init().unwrap();

    let defs = ma.get_module_definitions();
    assert_eq!(defs.len(), 16);
    let mut keys: Vec<_> = defs.keys().cloned().collect();
    keys.sort();
    let expected = vec![
        "main_test::common::modules::CancelWaitModule",
        "main_test::common::modules::CounterModule",
        "main_test::common::modules::DynSpecModule",
        "main_test::common::modules::NumberedConfigModule",
        "main_test::common::modules::PendingStopModule",
        "main_test::common::modules::StuckSleepModule",
        "modular_agent_core::external_module::ExternalInputModule",
        "modular_agent_core::external_module::ExternalOutputModule",
        "modular_agent_core::external_module::LocalInputModule",
        "modular_agent_core::external_module::LocalOutputModule",
        "modular_agent_core::test_utils::TestProbeModule",
        "modular_agent_core::tool::CallToolMessageModule",
        "modular_agent_core::tool::CallToolModule",
        "modular_agent_core::tool::CustomToolModule",
        "modular_agent_core::tool::ListToolsModule",
        "modular_agent_core::tool::LoopControlModule",
    ];
    assert_eq!(keys, expected);

    ma.quit();
}

#[test]
fn test_module_definition() {
    let ma = ModularAgent::init().unwrap();

    let def = ma.get_module_definition(COUNTER_DEF).unwrap();
    assert_eq!(def.name, COUNTER_DEF);

    ma.quit();
}

#[test]
fn test_module_default_configs() {
    let ma = ModularAgent::init().unwrap();

    let configs = ma.get_module_config_specs(COUNTER_DEF).unwrap();
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
async fn test_add_module() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma.get_module_definition(COUNTER_DEF).unwrap();
    let spec = def.to_spec();

    let module_id = ma.add_module(patch_id.clone(), spec).await.unwrap();
    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    assert!(patch_spec.modules.iter().any(|a| a.id == module_id));

    ma.quit();
}

#[tokio::test]
async fn test_remove_module() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma.get_module_definition(COUNTER_DEF).unwrap();

    let spec = def.to_spec();
    let module_id = ma.add_module(patch_id.clone(), spec).await.unwrap();

    ma.remove_module(&patch_id, &module_id).await.unwrap();
    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    assert!(!patch_spec.modules.iter().any(|a| a.id == module_id));

    ma.quit();
}

#[tokio::test]
async fn test_remove_after_connect_module() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();

    let def = ma.get_module_definition(COUNTER_DEF).unwrap();

    let spec = def.to_spec();
    let agent1_id = ma.add_module(patch_id.clone(), spec).await.unwrap();

    let spec = def.to_spec();
    let agent2_id = ma.add_module(patch_id.clone(), spec).await.unwrap();

    let connection_spec = ma::ConnectionSpec {
        source: agent1_id.clone(),
        source_handle: "count".into(),
        target: agent2_id.clone(),
        target_handle: "in".into(),
    };

    ma.add_connection(&patch_id, connection_spec).await.unwrap();

    ma.remove_module(&patch_id, &agent1_id).await.unwrap();
    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    assert!(!patch_spec.modules.iter().any(|a| a.id == agent1_id));

    ma.quit();
}

#[tokio::test]
async fn test_duplicate_connection_leaves_spec_unchanged() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma.get_module_definition(COUNTER_DEF).unwrap();
    let agent1_id = ma
        .add_module(patch_id.clone(), def.to_spec())
        .await
        .unwrap();
    let agent2_id = ma
        .add_module(patch_id.clone(), def.to_spec())
        .await
        .unwrap();

    let connection = ma::ConnectionSpec {
        source: agent1_id.clone(),
        source_handle: "count".into(),
        target: agent2_id.clone(),
        target_handle: "in".into(),
    };
    ma.add_connection(&patch_id, connection.clone())
        .await
        .unwrap();

    let err = ma.add_connection(&patch_id, connection).await.unwrap_err();
    assert!(matches!(err, ma::Error::ConnectionAlreadyExists));

    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    assert_eq!(patch_spec.connections.len(), 1);

    ma.quit();
}

#[tokio::test]
async fn test_remove_spec_only_module() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    // A module whose definition is unknown ends up in the spec without a
    // runtime instance; it must still be removable.
    let spec = ma::PatchSpec {
        modules: vec![ma::ModuleSpec {
            id: "orphan".into(),
            def_name: "no_such::Definition".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let patch_id = ma.add_patch(spec).unwrap();

    // get_patch_spec exposes spec-only modules, so the editor can see and
    // address them like any other module.
    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    let orphan_id = patch_spec.modules[0].id.clone();

    ma.remove_module(&patch_id, &orphan_id).await.unwrap();
    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    assert!(patch_spec.modules.is_empty());

    // A module in neither the runtime nor the spec is still an error.
    let err = ma.remove_module(&patch_id, "missing").await.unwrap_err();
    assert!(matches!(err, ma::Error::ModuleNotFound(_)));

    ma.quit();
}

#[tokio::test]
async fn test_spec_only_module_survives_patch_spec_and_updates() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let mut orphan_configs = ma::ModuleConfigs::default();
    orphan_configs.set("channel".into(), ma::Value::string("general"));
    orphan_configs.set("token".into(), ma::Value::string("secret"));

    let spec = ma::PatchSpec {
        modules: vec![
            ma::ModuleSpec {
                id: "orphan".into(),
                def_name: "no_such::Definition".into(),
                outputs: Some(vec!["message".into()]),
                configs: Some(orphan_configs),
                extensions: [("x".to_string(), serde_json::json!(10))]
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
            ma::ModuleSpec {
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
    let patch_id = ma.add_patch(spec).unwrap();

    // The unknown definition has no live instance, but the stored entry must
    // still be reported - save_patch writes whatever this returns.
    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    assert_eq!(patch_spec.modules.len(), 2);
    assert_eq!(patch_spec.connections.len(), 1);
    let orphan_id = patch_spec.modules[0].id.clone();
    assert_eq!(patch_spec.modules[0].def_name, "no_such::Definition");

    ma.update_module_spec(&orphan_id, &serde_json::json!({ "x": 42, "color": 3 }))
        .await
        .unwrap();

    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    let orphan = &patch_spec.modules[0];
    assert_eq!(orphan.extensions.get("x"), Some(&serde_json::json!(42)));
    assert_eq!(orphan.extensions.get("color"), Some(&serde_json::json!(3)));
    assert_eq!(
        orphan.outputs.as_deref(),
        Some(["message".to_string()].as_slice())
    );

    // The other half of the overlay: a live module's patch lands only on the
    // instance, never on the stored entry, so get_patch_spec must reflect
    // the instance spec for live modules.
    let counter_id = patch_spec
        .modules
        .iter()
        .find(|a| a.def_name == COUNTER_DEF)
        .unwrap()
        .id
        .clone();
    ma.update_module_spec(&counter_id, &serde_json::json!({ "x": 42 }))
        .await
        .unwrap();
    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    let counter = patch_spec
        .modules
        .iter()
        .find(|a| a.id == counter_id)
        .unwrap();
    assert_eq!(counter.extensions.get("x"), Some(&serde_json::json!(42)));

    let mut new_configs = ma::ModuleConfigs::default();
    new_configs.set("channel".into(), ma::Value::string("random"));
    ma.set_module_configs(orphan_id.clone(), new_configs)
        .await
        .unwrap();

    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    let configs = patch_spec.modules[0].configs.as_ref().unwrap();
    assert_eq!(configs.get_string("channel").unwrap(), "random");
    // Setting one key must merge, not replace: the untouched key survives.
    assert_eq!(configs.get_string("token").unwrap(), "secret");

    // An id in neither the runtime nor any patch spec is still an error.
    let err = ma
        .update_module_spec("missing", &serde_json::json!({ "x": 1 }))
        .await
        .unwrap_err();
    assert!(matches!(err, ma::Error::ModuleNotFound(_)));

    ma.quit();
}

#[tokio::test]
async fn test_add_module_registers_constructed_spec() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma
        .get_module_definition(common::modules::DynSpecModule::DEF_NAME)
        .unwrap();

    let module_id = ma
        .add_module(patch_id.clone(), def.to_spec())
        .await
        .unwrap();

    // The raw patch spec (not overlaid with live module specs) must contain
    // the config and port that new() generated.
    let patch = ma.get_patch(&patch_id).unwrap();
    let registered = {
        let patch = patch.lock().await;
        patch
            .spec()
            .modules
            .iter()
            .find(|a| a.id == module_id)
            .cloned()
            .unwrap()
    };
    let configs = registered.configs.expect("configs must be present");
    assert!(configs.contains_key(common::modules::CONFIG_DYN));
    let outputs = registered.outputs.expect("outputs must be present");
    assert!(outputs.iter().any(|p| p == common::modules::PORT_DYN_OUT));

    // get_patch_spec (save path) must expose them as well.
    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    let saved = patch_spec
        .modules
        .iter()
        .find(|a| a.id == module_id)
        .unwrap();
    let configs = saved.configs.as_ref().expect("configs must be present");
    assert!(configs.contains_key(common::modules::CONFIG_DYN));

    ma.quit();
}

#[tokio::test]
async fn test_add_modules_and_connections_returns_constructed_specs() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma
        .get_module_definition(common::modules::DynSpecModule::DEF_NAME)
        .unwrap();

    let (added, _) = ma
        .add_modules_and_connections(&patch_id, &vec![def.to_spec()], &vec![])
        .await
        .unwrap();
    assert_eq!(added.len(), 1);

    // The returned spec must be the constructed one, including the config
    // and port that new() generated.
    let configs = added[0].configs.as_ref().expect("configs must be present");
    assert!(configs.contains_key(common::modules::CONFIG_DYN));
    let outputs = added[0].outputs.as_ref().expect("outputs must be present");
    assert!(outputs.iter().any(|p| p == common::modules::PORT_DYN_OUT));

    // The patch must have registered the constructed spec as well.
    let patch = ma.get_patch(&patch_id).unwrap();
    let registered = {
        let patch = patch.lock().await;
        patch
            .spec()
            .modules
            .iter()
            .find(|a| a.id == added[0].id)
            .cloned()
            .unwrap()
    };
    let configs = registered.configs.expect("configs must be present");
    assert!(configs.contains_key(common::modules::CONFIG_DYN));

    ma.quit();
}

#[tokio::test]
async fn test_update_module_spec_configs_regenerates_dynamic_spec() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma
        .get_module_definition(common::modules::NumberedConfigModule::DEF_NAME)
        .unwrap();
    let module_id = ma
        .add_module(patch_id.clone(), def.to_spec())
        .await
        .unwrap();

    ma.update_module_spec(&module_id, &serde_json::json!({ "configs": { "n": 3 } }))
        .await
        .unwrap();

    // A configs patch must run configs_changed(), which is what grows the
    // third condition and the third output port.
    let spec = ma.get_module_spec(&module_id).await.unwrap();
    let configs = spec.configs.expect("configs must be present");
    assert!(configs.contains_key("c2"));
    let outputs = spec.outputs.expect("outputs must be present");
    assert!(outputs.iter().any(|p| p == "2"));

    ma.quit();
}

#[tokio::test]
async fn test_set_module_configs_accepts_generated_key() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma
        .get_module_definition(common::modules::NumberedConfigModule::DEF_NAME)
        .unwrap();

    let mut spec = def.to_spec();
    let mut configs = spec.configs.take().unwrap();
    configs.set(common::modules::CONFIG_N.into(), ma::Value::integer(3));
    spec.configs = Some(configs);
    let module_id = ma.add_module(patch_id.clone(), spec).await.unwrap();

    let created = ma.get_module_spec(&module_id).await.unwrap();
    let mut configs = created.configs.expect("configs must be present");
    assert!(
        configs.contains_key("c2"),
        "new() must generate c2 from n=3"
    );

    configs.set("c2".into(), ma::Value::string("hello"));
    ma.set_module_configs(module_id.clone(), configs)
        .await
        .unwrap();

    let spec = ma.get_module_spec(&module_id).await.unwrap();
    let configs = spec.configs.expect("configs must be present");
    assert_eq!(configs.get_string("c2").unwrap(), "hello");

    ma.quit();
}

#[tokio::test]
async fn test_numbered_config_restores_parked_stale_key() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma
        .get_module_definition(common::modules::NumberedConfigModule::DEF_NAME)
        .unwrap();

    // reconcile_spec parks configs the definition does not declare under a
    // "_" prefix when a patch is loaded; new() must pick the value back up.
    let mut spec = def.to_spec();
    let mut configs = spec.configs.take().unwrap();
    configs.set(common::modules::CONFIG_N.into(), ma::Value::integer(3));
    configs.set("_c2".into(), ma::Value::string("parked"));
    spec.configs = Some(configs);
    let module_id = ma.add_module(patch_id.clone(), spec).await.unwrap();

    let created = ma.get_module_spec(&module_id).await.unwrap();
    let configs = created.configs.expect("configs must be present");
    assert_eq!(configs.get_string("c2").unwrap(), "parked");
    assert!(!configs.contains_key("_c2"), "the parked key must be gone");

    ma.quit();
}

#[tokio::test]
async fn test_failed_batch_add_rolls_back() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma.get_module_definition(COUNTER_DEF).unwrap();

    let modules = vec![
        def.to_spec(),
        ma::ModuleSpec {
            def_name: "no_such::Definition".into(),
            ..Default::default()
        },
    ];
    let err = ma
        .add_modules_and_connections(&patch_id, &modules, &vec![])
        .await
        .unwrap_err();
    assert!(matches!(err, ma::Error::UnknownDefName(_)));

    // The valid first module must not survive the failed batch.
    let patch = ma.get_patch(&patch_id).unwrap();
    assert!(patch.lock().await.spec().modules.is_empty());

    ma.quit();
}
