extern crate modular_agent_core as ma;

use im::vector;
use ma::test_utils::{self, ProbeReceiver, probe_receiver, recv_probe};
use ma::{AgentContext, AgentValue, ConnectionSpec, ModularAgent};

const ZIP_TO_ARRAY_DEF: &str = "modular_agent_std::array::ZipToArrayAgent";
const MAP_DEF: &str = "modular_agent_std::array::MapAgent";
const COLLECT_DEF: &str = "modular_agent_std::array::CollectAgent";
const PROBE_DEF: &str = "modular_agent_core::test_utils::TestProbeAgent";

/// Add a probe agent wired to `source`'s `out_port` and return its receiver
/// factory id (call `probe_receiver` after the patch is started).
async fn add_probe(ma: &ModularAgent, patch_id: &str, source: &str, out_port: &str) -> String {
    let probe_def = ma.get_agent_definition(PROBE_DEF).unwrap();
    let probe_id = ma
        .add_agent(patch_id.to_string(), probe_def.to_spec())
        .await
        .unwrap();
    ma.add_connection(
        patch_id,
        ConnectionSpec {
            source: source.to_string(),
            source_handle: out_port.into(),
            target: probe_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();
    probe_id
}

#[tokio::test]
async fn test_map_emits_items_in_order_with_frames() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = ma.new_patch().unwrap();
    let map_id = ma
        .add_agent(
            patch_id.clone(),
            ma.get_agent_definition(MAP_DEF).unwrap().to_spec(),
        )
        .await
        .unwrap();
    let probe_id = add_probe(&ma, &patch_id, &map_id, "value").await;
    ma.start_patch(&patch_id).await.unwrap();
    let probe = probe_receiver(&ma, &probe_id).await.unwrap();

    let ctx = AgentContext::new();
    let agent = ma.get_agent(&map_id).unwrap();
    agent
        .lock()
        .await
        .process(
            ctx.clone(),
            "array".into(),
            AgentValue::array(vector![
                AgentValue::integer(10),
                AgentValue::integer(20),
                AgentValue::integer(30),
            ]),
        )
        .await
        .unwrap();

    // One emission per item, in order, each carrying a map frame (index, length)
    for (i, expected) in [10, 20, 30].iter().enumerate() {
        let (out_ctx, value) = recv_probe(&probe).await.unwrap();
        assert_eq!(value, AgentValue::integer(*expected));
        assert_eq!(out_ctx.current_map_frame().unwrap(), Some((i, 3)));
        assert_eq!(out_ctx.id(), ctx.id());
    }

    ma.quit();
}

#[tokio::test]
async fn test_map_collect_roundtrip() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = ma.new_patch().unwrap();
    let map_id = ma
        .add_agent(
            patch_id.clone(),
            ma.get_agent_definition(MAP_DEF).unwrap().to_spec(),
        )
        .await
        .unwrap();
    let collect_id = ma
        .add_agent(
            patch_id.clone(),
            ma.get_agent_definition(COLLECT_DEF).unwrap().to_spec(),
        )
        .await
        .unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: map_id.clone(),
            source_handle: "value".into(),
            target: collect_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();
    let probe_id = add_probe(&ma, &patch_id, &collect_id, "array").await;
    ma.start_patch(&patch_id).await.unwrap();
    let probe = probe_receiver(&ma, &probe_id).await.unwrap();

    let input = AgentValue::array(vector![
        AgentValue::string("a"),
        AgentValue::string("b"),
        AgentValue::string("c"),
    ]);
    let ctx = AgentContext::new();
    let agent = ma.get_agent(&map_id).unwrap();
    agent
        .lock()
        .await
        .process(ctx.clone(), "array".into(), input.clone())
        .await
        .unwrap();

    // Collect reassembles the mapped items and pops the map frame
    let (out_ctx, value) = recv_probe(&probe).await.unwrap();
    assert_eq!(value, input);
    assert_eq!(out_ctx.current_map_frame().unwrap(), None);
    assert_eq!(out_ctx.id(), ctx.id());

    ma.quit();
}

#[tokio::test]
async fn test_zip_to_array_fifo_pairs_in_arrival_order() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = ma.new_patch().unwrap();
    let zip_id = ma
        .add_agent(
            patch_id.clone(),
            ma.get_agent_definition(ZIP_TO_ARRAY_DEF).unwrap().to_spec(),
        )
        .await
        .unwrap();
    let probe_id = add_probe(&ma, &patch_id, &zip_id, "array").await;
    ma.start_patch(&patch_id).await.unwrap();
    let probe = probe_receiver(&ma, &probe_id).await.unwrap();

    let ctx = AgentContext::new();
    let agent = ma.get_agent(&zip_id).unwrap();
    {
        let mut guard = agent.lock().await;
        // Port 1 values queue up until port 0 arrives, then pair head-first
        for (port, v) in [("1", 10), ("1", 20), ("0", 1), ("0", 2)] {
            guard
                .process(ctx.clone(), port.into(), AgentValue::integer(v))
                .await
                .unwrap();
        }
    }

    let (_c, value) = recv_probe(&probe).await.unwrap();
    assert_eq!(
        value,
        AgentValue::array(vector![AgentValue::integer(1), AgentValue::integer(10)])
    );
    let (_c, value) = recv_probe(&probe).await.unwrap();
    assert_eq!(
        value,
        AgentValue::array(vector![AgentValue::integer(2), AgentValue::integer(20)])
    );

    ma.quit();
}

#[tokio::test]
async fn test_zip_to_array_n3() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = ma.new_patch().unwrap();
    let mut zip_spec = ma.get_agent_definition(ZIP_TO_ARRAY_DEF).unwrap().to_spec();
    if let Some(configs) = zip_spec.configs.as_mut() {
        configs.set("n".into(), AgentValue::integer(3));
    }
    let zip_id = ma.add_agent(patch_id.clone(), zip_spec).await.unwrap();
    let probe_id = add_probe(&ma, &patch_id, &zip_id, "array").await;
    ma.start_patch(&patch_id).await.unwrap();
    let probe = probe_receiver(&ma, &probe_id).await.unwrap();

    let ctx = AgentContext::new();
    let agent = ma.get_agent(&zip_id).unwrap();
    {
        let mut guard = agent.lock().await;
        for (port, v) in [("0", 1), ("1", 2), ("2", 3)] {
            guard
                .process(ctx.clone(), port.into(), AgentValue::integer(v))
                .await
                .unwrap();
        }
    }

    let (_c, value) = recv_probe(&probe).await.unwrap();
    assert_eq!(
        value,
        AgentValue::array(vector![
            AgentValue::integer(1),
            AgentValue::integer(2),
            AgentValue::integer(3),
        ])
    );

    ma.quit();
}

/// Wire a context-mode ZipToArray to a probe on its `array` output.
async fn setup_ctx_zip_with_probe(ma: &ModularAgent) -> (String, ProbeReceiver) {
    let patch_id = ma.new_patch().unwrap();

    let zip_def = ma.get_agent_definition(ZIP_TO_ARRAY_DEF).unwrap();
    let mut zip_spec = zip_def.to_spec();
    if let Some(configs) = zip_spec.configs.as_mut() {
        configs.set("use_ctx".into(), AgentValue::boolean(true));
    }
    let zip_id = ma.add_agent(patch_id.clone(), zip_spec).await.unwrap();

    let probe_def = ma.get_agent_definition(PROBE_DEF).unwrap();
    let probe_id = ma
        .add_agent(patch_id.clone(), probe_def.to_spec())
        .await
        .unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: zip_id.clone(),
            source_handle: "array".into(),
            target: probe_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();
    ma.start_patch(&patch_id).await.unwrap();

    let probe = probe_receiver(ma, &probe_id).await.unwrap();
    (zip_id, probe)
}

#[tokio::test]
async fn test_zip_to_array_context_mode_interleaved() {
    let ma = test_utils::setup_modular_agent().await;
    let (zip_id, probe) = setup_ctx_zip_with_probe(&ma).await;

    // Two flows interleave: flow B completes first even though flow A started
    // first, so values pair by ctx, not by arrival order (FIFO would pair 1
    // with 4 here)
    let ctx_a = AgentContext::new();
    let ctx_b = AgentContext::new();

    let agent = ma.get_agent(&zip_id).unwrap();
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

    let (out_ctx, value) = recv_probe(&probe).await.unwrap();
    assert_eq!(
        value,
        AgentValue::array(vector![AgentValue::integer(3), AgentValue::integer(4)])
    );
    assert_eq!(out_ctx.id(), ctx_b.id());

    {
        let mut guard = agent.lock().await;
        guard
            .process(ctx_a.clone(), "1".into(), AgentValue::integer(2))
            .await
            .unwrap();
    }

    let (out_ctx, value) = recv_probe(&probe).await.unwrap();
    assert_eq!(
        value,
        AgentValue::array(vector![AgentValue::integer(1), AgentValue::integer(2)])
    );
    assert_eq!(out_ctx.id(), ctx_a.id());

    ma.quit();
}
