extern crate modular_agent_core as ma;

use std::time::Duration;

use ma::test_utils;
use ma::{ModuleConfigs, ModuleStatus, Value};

use crate::common;
use common::modules::{CONFIG_VALUE, FailStartModule};

/// A module whose start() fails must end up exactly like a stopped one: back
/// in `Init`, without an inbox, so config edits are written to it directly
/// and stop_module skips stop().
#[tokio::test]
async fn failed_start_leaves_module_in_init_and_accepts_configs() {
    let ma = test_utils::setup_modular_agent().await;
    let patch_id = ma.new_patch().unwrap();
    let spec = ma.new_module_spec(FailStartModule::DEF_NAME).unwrap();
    let module_id = ma.add_module(patch_id.clone(), spec).await.unwrap();

    ma.start_patch(&patch_id).await.unwrap();
    // start() runs inside the spawned module loop.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let module = ma.get_module(&module_id).unwrap();
    assert_eq!(*module.lock().await.status(), ModuleStatus::Init);

    // With a stale inbox this would fail with SendMessageFailed instead of
    // being applied to the module.
    let mut configs = ModuleConfigs::new();
    configs.set(CONFIG_VALUE.into(), Value::string("x"));
    ma.set_module_configs(module_id.clone(), configs)
        .await
        .unwrap();
    assert_eq!(
        module
            .lock()
            .await
            .configs()
            .unwrap()
            .get(CONFIG_VALUE)
            .unwrap(),
        &Value::string("x")
    );

    // stop() returns Err, so Ok here proves it was skipped.
    ma.stop_module(&module_id).await.unwrap();
    // Init again, so the restart path is taken rather than a silent no-op.
    ma.start_module(&module_id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(*module.lock().await.status(), ModuleStatus::Init);

    ma.quit();
}
