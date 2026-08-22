extern crate modular_agent_core as ma;

use std::time::Duration;

use ma::{AgentError, AgentValue, ConnectionSpec, ModularAgent, ModularAgentEvent};
use tokio::time::timeout;

use crate::common;
use common::agents::PendingStopAgent;

const EXT_IN_DEF: &str = "modular_agent_core::external_agent::ExternalInputAgent";
const EXT_OUT_DEF: &str = "modular_agent_core::external_agent::ExternalOutputAgent";

fn ext_agent_spec(ma: &ModularAgent, def_name: &str, channel: &str) -> ma::AgentSpec {
    let mut spec = ma.new_agent_spec(def_name).unwrap();
    spec.configs
        .as_mut()
        .unwrap()
        .set("name".to_string(), AgentValue::string(channel));
    spec
}

/// Builds and starts a patch: ExtIn(channel_in) -> ExtOut(channel_out).
async fn start_ext_patch(ma: &ModularAgent, channel_in: &str, channel_out: &str) -> String {
    let patch_id = ma.new_patch().unwrap();
    let in_id = ma
        .add_agent(patch_id.clone(), ext_agent_spec(ma, EXT_IN_DEF, channel_in))
        .await
        .unwrap();
    let out_id = ma
        .add_agent(
            patch_id.clone(),
            ext_agent_spec(ma, EXT_OUT_DEF, channel_out),
        )
        .await
        .unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: in_id,
            source_handle: "value".into(),
            target: out_id,
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();
    ma.start_patch(&patch_id).await.unwrap();
    patch_id
}

#[tokio::test]
async fn shutdown_stops_running_patches_and_ends_subscribers() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = start_ext_patch(&ma, "shutdown_ok_in", "shutdown_ok_out").await;

    let mut events = ma.subscribe();
    let mut ext_out_rx = ma.subscribe_to_event(|envelope| match envelope.event {
        ModularAgentEvent::ExternalOutput(name, value) => Some((name, value)),
        _ => None,
    });

    ma.shutdown(Duration::from_secs(1)).await.unwrap();

    // The forwarder behind subscribe_to_event is part of shutdown: once it
    // has exited, the channel reports closed instead of blocking forever.
    let remaining = timeout(Duration::from_secs(1), ext_out_rx.recv())
        .await
        .expect("subscriber channel did not close after shutdown");
    assert!(remaining.is_none());

    let mut stopped = false;
    while let Ok(envelope) = events.try_recv() {
        if matches!(
            &envelope.event,
            ModularAgentEvent::PatchStopped { patch_id: id } if *id == patch_id
        ) {
            stopped = true;
        }
    }
    assert!(
        stopped,
        "PatchStopped was not emitted for the running patch"
    );

    let patch = ma.get_patch(&patch_id).unwrap();
    assert!(!patch.lock().await.running());
}

#[tokio::test]
async fn shutdown_times_out_on_agent_whose_stop_never_returns() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let spec = ma.new_agent_spec(PendingStopAgent::DEF_NAME).unwrap();
    ma.add_agent(patch_id.clone(), spec).await.unwrap();
    ma.start_patch(&patch_id).await.unwrap();
    // Agent start() runs inside the spawned agent loop; stop() is only
    // invoked on an agent that has finished starting.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let timeout = Duration::from_millis(100);
    let result = ma.shutdown(timeout).await;
    assert!(
        matches!(result, Err(AgentError::ShutdownTimeout(d)) if d == timeout),
        "expected ShutdownTimeout, got {:?}",
        result
    );
}
