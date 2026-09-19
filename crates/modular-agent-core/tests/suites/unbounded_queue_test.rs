extern crate modular_agent_core as ma;

use std::time::Duration;

use ma::test_utils::{self, TestProbeModule, probe_receiver};
use ma::{ConnectionSpec, ModularAgent, ModuleSpec, Value};

use crate::common;
use common::modules::{CounterModule, StuckSleepModule};

const EXT_IN_DEF: &str = "modular_agent_core::external_module::ExternalInputModule";

fn set_config(spec: &mut ModuleSpec, key: &str, value: Value) {
    let mut configs = spec.configs.take().unwrap_or_default();
    configs.set(key.into(), value);
    spec.configs = Some(configs);
}

/// Builds and starts a patch: ExtIn(channel) -> module(def) -> probe.
/// Returns the probe id.
async fn start_chain(ma: &ModularAgent, channel: &str, module_def: &str, out_port: &str) -> String {
    let patch_id = ma.new_patch().unwrap();

    let mut ext_spec = ma.new_module_spec(EXT_IN_DEF).unwrap();
    set_config(&mut ext_spec, "name", Value::string(channel));
    let ext_id = ma.add_module(patch_id.clone(), ext_spec).await.unwrap();

    let module_spec = ma.new_module_spec(module_def).unwrap();
    let module_id = ma.add_module(patch_id.clone(), module_spec).await.unwrap();

    let probe_spec = ma.new_module_spec(TestProbeModule::DEF_NAME).unwrap();
    let probe_id = ma.add_module(patch_id.clone(), probe_spec).await.unwrap();

    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: ext_id,
            source_handle: "value".into(),
            target: module_id.clone(),
            target_handle: "in".into(),
        },
    )
    .await
    .unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: module_id,
            source_handle: out_port.into(),
            target: probe_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();

    ma.start_patch(&patch_id).await.unwrap();
    // Module start() runs inside the spawned module loop; give the external
    // input module a moment to register its channel.
    tokio::time::sleep(Duration::from_millis(100)).await;

    probe_id
}

/// A backlog far beyond the old inbox capacity on one module must not stall
/// delivery for unrelated flows. Under the old bounded channels the router's
/// awaited send into the full inbox parked the single routing task, so the
/// unrelated flow below would never see its output.
#[tokio::test]
async fn flooded_inbox_does_not_stall_unrelated_flows() {
    let ma = test_utils::setup_modular_agent().await;

    let blocked_probe = start_chain(&ma, "uq_blocked", StuckSleepModule::DEF_NAME, "out").await;
    let free_probe = start_chain(&ma, "uq_free", CounterModule::DEF_NAME, "count").await;

    // Park the module inside its 30s process().
    let blocked = probe_receiver(&ma, &blocked_probe).await.unwrap();
    ma.write_external_input("uq_blocked".into(), Value::unit())
        .await
        .unwrap();
    let (_ctx, value) = blocked
        .recv_with_timeout(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(value, Value::string("started"));

    // Pile up a backlog past the old 1024-message inbox capacity. The probe
    // is intentionally not asserted per message: event delivery may lag
    // under this burst, only end-to-end output matters here.
    for _ in 0..1500 {
        ma.write_external_input("uq_blocked".into(), Value::unit())
            .await
            .unwrap();
    }

    let free = probe_receiver(&ma, &free_probe).await.unwrap();
    ma.write_external_input("uq_free".into(), Value::unit())
        .await
        .unwrap();
    let (_ctx, value) = free
        .recv_with_timeout(Duration::from_secs(5))
        .await
        .expect("unrelated flow must keep delivering while another inbox is flooded");
    assert_eq!(value, Value::integer(1));

    ma.quit();
}
