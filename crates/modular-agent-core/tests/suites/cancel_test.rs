extern crate modular_agent_core as ma;

use std::time::{Duration, Instant};

use ma::test_utils::{self, TestProbeModule, probe_receiver};
use ma::tool::get_tool;
use ma::{
    CancellationToken, ConnectionSpec, Error, ModularAgent, ModuleContext, ModuleSpec, Value,
};

use crate::common;
use common::modules::{CancelWaitModule, StuckSleepModule};

const EXT_IN_DEF: &str = "modular_agent_core::external_module::ExternalInputModule";
const CUSTOM_TOOL_DEF: &str = "modular_agent_core::tool::CustomToolModule";

fn set_config(spec: &mut ModuleSpec, key: &str, value: Value) {
    let mut configs = spec.configs.take().unwrap_or_default();
    configs.set(key.into(), value);
    spec.configs = Some(configs);
}

/// Builds and starts a patch: ExtIn(channel) -> module(def) -> probe.
/// Returns (module_id, probe_id).
async fn start_chain(ma: &ModularAgent, channel: &str, module_def: &str) -> (String, String) {
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
            source: module_id.clone(),
            source_handle: "out".into(),
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

    (module_id, probe_id)
}

#[tokio::test]
async fn stop_module_returns_promptly_during_long_process() {
    let ma = test_utils::setup_modular_agent().await;
    let (module_id, probe_id) =
        start_chain(&ma, "cancel_test_stop", StuckSleepModule::DEF_NAME).await;

    let probe = probe_receiver(&ma, &probe_id).await.unwrap();
    ma.write_external_input("cancel_test_stop".into(), Value::unit())
        .await
        .unwrap();

    // The module is now sleeping 30s inside process(), holding its lock.
    let (_ctx, value) = probe
        .recv_with_timeout(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(value, Value::string("started"));

    let stop_started = Instant::now();
    tokio::time::timeout(Duration::from_secs(5), ma.stop_module(&module_id))
        .await
        .expect("stop_module must not hang behind a long-running process()")
        .unwrap();
    assert!(
        stop_started.elapsed() < Duration::from_secs(3),
        "stop_module took {:?}",
        stop_started.elapsed()
    );

    ma.quit();
}

#[tokio::test]
async fn abort_context_cancels_running_flow() {
    let ma = test_utils::setup_modular_agent().await;
    let (module_id, probe_id) =
        start_chain(&ma, "cancel_test_abort", CancelWaitModule::DEF_NAME).await;

    let probe = probe_receiver(&ma, &probe_id).await.unwrap();
    ma.write_external_input("cancel_test_abort".into(), Value::unit())
        .await
        .unwrap();

    // The module emitted "started" and is now waiting on its cancel token.
    let (ctx, value) = probe
        .recv_with_timeout(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(value, Value::string("started"));

    assert!(ma.abort_context(ctx.id()));
    assert!(ctx.is_cancelled());

    // Cancellation must not block delivery: the module's "aborted" wind-down
    // emit and any later message carrying the fired token still reach
    // downstream modules — history repair depends on this. Suppressing
    // external work after abort is the responsibility of the modules that
    // initiate it (see the AsModule cancellation contract), not of routing.
    let (_ctx, value) = probe
        .recv_with_timeout(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(value, Value::string("aborted"));

    ma.send_module_out(
        module_id,
        ctx,
        "out".into(),
        Value::string("queued-after-abort"),
    )
    .await
    .unwrap();
    let (_ctx, value) = probe
        .recv_with_timeout(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(value, Value::string("queued-after-abort"));

    ma.quit();
}

#[tokio::test]
async fn start_module_after_stop_patch_processes_inputs() {
    let ma = test_utils::setup_modular_agent().await;
    let patch_id = ma.new_patch().unwrap();

    let mut ext_spec = ma.new_module_spec(EXT_IN_DEF).unwrap();
    set_config(&mut ext_spec, "name", Value::string("cancel_test_restart"));
    let ext_id = ma.add_module(patch_id.clone(), ext_spec).await.unwrap();

    let probe_spec = ma.new_module_spec(TestProbeModule::DEF_NAME).unwrap();
    let probe_id = ma.add_module(patch_id.clone(), probe_spec).await.unwrap();

    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: ext_id.clone(),
            source_handle: "value".into(),
            target: probe_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();

    ma.start_patch(&patch_id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    ma.stop_patch(&patch_id).await.unwrap();

    // Individually restarted modules must get live cancellation tokens, not
    // children of the parent token fired by stop_patch — a born-cancelled
    // token would make them silently skip every input.
    ma.start_module(&ext_id).await.unwrap();
    ma.start_module(&probe_id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let probe = probe_receiver(&ma, &probe_id).await.unwrap();
    ma.write_external_input("cancel_test_restart".into(), Value::integer(7))
        .await
        .unwrap();
    let (_ctx, value) = probe
        .recv_with_timeout(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(value, Value::integer(7));

    ma.quit();
}

#[tokio::test]
async fn already_cancelled_custom_tool_does_not_emit_tool_in() {
    let tool_name = "cancel_test_custom_tool";

    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let mut spec = ma.new_module_spec(CUSTOM_TOOL_DEF).unwrap();
    set_config(&mut spec, "name", Value::string(tool_name));
    let module_id = ma.add_module(patch_id.clone(), spec).await.unwrap();
    let probe_spec = ma.new_module_spec(TestProbeModule::DEF_NAME).unwrap();
    let probe_id = ma.add_module(patch_id.clone(), probe_spec).await.unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: module_id,
            source_handle: "tool_in".into(),
            target: probe_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();

    ma.start_patch(&patch_id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let tool = get_tool(tool_name).unwrap();
    let probe = probe_receiver(&ma, &probe_id).await.unwrap();
    let token = CancellationToken::new();
    token.cancel();
    let ctx = ModuleContext::new().with_cancel_token(token.clone());

    let wait_started = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(5), tool.call(ctx, Value::unit()))
        .await
        .expect("cancelled tool call must not wait for the timeout");
    assert!(matches!(result, Err(Error::Cancelled)));
    assert!(wait_started.elapsed() < Duration::from_secs(3));
    assert!(
        probe
            .recv_with_timeout(Duration::from_millis(500))
            .await
            .is_err(),
        "an already-cancelled tool call must not emit tool_in"
    );

    ma.stop_patch(&patch_id).await.unwrap();
    ma.quit();
}
