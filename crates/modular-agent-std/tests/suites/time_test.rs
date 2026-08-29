extern crate modular_agent_core as ma;

use std::time::Duration;

use ma::test_utils::{self, ProbeReceiver, probe_receiver, recv_probe_with_timeout};
use ma::{AgentConfigs, AgentContext, AgentValue, ConnectionSpec, ModularAgent};

const DELAY_DEF: &str = "modular_agent_std::time::DelayAgent";
const INTERVAL_DEF: &str = "modular_agent_std::time::IntervalTimerAgent";
const ON_START_DEF: &str = "modular_agent_std::time::OnStartAgent";
const SCHEDULE_DEF: &str = "modular_agent_std::time::ScheduleTimerAgent";
const THROTTLE_DEF: &str = "modular_agent_std::time::ThrottleTimeAgent";
const PROBE_DEF: &str = "modular_agent_core::test_utils::TestProbeAgent";

// Timer emissions run in real time on the shared agent runtime, so these tests use
// small intervals, generous receive timeouts, and assert only counts, values and
// per-path order - never exact timing.
const TIMER_TIMEOUT: Duration = Duration::from_secs(5);
const QUIET_TIMEOUT: Duration = Duration::from_millis(400);

/// Wire one agent of `def_name` (configs adjusted by `configure`) to a probe on
/// `out_port` and start the patch.
async fn setup_agent_with_probe(
    ma: &ModularAgent,
    def_name: &str,
    out_port: &str,
    configure: impl FnOnce(&mut AgentConfigs),
) -> (String, String, ProbeReceiver) {
    let patch_id = ma.new_patch().unwrap();

    let mut spec = ma.get_agent_definition(def_name).unwrap().to_spec();
    if let Some(configs) = spec.configs.as_mut() {
        configure(configs);
    }
    let agent_id = ma.add_agent(patch_id.clone(), spec).await.unwrap();

    let probe_def = ma.get_agent_definition(PROBE_DEF).unwrap();
    let probe_id = ma
        .add_agent(patch_id.clone(), probe_def.to_spec())
        .await
        .unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: agent_id.clone(),
            source_handle: out_port.into(),
            target: probe_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();
    ma.start_patch(&patch_id).await.unwrap();

    let probe = probe_receiver(ma, &probe_id).await.unwrap();
    (patch_id, agent_id, probe)
}

#[tokio::test]
async fn test_delay_reemits_in_order() {
    let ma = test_utils::setup_modular_agent().await;
    let (_patch_id, delay_id, probe) = setup_agent_with_probe(&ma, DELAY_DEF, "value", |cfg| {
        cfg.set("delay".into(), AgentValue::integer(50));
    })
    .await;

    let ctx = AgentContext::new();
    let agent = ma.get_agent(&delay_id).unwrap();
    {
        let mut guard = agent.lock().await;
        guard
            .process(ctx.clone(), "value".into(), AgentValue::integer(1))
            .await
            .unwrap();
        guard
            .process(ctx.clone(), "value".into(), AgentValue::integer(2))
            .await
            .unwrap();
    }

    // Each value is re-emitted on the same port with the ctx it arrived with
    let (out_ctx, value) = recv_probe_with_timeout(&probe, TIMER_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(value, AgentValue::integer(1));
    assert_eq!(out_ctx.id(), ctx.id());
    let (out_ctx, value) = recv_probe_with_timeout(&probe, TIMER_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(value, AgentValue::integer(2));
    assert_eq!(out_ctx.id(), ctx.id());

    ma.quit();
}

#[tokio::test]
async fn test_interval_timer_ticks_and_stops() {
    let ma = test_utils::setup_modular_agent().await;
    let (patch_id, _agent_id, probe) = setup_agent_with_probe(&ma, INTERVAL_DEF, "unit", |cfg| {
        cfg.set("interval".into(), AgentValue::string("100ms"));
    })
    .await;

    // The first tick fires after one interval, then keeps repeating
    for _ in 0..2 {
        let (_ctx, value) = recv_probe_with_timeout(&probe, TIMER_TIMEOUT)
            .await
            .unwrap();
        assert_eq!(value, AgentValue::unit());
    }

    // After stop, no runaway ticks keep reaching the graph (a few queued
    // leftovers may still drain first)
    ma.stop_patch(&patch_id).await.unwrap();
    let mut leftovers = 0;
    while recv_probe_with_timeout(&probe, QUIET_TIMEOUT).await.is_ok() {
        leftovers += 1;
        assert!(leftovers < 10, "interval timer still ticking after stop");
    }

    ma.quit();
}

#[tokio::test]
async fn test_on_start_emits_unit_once() {
    let ma = test_utils::setup_modular_agent().await;
    let (_patch_id, _agent_id, probe) = setup_agent_with_probe(&ma, ON_START_DEF, "unit", |cfg| {
        cfg.set("delay".into(), AgentValue::integer(50));
    })
    .await;

    let (_ctx, value) = recv_probe_with_timeout(&probe, TIMER_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(value, AgentValue::unit());

    // One-shot: nothing further arrives
    assert!(
        recv_probe_with_timeout(&probe, QUIET_TIMEOUT)
            .await
            .is_err()
    );

    ma.quit();
}

#[tokio::test]
async fn test_schedule_timer_fires_on_cron() {
    let ma = test_utils::setup_modular_agent().await;
    // Every second; the output is the local timestamp in seconds
    let (_patch_id, _agent_id, probe) = setup_agent_with_probe(&ma, SCHEDULE_DEF, "time", |cfg| {
        cfg.set("schedule".into(), AgentValue::string("* * * * * * *"));
    })
    .await;

    let (_ctx, value) = recv_probe_with_timeout(&probe, Duration::from_secs(10))
        .await
        .unwrap();
    assert!(
        matches!(value, AgentValue::Integer(_)),
        "schedule timer must emit an integer timestamp, got {:?}",
        value
    );

    ma.quit();
}

/// Injects `values` into the throttle in one burst while holding the agent lock.
async fn burst(ma: &ModularAgent, agent_id: &str, ctx: &AgentContext, values: &[i64]) {
    let agent = ma.get_agent(agent_id).unwrap();
    let mut guard = agent.lock().await;
    for v in values {
        guard
            .process(ctx.clone(), "value".into(), AgentValue::integer(*v))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn test_throttle_capacity0_leading_edge_only() {
    let ma = test_utils::setup_modular_agent().await;
    let (_patch_id, throttle_id, probe) =
        setup_agent_with_probe(&ma, THROTTLE_DEF, "value", |cfg| {
            cfg.set("interval".into(), AgentValue::string("200ms"));
            cfg.set("capacity".into(), AgentValue::integer(0));
        })
        .await;

    let ctx = AgentContext::new();
    burst(&ma, &throttle_id, &ctx, &[1, 2, 3]).await;

    // Leading edge passes, the rest of the window is dropped
    let (_c, value) = recv_probe_with_timeout(&probe, TIMER_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(value, AgentValue::integer(1));
    assert!(
        recv_probe_with_timeout(&probe, QUIET_TIMEOUT)
            .await
            .is_err()
    );

    // The idle timer self-terminates, so the next value is a fresh leading edge
    burst(&ma, &throttle_id, &ctx, &[4]).await;
    let (_c, value) = recv_probe_with_timeout(&probe, TIMER_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(value, AgentValue::integer(4));

    ma.quit();
}

#[tokio::test]
async fn test_throttle_capacity1_keeps_newest() {
    let ma = test_utils::setup_modular_agent().await;
    let (_patch_id, throttle_id, probe) =
        setup_agent_with_probe(&ma, THROTTLE_DEF, "value", |cfg| {
            cfg.set("interval".into(), AgentValue::string("200ms"));
            cfg.set("capacity".into(), AgentValue::integer(1));
        })
        .await;

    let ctx = AgentContext::new();
    burst(&ma, &throttle_id, &ctx, &[1, 2, 3]).await;

    // Leading edge, then only the newest waiting value survives the window
    let (_c, value) = recv_probe_with_timeout(&probe, TIMER_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(value, AgentValue::integer(1));
    let (_c, value) = recv_probe_with_timeout(&probe, TIMER_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(value, AgentValue::integer(3));
    assert!(
        recv_probe_with_timeout(&probe, QUIET_TIMEOUT)
            .await
            .is_err()
    );

    ma.quit();
}

#[tokio::test]
async fn test_throttle_unbounded_queues_all() {
    let ma = test_utils::setup_modular_agent().await;
    let (_patch_id, throttle_id, probe) =
        setup_agent_with_probe(&ma, THROTTLE_DEF, "value", |cfg| {
            cfg.set("interval".into(), AgentValue::string("100ms"));
            cfg.set("capacity".into(), AgentValue::integer(-1));
        })
        .await;

    let ctx = AgentContext::new();
    burst(&ma, &throttle_id, &ctx, &[1, 2, 3]).await;

    // Rate-limited queue: everything comes out, one per window, in order
    for expected in [1, 2, 3] {
        let (_c, value) = recv_probe_with_timeout(&probe, TIMER_TIMEOUT)
            .await
            .unwrap();
        assert_eq!(value, AgentValue::integer(expected));
    }
    assert!(
        recv_probe_with_timeout(&probe, QUIET_TIMEOUT)
            .await
            .is_err()
    );

    ma.quit();
}
