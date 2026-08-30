extern crate modular_agent_core as ma;

use std::time::Duration;

use ma::test_utils::{self, TestProbeAgent, probe_receiver};
use ma::{AgentSpec, AgentValue, ConnectionSpec, ModularAgent};

use crate::common;
use common::agents::{CounterAgent, StuckSleepAgent};

const EXT_IN_DEF: &str = "modular_agent_core::external_agent::ExternalInputAgent";

fn set_config(spec: &mut AgentSpec, key: &str, value: AgentValue) {
    let mut configs = spec.configs.take().unwrap_or_default();
    configs.set(key.into(), value);
    spec.configs = Some(configs);
}

/// Builds and starts a patch: ExtIn(channel) -> agent(def) -> probe.
/// Returns the probe id.
async fn start_chain(ma: &ModularAgent, channel: &str, agent_def: &str, out_port: &str) -> String {
    let patch_id = ma.new_patch().unwrap();

    let mut ext_spec = ma.new_agent_spec(EXT_IN_DEF).unwrap();
    set_config(&mut ext_spec, "name", AgentValue::string(channel));
    let ext_id = ma.add_agent(patch_id.clone(), ext_spec).await.unwrap();

    let agent_spec = ma.new_agent_spec(agent_def).unwrap();
    let agent_id = ma.add_agent(patch_id.clone(), agent_spec).await.unwrap();

    let probe_spec = ma.new_agent_spec(TestProbeAgent::DEF_NAME).unwrap();
    let probe_id = ma.add_agent(patch_id.clone(), probe_spec).await.unwrap();

    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: ext_id,
            source_handle: "value".into(),
            target: agent_id.clone(),
            target_handle: "in".into(),
        },
    )
    .await
    .unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: agent_id,
            source_handle: out_port.into(),
            target: probe_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();

    ma.start_patch(&patch_id).await.unwrap();
    // Agent start() runs inside the spawned agent loop; give the external
    // input agent a moment to register its channel.
    tokio::time::sleep(Duration::from_millis(100)).await;

    probe_id
}

/// A backlog far beyond the old inbox capacity on one agent must not stall
/// delivery for unrelated flows. Under the old bounded channels the router's
/// awaited send into the full inbox parked the single routing task, so the
/// unrelated flow below would never see its output.
#[tokio::test]
async fn flooded_inbox_does_not_stall_unrelated_flows() {
    let ma = test_utils::setup_modular_agent().await;

    let blocked_probe = start_chain(&ma, "uq_blocked", StuckSleepAgent::DEF_NAME, "out").await;
    let free_probe = start_chain(&ma, "uq_free", CounterAgent::DEF_NAME, "count").await;

    // Park the agent inside its 30s process().
    let blocked = probe_receiver(&ma, &blocked_probe).await.unwrap();
    ma.write_external_input("uq_blocked".into(), AgentValue::unit())
        .await
        .unwrap();
    let (_ctx, value) = blocked
        .recv_with_timeout(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(value, AgentValue::string("started"));

    // Pile up a backlog past the old 1024-message inbox capacity. The probe
    // is intentionally not asserted per message: event delivery may lag
    // under this burst, only end-to-end output matters here.
    for _ in 0..1500 {
        ma.write_external_input("uq_blocked".into(), AgentValue::unit())
            .await
            .unwrap();
    }

    let free = probe_receiver(&ma, &free_probe).await.unwrap();
    ma.write_external_input("uq_free".into(), AgentValue::unit())
        .await
        .unwrap();
    let (_ctx, value) = free
        .recv_with_timeout(Duration::from_secs(5))
        .await
        .expect("unrelated flow must keep delivering while another inbox is flooded");
    assert_eq!(value, AgentValue::integer(1));

    ma.quit();
}
