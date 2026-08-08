extern crate modular_agent_core as ma;

use im::vector;
use ma::test_utils::{self, ProbeReceiver, probe_receiver, recv_probe};
use ma::{AgentContext, AgentValue, ConnectionSpec, ModularAgent};

const ZIP_TO_ARRAY_DEF: &str = "modular_agent_std::array::ZipToArrayAgent";
const PROBE_DEF: &str = "modular_agent_core::test_utils::TestProbeAgent";

/// Wire a context-mode ZipToArray to a probe on its `array` output.
async fn setup_ctx_zip_with_probe(ma: &ModularAgent) -> (String, ProbeReceiver) {
    let preset_id = ma.new_preset().unwrap();

    let zip_def = ma.get_agent_definition(ZIP_TO_ARRAY_DEF).unwrap();
    let mut zip_spec = zip_def.to_spec();
    if let Some(configs) = zip_spec.configs.as_mut() {
        configs.set("use_ctx".into(), AgentValue::boolean(true));
    }
    let zip_id = ma.add_agent(preset_id.clone(), zip_spec).await.unwrap();

    let probe_def = ma.get_agent_definition(PROBE_DEF).unwrap();
    let probe_id = ma
        .add_agent(preset_id.clone(), probe_def.to_spec())
        .await
        .unwrap();
    ma.add_connection(
        &preset_id,
        ConnectionSpec {
            source: zip_id.clone(),
            source_handle: "array".into(),
            target: probe_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();
    ma.start_preset(&preset_id).await.unwrap();

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
