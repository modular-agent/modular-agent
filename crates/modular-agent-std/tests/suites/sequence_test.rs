extern crate modular_agent_core as ma;

use ma::test_utils::{self, ProbeReceiver, probe_receiver, recv_probe};
use ma::{AgentContext, AgentValue, ConnectionSpec, ModularAgent};

const PATCH: &str = "tests/patches/Std_Sequence_test.json";

const SYNC_DEF: &str = "modular_agent_std::sequence::SyncAgent";
const PROBE_DEF: &str = "modular_agent_core::test_utils::TestProbeAgent";

#[tokio::test]
async fn test_sequence() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // One input fans out to every output port in order
    test_utils::write_and_expect_local_value(&ma, &patch_id, "seq_in", AgentValue::integer(42))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "seq_0", &AgentValue::integer(42))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "seq_1", &AgentValue::integer(42))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_sync_fifo() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // Nothing is emitted until every port has a value
    test_utils::write_and_expect_local_value(&ma, &patch_id, "sync_in0", AgentValue::integer(1))
        .await
        .unwrap();
    test_utils::write_and_expect_local_value(&ma, &patch_id, "sync_in1", AgentValue::integer(2))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "sync_0", &AgentValue::integer(1))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "sync_1", &AgentValue::integer(2))
        .await
        .unwrap();

    // Values queue per port and pair up first-in-first-out
    test_utils::write_and_expect_local_value(&ma, &patch_id, "sync_in0", AgentValue::integer(3))
        .await
        .unwrap();
    test_utils::write_and_expect_local_value(&ma, &patch_id, "sync_in0", AgentValue::integer(4))
        .await
        .unwrap();
    test_utils::write_and_expect_local_value(&ma, &patch_id, "sync_in1", AgentValue::integer(5))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "sync_0", &AgentValue::integer(3))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "sync_1", &AgentValue::integer(5))
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "sync_in1", AgentValue::integer(6))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "sync_0", &AgentValue::integer(4))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "sync_1", &AgentValue::integer(6))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_sync_context_mode() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // A single trigger fans out through Sequence, so both Sync ports see the
    // same ctx and the pair completes
    test_utils::write_and_expect_local_value(&ma, &patch_id, "syncctx_in", AgentValue::integer(7))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "syncctx_0", &AgentValue::integer(7))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "syncctx_1", &AgentValue::integer(7))
        .await
        .unwrap();

    // The buffer is released on completion, so the next trigger works the same
    test_utils::write_and_expect_local_value(&ma, &patch_id, "syncctx_in", AgentValue::integer(8))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "syncctx_0", &AgentValue::integer(8))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "syncctx_1", &AgentValue::integer(8))
        .await
        .unwrap();

    ma.quit();
}

/// Wire a context-mode Sync to one probe per output port so emitted ctxs can
/// be inspected.
async fn setup_ctx_sync_with_probes(ma: &ModularAgent) -> (String, ProbeReceiver, ProbeReceiver) {
    let patch_id = ma.new_patch().unwrap();

    let sync_def = ma.get_agent_definition(SYNC_DEF).unwrap();
    let mut sync_spec = sync_def.to_spec();
    if let Some(configs) = sync_spec.configs.as_mut() {
        configs.set("use_ctx".into(), AgentValue::boolean(true));
    }
    let sync_id = ma.add_agent(patch_id.clone(), sync_spec).await.unwrap();

    let probe_def = ma.get_agent_definition(PROBE_DEF).unwrap();
    let mut probe_ids = Vec::new();
    for port in ["0", "1"] {
        let probe_id = ma
            .add_agent(patch_id.clone(), probe_def.to_spec())
            .await
            .unwrap();
        ma.add_connection(
            &patch_id,
            ConnectionSpec {
                source: sync_id.clone(),
                source_handle: port.into(),
                target: probe_id.clone(),
                target_handle: "value".into(),
            },
        )
        .await
        .unwrap();
        probe_ids.push(probe_id);
    }
    ma.start_patch(&patch_id).await.unwrap();

    let probe0 = probe_receiver(ma, &probe_ids[0]).await.unwrap();
    let probe1 = probe_receiver(ma, &probe_ids[1]).await.unwrap();
    (sync_id, probe0, probe1)
}

#[tokio::test]
async fn test_sync_context_mode_keeps_slot_ctx() {
    let ma = test_utils::setup_modular_agent().await;
    let (sync_id, probe0, probe1) = setup_ctx_sync_with_probes(&ma).await;

    // Same ctx_key (same id, no map frames) but per-branch frames differ; each
    // slot must be emitted with the ctx its value arrived with
    let base = AgentContext::new();
    let ctx0 = base.push_frame("branch".into(), AgentValue::string("left"));
    let ctx1 = base.push_frame("branch".into(), AgentValue::string("right"));

    let agent = ma.get_agent(&sync_id).unwrap();
    {
        let mut guard = agent.lock().await;
        guard
            .process(ctx0, "0".into(), AgentValue::integer(10))
            .await
            .unwrap();
        guard
            .process(ctx1, "1".into(), AgentValue::integer(20))
            .await
            .unwrap();
    }

    let (out_ctx0, value0) = recv_probe(&probe0).await.unwrap();
    assert_eq!(value0, AgentValue::integer(10));
    assert_eq!(out_ctx0.id(), base.id());
    assert_eq!(
        out_ctx0.frames().unwrap()[0].data,
        AgentValue::string("left")
    );

    let (out_ctx1, value1) = recv_probe(&probe1).await.unwrap();
    assert_eq!(value1, AgentValue::integer(20));
    assert_eq!(out_ctx1.id(), base.id());
    assert_eq!(
        out_ctx1.frames().unwrap()[0].data,
        AgentValue::string("right")
    );

    ma.quit();
}

#[tokio::test]
async fn test_sync_context_mode_interleaved() {
    let ma = test_utils::setup_modular_agent().await;
    let (sync_id, probe0, probe1) = setup_ctx_sync_with_probes(&ma).await;

    // Two flows interleave: flow B completes first even though flow A started
    // first, so values pair by ctx, not by arrival order (FIFO would pair 1
    // with 4 here)
    let ctx_a = AgentContext::new();
    let ctx_b = AgentContext::new();

    let agent = ma.get_agent(&sync_id).unwrap();
    {
        let mut guard = agent.lock().await;
        guard
            .process(ctx_a.clone(), "0".into(), AgentValue::integer(1))
            .await
            .unwrap();
        guard
            .process(ctx_b.clone(), "0".into(), AgentValue::integer(3))
            .await
            .unwrap();
        guard
            .process(ctx_b.clone(), "1".into(), AgentValue::integer(4))
            .await
            .unwrap();
    }

    let (out_ctx, value) = recv_probe(&probe0).await.unwrap();
    assert_eq!(value, AgentValue::integer(3));
    assert_eq!(out_ctx.id(), ctx_b.id());
    let (out_ctx, value) = recv_probe(&probe1).await.unwrap();
    assert_eq!(value, AgentValue::integer(4));
    assert_eq!(out_ctx.id(), ctx_b.id());

    {
        let mut guard = agent.lock().await;
        guard
            .process(ctx_a.clone(), "1".into(), AgentValue::integer(2))
            .await
            .unwrap();
    }

    let (out_ctx, value) = recv_probe(&probe0).await.unwrap();
    assert_eq!(value, AgentValue::integer(1));
    assert_eq!(out_ctx.id(), ctx_a.id());
    let (out_ctx, value) = recv_probe(&probe1).await.unwrap();
    assert_eq!(value, AgentValue::integer(2));
    assert_eq!(out_ctx.id(), ctx_a.id());

    ma.quit();
}
