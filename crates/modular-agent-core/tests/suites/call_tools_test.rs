extern crate modular_agent_core as ma;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use ma::tool::{ExecutionMode, Tool, ToolInfo, call_tools, register_tool, unregister_tool};
use ma::{AgentContext, AgentError, AgentValue, Message, ToolCall, ToolCallFunction, async_trait};
use tokio::sync::{Barrier, Notify};

/// Test tool that always fails.
struct FailingTool {
    info: ToolInfo,
}

#[async_trait]
impl Tool for FailingTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(&self, _ctx: AgentContext, _args: AgentValue) -> Result<AgentValue, AgentError> {
        Err(AgentError::Other("intentional failure".to_string()))
    }
}

/// Test tool that always succeeds with a fixed string.
struct SucceedingTool {
    info: ToolInfo,
}

#[async_trait]
impl Tool for SucceedingTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(&self, _ctx: AgentContext, _args: AgentValue) -> Result<AgentValue, AgentError> {
        Ok(AgentValue::string("ok"))
    }
}

fn tool_info(name: &str) -> ToolInfo {
    ToolInfo::new(name, "", None)
}

fn tool_call(name: &str, id: &str, parameters: serde_json::Value) -> ToolCall {
    ToolCall {
        function: ToolCallFunction {
            name: name.to_string(),
            parameters,
            id: Some(id.to_string()),
            parse_error: None,
        },
    }
}

fn assert_error_result(msg: &Message, tool_name: &str, id: &str) {
    assert_eq!(msg.role, "tool");
    assert_eq!(msg.tool_name.as_deref(), Some(tool_name));
    assert_eq!(msg.id.as_deref(), Some(id));
    assert_eq!(msg.is_error, Some(true));
    assert!(!msg.text().is_empty());
}

#[tokio::test]
async fn failing_tool_yields_error_result_without_aborting_others() {
    let failing = "call_tools_test_failing";
    let succeeding = "call_tools_test_succeeding";
    register_tool(FailingTool {
        info: tool_info(failing),
    });
    register_tool(SucceedingTool {
        info: tool_info(succeeding),
    });

    let ctx = AgentContext::new();
    let calls = im::vector![
        tool_call(failing, "call1", serde_json::json!({})),
        tool_call(succeeding, "call2", serde_json::json!({})),
    ];
    let messages = call_tools(&ctx, &calls, 8).await.unwrap();

    assert_eq!(messages.len(), 2);
    assert_error_result(&messages[0], failing, "call1");
    assert_eq!(messages[1].role, "tool");
    assert_eq!(messages[1].tool_name.as_deref(), Some(succeeding));
    assert_eq!(messages[1].id.as_deref(), Some("call2"));
    assert_eq!(messages[1].is_error, None);
    assert_eq!(messages[1].text(), "\"ok\"");

    unregister_tool(failing);
    unregister_tool(succeeding);
}

#[tokio::test]
async fn parse_error_call_yields_error_result_without_executing() {
    let succeeding = "call_tools_test_parse_error_guard";
    register_tool(SucceedingTool {
        info: tool_info(succeeding),
    });

    let mut call = tool_call(succeeding, "call1", serde_json::json!({}));
    call.function.parse_error = Some("expected value at line 1 column 1".to_string());

    let ctx = AgentContext::new();
    let calls = im::vector![call];
    let messages = call_tools(&ctx, &calls, 8).await.unwrap();

    assert_eq!(messages.len(), 1);
    assert_error_result(&messages[0], succeeding, "call1");
    // The tool must not have run: a SucceedingTool result would be "ok".
    assert_ne!(messages[0].text(), "\"ok\"");

    unregister_tool(succeeding);
}

#[tokio::test]
async fn unknown_tool_yields_error_result() {
    let ctx = AgentContext::new();
    let calls = im::vector![tool_call(
        "call_tools_test_not_registered",
        "call1",
        serde_json::json!({})
    )];
    let messages = call_tools(&ctx, &calls, 8).await.unwrap();

    assert_eq!(messages.len(), 1);
    assert_error_result(&messages[0], "call_tools_test_not_registered", "call1");
}

fn parallel_info(name: &str) -> ToolInfo {
    ToolInfo::new(name, "", None).with_execution_mode(ExecutionMode::Parallel)
}

/// Parallel tool that waits on a shared barrier; it only completes when
/// another party reaches the barrier while this call is still in flight.
struct BarrierTool {
    info: ToolInfo,
    barrier: Arc<Barrier>,
}

#[async_trait]
impl Tool for BarrierTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(&self, _ctx: AgentContext, _args: AgentValue) -> Result<AgentValue, AgentError> {
        self.barrier.wait().await;
        Ok(AgentValue::string("ok"))
    }
}

/// Parallel tool that completes only after another tool notifies it.
struct WaitNotifyTool {
    info: ToolInfo,
    notify: Arc<Notify>,
}

#[async_trait]
impl Tool for WaitNotifyTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(&self, _ctx: AgentContext, _args: AgentValue) -> Result<AgentValue, AgentError> {
        self.notify.notified().await;
        Ok(AgentValue::string("slow"))
    }
}

/// Parallel tool that immediately notifies its peer and returns.
struct TriggerNotifyTool {
    info: ToolInfo,
    notify: Arc<Notify>,
}

#[async_trait]
impl Tool for TriggerNotifyTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(&self, _ctx: AgentContext, _args: AgentValue) -> Result<AgentValue, AgentError> {
        self.notify.notify_one();
        Ok(AgentValue::string("fast"))
    }
}

/// Tool that records start/end events into a shared log, yielding in between
/// so any concurrently running call gets a chance to interleave its events.
struct LoggingTool {
    info: ToolInfo,
    label: &'static str,
    log: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Tool for LoggingTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(&self, _ctx: AgentContext, _args: AgentValue) -> Result<AgentValue, AgentError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("start:{}", self.label));
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }
        self.log.lock().unwrap().push(format!("end:{}", self.label));
        Ok(AgentValue::string("ok"))
    }
}

/// Tool that tracks how many calls are in flight at once, yielding so that
/// concurrent calls can overlap if the scheduler allows them to.
struct CountingTool {
    info: ToolInfo,
    in_flight: Arc<AtomicUsize>,
    max_in_flight: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for CountingTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(&self, _ctx: AgentContext, _args: AgentValue) -> Result<AgentValue, AgentError> {
        let current = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_in_flight.fetch_max(current, Ordering::SeqCst);
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        Ok(AgentValue::string("ok"))
    }
}

#[tokio::test]
async fn parallel_tools_genuinely_overlap() {
    let names = ["call_tools_test_overlap_a", "call_tools_test_overlap_b"];
    let barrier = Arc::new(Barrier::new(2));
    for name in names {
        register_tool(BarrierTool {
            info: parallel_info(name),
            barrier: barrier.clone(),
        });
    }

    let ctx = AgentContext::new();
    let calls = im::vector![
        tool_call(names[0], "c1", serde_json::json!({})),
        tool_call(names[1], "c2", serde_json::json!({})),
    ];
    // If the calls ran sequentially the first would block on the barrier
    // forever; the timeout turns that hang into a test failure.
    let messages = tokio::time::timeout(Duration::from_secs(5), call_tools(&ctx, &calls, 8))
        .await
        .expect("parallel tools did not overlap: barrier never released")
        .unwrap();

    for name in names {
        unregister_tool(name);
    }
    assert_eq!(messages.len(), 2);
    for msg in &messages {
        assert_eq!(msg.is_error, None);
        assert_eq!(msg.text(), "\"ok\"");
    }
}

#[tokio::test]
async fn parallel_results_keep_input_order() {
    let slow = "call_tools_test_order_slow";
    let fast = "call_tools_test_order_fast";
    let notify = Arc::new(Notify::new());
    register_tool(WaitNotifyTool {
        info: parallel_info(slow),
        notify: notify.clone(),
    });
    register_tool(TriggerNotifyTool {
        info: parallel_info(fast),
        notify: notify.clone(),
    });

    let ctx = AgentContext::new();
    // The first call cannot finish until the second one has already
    // completed, so completion order is the reverse of input order.
    let calls = im::vector![
        tool_call(slow, "c1", serde_json::json!({})),
        tool_call(fast, "c2", serde_json::json!({})),
    ];
    let messages = tokio::time::timeout(Duration::from_secs(5), call_tools(&ctx, &calls, 8))
        .await
        .expect("slow tool was never notified: calls did not run concurrently")
        .unwrap();

    unregister_tool(slow);
    unregister_tool(fast);

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].tool_name.as_deref(), Some(slow));
    assert_eq!(messages[0].text(), "\"slow\"");
    assert_eq!(messages[1].tool_name.as_deref(), Some(fast));
    assert_eq!(messages[1].text(), "\"fast\"");
}

#[tokio::test]
async fn sequential_call_is_barrier_in_mixed_batch() {
    let p1 = "call_tools_test_mixed_p1";
    let p2 = "call_tools_test_mixed_p2";
    let s = "call_tools_test_mixed_s";
    let p3 = "call_tools_test_mixed_p3";
    let log = Arc::new(Mutex::new(Vec::new()));
    for (name, label, parallel) in [
        (p1, "p1", true),
        (p2, "p2", true),
        (s, "s", false),
        (p3, "p3", true),
    ] {
        let info = if parallel {
            parallel_info(name)
        } else {
            tool_info(name)
        };
        register_tool(LoggingTool {
            info,
            label,
            log: log.clone(),
        });
    }

    let ctx = AgentContext::new();
    let calls = im::vector![
        tool_call(p1, "c1", serde_json::json!({})),
        tool_call(p2, "c2", serde_json::json!({})),
        tool_call(s, "c3", serde_json::json!({})),
        tool_call(p3, "c4", serde_json::json!({})),
    ];
    let messages = call_tools(&ctx, &calls, 8).await.unwrap();

    for name in [p1, p2, s, p3] {
        unregister_tool(name);
    }

    assert_eq!(messages.len(), 4);
    for (msg, name) in messages.iter().zip([p1, p2, s, p3]) {
        assert_eq!(msg.tool_name.as_deref(), Some(name));
        assert_eq!(msg.is_error, None);
    }

    let events = log.lock().unwrap().clone();
    let pos = |event: &str| {
        events
            .iter()
            .position(|e| e == event)
            .unwrap_or_else(|| panic!("event {:?} missing from {:?}", event, events))
    };

    // p1 and p2 overlap: both start before either finishes.
    let latest_start = pos("start:p1").max(pos("start:p2"));
    let earliest_end = pos("end:p1").min(pos("end:p2"));
    assert!(
        latest_start < earliest_end,
        "p1/p2 did not overlap: {:?}",
        events
    );

    // The sequential call starts only after the whole parallel batch ended...
    assert!(pos("end:p1") < pos("start:s"), "events: {:?}", events);
    assert!(pos("end:p2") < pos("start:s"), "events: {:?}", events);
    // ...runs alone (no event interleaves between its start and end)...
    assert_eq!(pos("end:s"), pos("start:s") + 1, "events: {:?}", events);
    // ...and the following parallel call starts only after it finished.
    assert!(pos("end:s") < pos("start:p3"), "events: {:?}", events);
}

#[tokio::test]
async fn sequential_tools_never_overlap() {
    let names = ["call_tools_test_seq_a", "call_tools_test_seq_b"];
    let in_flight = Arc::new(AtomicUsize::new(0));
    let max_in_flight = Arc::new(AtomicUsize::new(0));
    for name in names {
        register_tool(CountingTool {
            info: tool_info(name),
            in_flight: in_flight.clone(),
            max_in_flight: max_in_flight.clone(),
        });
    }

    let ctx = AgentContext::new();
    let calls = im::vector![
        tool_call(names[0], "c1", serde_json::json!({})),
        tool_call(names[1], "c2", serde_json::json!({})),
    ];
    let messages = call_tools(&ctx, &calls, 8).await.unwrap();

    for name in names {
        unregister_tool(name);
    }
    assert_eq!(messages.len(), 2);
    assert_eq!(max_in_flight.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn failing_parallel_call_yields_error_without_affecting_others() {
    let failing = "call_tools_test_parallel_failing";
    let succeeding = "call_tools_test_parallel_succeeding";
    register_tool(FailingTool {
        info: parallel_info(failing),
    });
    register_tool(SucceedingTool {
        info: parallel_info(succeeding),
    });

    let ctx = AgentContext::new();
    let calls = im::vector![
        tool_call(failing, "c1", serde_json::json!({})),
        tool_call(succeeding, "c2", serde_json::json!({})),
    ];
    let messages = call_tools(&ctx, &calls, 8).await.unwrap();

    unregister_tool(failing);
    unregister_tool(succeeding);

    assert_eq!(messages.len(), 2);
    assert_error_result(&messages[0], failing, "c1");
    assert_eq!(messages[1].is_error, None);
    assert_eq!(messages[1].text(), "\"ok\"");
}

#[tokio::test]
async fn max_concurrency_caps_in_flight_calls() {
    let names = [
        "call_tools_test_cap_a",
        "call_tools_test_cap_b",
        "call_tools_test_cap_c",
        "call_tools_test_cap_d",
    ];
    let in_flight = Arc::new(AtomicUsize::new(0));
    let max_in_flight = Arc::new(AtomicUsize::new(0));
    for name in names {
        register_tool(CountingTool {
            info: parallel_info(name),
            in_flight: in_flight.clone(),
            max_in_flight: max_in_flight.clone(),
        });
    }

    let ctx = AgentContext::new();
    let calls = names
        .into_iter()
        .enumerate()
        .map(|(i, name)| tool_call(name, &format!("c{}", i), serde_json::json!({})))
        .collect::<im::Vector<_>>();
    let messages = call_tools(&ctx, &calls, 2).await.unwrap();

    for name in names {
        unregister_tool(name);
    }
    assert_eq!(messages.len(), 4);
    let max = max_in_flight.load(Ordering::SeqCst);
    assert!(max <= 2, "max_concurrency=2 exceeded: {} in flight", max);
    // The yields inside CountingTool guarantee the buffer actually fills, so
    // anything below 2 would mean the batch degenerated to sequential.
    assert_eq!(max, 2);
}

#[tokio::test]
async fn error_results_survive_message_serde() {
    let failing = "call_tools_test_serde";
    register_tool(FailingTool {
        info: tool_info(failing),
    });

    let ctx = AgentContext::new();
    let calls = im::vector![tool_call(failing, "call1", serde_json::json!({}))];
    let messages = call_tools(&ctx, &calls, 8).await.unwrap();
    assert_eq!(messages.len(), 1);

    let json = serde_json::to_value(&messages[0]).unwrap();
    let restored: Message = serde_json::from_value(json).unwrap();
    assert_error_result(&restored, failing, "call1");

    unregister_tool(failing);
}
