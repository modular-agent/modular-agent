extern crate modular_agent_core as ma;

use std::time::Duration;

use ma::test_utils;
use ma::{ModularAgentEvent, ModuleConfigs, ModuleStatus, Value};
use tokio::sync::broadcast;

use crate::common;
use common::modules::{CONFIG_VALUE, CounterModule, FailStartModule};

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

/// Status transitions received so far for one module, in order.
fn status_events(
    rx: &mut broadcast::Receiver<ma::EventEnvelope>,
    module_id: &str,
) -> Vec<ModuleStatus> {
    let mut statuses = Vec::new();
    while let Ok(envelope) = rx.try_recv() {
        if let ModularAgentEvent::ModuleStatusChanged {
            module_id: id,
            status,
        } = envelope.event
            && id == module_id
        {
            statuses.push(status);
        }
    }
    statuses
}

/// A host must be able to tell a module whose start() failed from one that
/// is running, both live (status events) and after the fact (the mirror
/// behind get_module_statuses), while the patch itself reports running.
#[tokio::test]
async fn status_events_and_mirror_track_start_failure() {
    let ma = test_utils::setup_modular_agent().await;
    let patch_id = ma.new_patch().unwrap();
    let fail_spec = ma.new_module_spec(FailStartModule::DEF_NAME).unwrap();
    let fail_id = ma.add_module(patch_id.clone(), fail_spec).await.unwrap();
    let ok_spec = ma.new_module_spec(CounterModule::DEF_NAME).unwrap();
    let ok_id = ma.add_module(patch_id.clone(), ok_spec).await.unwrap();

    let statuses = ma.get_module_statuses(&patch_id).await.unwrap();
    assert_eq!(statuses[&fail_id], ModuleStatus::Init);
    assert_eq!(statuses[&ok_id], ModuleStatus::Init);

    let mut fail_rx = ma.subscribe();
    let mut ok_rx = ma.subscribe();
    ma.start_patch(&patch_id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(ma.get_patch_info(&patch_id).await.unwrap().running);
    let statuses = ma.get_module_statuses(&patch_id).await.unwrap();
    assert_eq!(statuses[&fail_id], ModuleStatus::Init);
    assert_eq!(statuses[&ok_id], ModuleStatus::Start);
    assert_eq!(
        status_events(&mut fail_rx, &fail_id),
        [ModuleStatus::Start, ModuleStatus::Init]
    );
    assert_eq!(status_events(&mut ok_rx, &ok_id), [ModuleStatus::Start]);

    ma.stop_patch(&patch_id).await.unwrap();

    let statuses = ma.get_module_statuses(&patch_id).await.unwrap();
    assert_eq!(statuses[&fail_id], ModuleStatus::Init);
    assert_eq!(statuses[&ok_id], ModuleStatus::Init);
    // stop() is skipped for the failed module, so no further transitions.
    assert_eq!(status_events(&mut fail_rx, &fail_id), []);
    assert_eq!(
        status_events(&mut ok_rx, &ok_id),
        [ModuleStatus::Stop, ModuleStatus::Init]
    );

    ma.quit();
}
