extern crate modular_agent_core as ma;

use std::time::Duration;

use ma::{AgentValue, ConnectionSpec, EventEnvelope, ModularAgent, ModularAgentEvent};
use tokio::sync::broadcast;
use tokio::time::timeout;

const EXT_IN_DEF: &str = "modular_agent_core::external_agent::ExternalInputAgent";
const EXT_OUT_DEF: &str = "modular_agent_core::external_agent::ExternalOutputAgent";

async fn next_event(rx: &mut broadcast::Receiver<EventEnvelope>) -> EventEnvelope {
    timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for event")
        .expect("event channel closed")
}

/// Receives events until one matches, returning its envelope. Unrelated
/// events (e.g. AgentIn emitted while a flow runs) are skipped.
async fn expect_event(
    rx: &mut broadcast::Receiver<EventEnvelope>,
    mut matches: impl FnMut(&ModularAgentEvent) -> bool,
) -> EventEnvelope {
    loop {
        let envelope = next_event(rx).await;
        if matches(&envelope.event) {
            return envelope;
        }
    }
}

fn ext_agent_spec(ma: &ModularAgent, def_name: &str, channel: &str) -> ma::AgentSpec {
    let mut spec = ma.new_agent_spec(def_name).unwrap();
    spec.configs
        .as_mut()
        .unwrap()
        .set("name".to_string(), AgentValue::string(channel));
    spec
}

#[tokio::test]
async fn test_origin_stamped_on_tagged_handle_and_stripped_at_runtime() {
    let base = ModularAgent::init().unwrap();
    base.ready().await.unwrap();
    let mcp = base.with_origin("mcp");

    let mut rx = base.subscribe();

    // Structural changes made through the tagged handle carry its origin.
    let preset_id = mcp.new_preset().unwrap();
    let envelope = expect_event(&mut rx, |e| {
        matches!(e, ModularAgentEvent::PresetAdded { .. })
    })
    .await;
    assert_eq!(envelope.origin.as_deref(), Some("mcp"));

    let in_id = mcp
        .add_agent(
            preset_id.clone(),
            ext_agent_spec(&mcp, EXT_IN_DEF, "origin_in"),
        )
        .await
        .unwrap();
    let envelope = expect_event(&mut rx, |e| {
        matches!(e, ModularAgentEvent::PresetStructureChanged { .. })
    })
    .await;
    assert_eq!(envelope.origin.as_deref(), Some("mcp"));

    let out_id = mcp
        .add_agent(
            preset_id.clone(),
            ext_agent_spec(&mcp, EXT_OUT_DEF, "origin_out"),
        )
        .await
        .unwrap();
    mcp.add_connection(
        &preset_id,
        ConnectionSpec {
            source: in_id,
            source_handle: "value".into(),
            target: out_id,
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();

    // Run the flow: the resulting ExternalOutput is emitted by the agent
    // runtime through the handle stored at agent creation, which must have
    // been stripped of the creator's origin.
    mcp.start_preset(&preset_id).await.unwrap();
    mcp.write_external_input("origin_in".into(), AgentValue::string("hello"))
        .await
        .unwrap();

    let envelope = expect_event(
        &mut rx,
        |e| matches!(e, ModularAgentEvent::ExternalOutput(name, _) if name == "origin_out"),
    )
    .await;
    assert_eq!(
        envelope.origin, None,
        "runtime events must not inherit the origin of the handle that created the agent"
    );

    mcp.stop_preset(&preset_id).await.unwrap();
    base.quit();
}

#[tokio::test]
async fn test_update_agent_spec_emit_rules() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let agent_id = ma
        .add_agent(preset_id.clone(), ma.new_agent_spec(EXT_OUT_DEF).unwrap())
        .await
        .unwrap();

    // Subscribe after setup so only the two patches below produce events.
    let mut rx = ma.subscribe();

    let configs_only = serde_json::json!({ "configs": { "name": "ch" } });
    ma.update_agent_spec(&agent_id, &configs_only)
        .await
        .unwrap();

    let structural = serde_json::json!({ "x": 480.0 });
    ma.update_agent_spec(&agent_id, &structural).await.unwrap();

    // Events are emitted synchronously, so the exact sequence proves the
    // configs-only patch produced no PresetStructureChanged.
    let e1 = next_event(&mut rx).await;
    assert!(matches!(e1.event, ModularAgentEvent::AgentSpecUpdated(ref id) if id == &agent_id));

    let e2 = next_event(&mut rx).await;
    assert!(matches!(e2.event, ModularAgentEvent::AgentSpecUpdated(ref id) if id == &agent_id));

    let e3 = next_event(&mut rx).await;
    assert!(
        matches!(e3.event, ModularAgentEvent::PresetStructureChanged { preset_id: ref p } if p == &preset_id)
    );

    // Nothing else is running, so no further events may be pending.
    assert!(matches!(
        rx.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));

    ma.quit();
}
