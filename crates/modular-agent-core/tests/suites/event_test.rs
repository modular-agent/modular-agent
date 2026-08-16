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
    let patch_id = mcp.new_patch().unwrap();
    let envelope = expect_event(&mut rx, |e| {
        matches!(e, ModularAgentEvent::PatchAdded { .. })
    })
    .await;
    assert_eq!(envelope.origin.as_deref(), Some("mcp"));

    let in_id = mcp
        .add_agent(
            patch_id.clone(),
            ext_agent_spec(&mcp, EXT_IN_DEF, "origin_in"),
        )
        .await
        .unwrap();
    let envelope = expect_event(&mut rx, |e| {
        matches!(e, ModularAgentEvent::PatchStructureChanged { .. })
    })
    .await;
    assert_eq!(envelope.origin.as_deref(), Some("mcp"));

    let out_id = mcp
        .add_agent(
            patch_id.clone(),
            ext_agent_spec(&mcp, EXT_OUT_DEF, "origin_out"),
        )
        .await
        .unwrap();
    mcp.add_connection(
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

    // Run the flow: the resulting ExternalOutput is emitted by the agent
    // runtime through the handle stored at agent creation, which must have
    // been stripped of the creator's origin.
    mcp.start_patch(&patch_id).await.unwrap();
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

    mcp.stop_patch(&patch_id).await.unwrap();
    base.quit();
}

#[tokio::test]
async fn test_update_agent_spec_emit_rules() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let agent_id = ma
        .add_agent(patch_id.clone(), ma.new_agent_spec(EXT_OUT_DEF).unwrap())
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
    // configs-only patch produced no PatchStructureChanged.
    let e1 = next_event(&mut rx).await;
    assert!(matches!(e1.event, ModularAgentEvent::AgentSpecUpdated(ref id) if id == &agent_id));

    let e2 = next_event(&mut rx).await;
    assert!(matches!(e2.event, ModularAgentEvent::AgentSpecUpdated(ref id) if id == &agent_id));

    let e3 = next_event(&mut rx).await;
    assert!(
        matches!(e3.event, ModularAgentEvent::PatchStructureChanged { patch_id: ref p } if p == &patch_id)
    );

    // Nothing else is running, so no further events may be pending.
    assert!(matches!(
        rx.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));

    ma.quit();
}

#[tokio::test]
async fn test_update_agent_spec_spec_only_emit_rules() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    // An unknown definition leaves the agent spec-only: no live instance,
    // so update_agent_spec patches the stored patch spec entry instead.
    let spec = ma::PatchSpec {
        agents: vec![ma::AgentSpec {
            id: "orphan".into(),
            def_name: "no_such::Definition".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let patch_id = ma.add_patch(spec).unwrap();
    let orphan_id = ma.get_patch_spec(&patch_id).await.unwrap().agents[0]
        .id
        .clone();

    // Subscribe after setup so only the two patches below produce events.
    let mut rx = ma.subscribe();

    let configs_only = serde_json::json!({ "configs": { "channel": "ch" } });
    ma.update_agent_spec(&orphan_id, &configs_only)
        .await
        .unwrap();

    let structural = serde_json::json!({ "x": 480.0 });
    ma.update_agent_spec(&orphan_id, &structural).await.unwrap();

    // Same contract as the live path (test_update_agent_spec_emit_rules):
    // the configs-only patch produces no PatchStructureChanged, so hosts
    // cannot tell a spec-only agent from a live one.
    let e1 = next_event(&mut rx).await;
    assert!(matches!(e1.event, ModularAgentEvent::AgentSpecUpdated(ref id) if id == &orphan_id));

    let e2 = next_event(&mut rx).await;
    assert!(matches!(e2.event, ModularAgentEvent::AgentSpecUpdated(ref id) if id == &orphan_id));

    let e3 = next_event(&mut rx).await;
    assert!(
        matches!(e3.event, ModularAgentEvent::PatchStructureChanged { patch_id: ref p } if p == &patch_id)
    );

    assert!(matches!(
        rx.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));

    ma.quit();
}

#[tokio::test]
async fn test_set_agent_configs_spec_only_emit_rules() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let spec = ma::PatchSpec {
        agents: vec![ma::AgentSpec {
            id: "orphan".into(),
            def_name: "no_such::Definition".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let patch_id = ma.add_patch(spec).unwrap();
    let orphan_id = ma.get_patch_spec(&patch_id).await.unwrap().agents[0]
        .id
        .clone();

    let mut rx = ma.subscribe();

    let mut configs = ma::AgentConfigs::default();
    configs.set("channel".into(), AgentValue::string("random"));
    ma.set_agent_configs(orphan_id.clone(), configs)
        .await
        .unwrap();

    // Same contract as the live non-running branch: one AgentConfigUpdated
    // per key and nothing else - no AgentSpecUpdated, no structure event.
    let e = next_event(&mut rx).await;
    assert!(matches!(
        e.event,
        ModularAgentEvent::AgentConfigUpdated(ref id, ref key, ref value)
            if id == &orphan_id && key == "channel" && value == &AgentValue::string("random")
    ));

    assert!(matches!(
        rx.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));

    ma.quit();
}

#[tokio::test]
async fn test_update_agent_spec_announces_dynamic_config_changes() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma
        .get_agent_definition(crate::common::agents::NumberedConfigAgent::DEF_NAME)
        .unwrap();
    let agent_id = ma.add_agent(patch_id.clone(), def.to_spec()).await.unwrap();

    let mut rx = ma.subscribe();

    // A successful configs patch produces exactly two AgentSpecUpdated: the
    // agent's own emit from configs_changed plus the orchestrator's, and no
    // PatchStructureChanged for a configs-only patch.
    ma.update_agent_spec(&agent_id, &serde_json::json!({ "configs": { "n": 3 } }))
        .await
        .unwrap();

    for _ in 0..2 {
        let e = next_event(&mut rx).await;
        assert!(matches!(e.event, ModularAgentEvent::AgentSpecUpdated(ref id) if id == &agent_id));
    }
    assert!(matches!(
        rx.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));

    // A patch the agent rejects after committing it (Switch stores an
    // unparsable condition as never-matching) must return the error AND
    // still announce the spec change - the value is in the live spec.
    let bad = serde_json::json!({ "configs": { "c2": crate::common::agents::INVALID_CONDITION } });
    ma.update_agent_spec(&agent_id, &bad)
        .await
        .expect_err("the committed config error must propagate");

    let e = next_event(&mut rx).await;
    assert!(matches!(e.event, ModularAgentEvent::AgentSpecUpdated(ref id) if id == &agent_id));

    let spec = ma.get_agent_spec(&agent_id).await.unwrap();
    assert_eq!(
        spec.configs.unwrap().get_string("c2").unwrap(),
        crate::common::agents::INVALID_CONDITION,
        "the rejected value stays committed, so it must have been announced"
    );

    ma.quit();
}

#[tokio::test]
async fn test_ext_in_renamed_while_stopped_delivers_once() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let in_id = ma
        .add_agent(patch_id.clone(), ext_agent_spec(&ma, EXT_IN_DEF, "before"))
        .await
        .unwrap();
    let out_id = ma
        .add_agent(
            patch_id.clone(),
            ext_agent_spec(&ma, EXT_OUT_DEF, "renamed_out"),
        )
        .await
        .unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: in_id.clone(),
            source_handle: "value".into(),
            target: out_id,
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();

    // Renaming the channel while the patch is stopped must only re-point
    // the agent's state: registration happens in start(), and a second
    // entry would deliver every input twice.
    ma.update_agent_spec(
        &in_id,
        &serde_json::json!({ "configs": { "name": "renamed_in" } }),
    )
    .await
    .unwrap();
    ma.start_patch(&patch_id).await.unwrap();

    let mut rx = ma.subscribe();
    ma.write_external_input("renamed_in".into(), AgentValue::string("a"))
        .await
        .unwrap();
    let e = expect_event(
        &mut rx,
        |e| matches!(e, ModularAgentEvent::ExternalOutput(name, _) if name == "renamed_out"),
    )
    .await;
    assert!(
        matches!(e.event, ModularAgentEvent::ExternalOutput(_, ref v) if v == &AgentValue::string("a"))
    );

    // A duplicate registration would deliver "a" a second time here.
    ma.write_external_input("renamed_in".into(), AgentValue::string("b"))
        .await
        .unwrap();
    let e = expect_event(
        &mut rx,
        |e| matches!(e, ModularAgentEvent::ExternalOutput(name, _) if name == "renamed_out"),
    )
    .await;
    assert!(
        matches!(e.event, ModularAgentEvent::ExternalOutput(_, ref v) if v == &AgentValue::string("b")),
        "each input must be delivered exactly once"
    );

    ma.stop_patch(&patch_id).await.unwrap();
    ma.quit();
}
