extern crate modular_agent_core as ma;

use ma::test_utils::{self, probe_receiver, recv_probe};
use ma::{ConnectionSpec, ModuleContext, Value};

const COUNTER_DEF: &str = "modular_agent_std::utils::CounterModule";
const PROBE_DEF: &str = "modular_agent_core::test_utils::TestProbeModule";

#[tokio::test]
async fn test_counter_counts_and_resets() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = ma.new_patch().unwrap();
    let counter_id = ma
        .add_module(
            patch_id.clone(),
            ma.get_module_definition(COUNTER_DEF).unwrap().to_spec(),
        )
        .await
        .unwrap();
    let probe_id = ma
        .add_module(
            patch_id.clone(),
            ma.get_module_definition(PROBE_DEF).unwrap().to_spec(),
        )
        .await
        .unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: counter_id.clone(),
            source_handle: "count".into(),
            target: probe_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();
    ma.start_patch(&patch_id).await.unwrap();
    let probe = probe_receiver(&ma, &probe_id).await.unwrap();

    let ctx = ModuleContext::new();
    let module = ma.get_module(&counter_id).unwrap();
    {
        let mut guard = module.lock().await;
        for port in ["value", "value", "reset", "value"] {
            guard
                .process(ctx.clone(), port.into(), Value::unit())
                .await
                .unwrap();
        }
    }

    // Every input emits the running count; reset drops it back to 0
    for expected in [1, 2, 0, 1] {
        let (_ctx, value) = recv_probe(&probe).await.unwrap();
        assert_eq!(value, Value::integer(expected));
    }

    ma.quit();
}
