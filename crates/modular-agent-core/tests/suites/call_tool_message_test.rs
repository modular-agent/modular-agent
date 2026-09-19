extern crate modular_agent_core as ma;

use modular_agent_core::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ma::test_utils::{TestProbeModule, probe_receiver, recv_probe};
use ma::tool::{CallToolMessageModule, Tool, ToolInfo, register_tool, unregister_tool};
use ma::{
    AsModule, ConnectionSpec, Message, ModularAgent, Module, ModuleContext, ToolCall,
    ToolCallFunction, Value, async_trait,
};

const CALL_TOOL_MESSAGE_DEF: &str = CallToolMessageModule::DEF_NAME;

/// Test tool that counts how many times it has been invoked.
struct CountingTool {
    info: ToolInfo,
    count: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for CountingTool {
    fn info(&self) -> &ToolInfo {
        &self.info
    }

    async fn call(&self, _ctx: ModuleContext, _args: Value) -> Result<Value> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(Value::string("ok"))
    }
}

/// Registers a counting tool and returns its shared call counter.
fn register_counting_tool(name: &str) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    register_tool(CountingTool {
        info: ToolInfo::new(name, "", None),
        count: count.clone(),
    });
    count
}

/// Builds an assistant message carrying a single tool call.
fn tool_call_message(tool_name: &str, id: Option<&str>, streaming: bool) -> Value {
    let mut msg = Message::assistant(String::new());
    msg.streaming = streaming;
    msg.tool_calls = Some(
        vec![ToolCall {
            function: ToolCallFunction {
                id: id.map(|s| s.to_string()),
                name: tool_name.to_string(),
                parameters: serde_json::json!({}),
                parse_error: None,
            },
        }]
        .into(),
    );
    Value::message(msg)
}

async fn setup_module(ma: &ModularAgent) -> CallToolMessageModule {
    let def = ma.get_module_definition(CALL_TOOL_MESSAGE_DEF).unwrap();
    let spec = def.to_spec();
    let mut module =
        <CallToolMessageModule as AsModule>::new(ma.clone(), "call_tool_message".into(), spec)
            .unwrap();
    Module::start(&mut module).await.unwrap();
    module
}

#[tokio::test]
async fn streaming_message_does_not_execute_tool() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();
    let tool_name = "call_tool_message_test_streaming";
    let count = register_counting_tool(tool_name);
    let mut module = setup_module(&ma).await;

    let ctx = ModuleContext::new();
    let value = tool_call_message(tool_name, Some("call1"), true);
    Module::process(&mut module, ctx, "message".into(), value)
        .await
        .unwrap();

    assert_eq!(count.load(Ordering::SeqCst), 0);

    unregister_tool(tool_name);
    ma.quit();
}

#[tokio::test]
async fn duplicate_call_id_executes_once() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();
    let tool_name = "call_tool_message_test_dedup";
    let count = register_counting_tool(tool_name);
    let mut module = setup_module(&ma).await;

    // Same ctx and same call id delivered twice (e.g. Claude's duplicate final emit).
    let ctx = ModuleContext::new();
    for _ in 0..2 {
        let value = tool_call_message(tool_name, Some("call1"), false);
        Module::process(&mut module, ctx.clone(), "message".into(), value)
            .await
            .unwrap();
    }

    assert_eq!(count.load(Ordering::SeqCst), 1);

    unregister_tool(tool_name);
    ma.quit();
}

#[tokio::test]
async fn length_stop_reason_skips_execution_and_synthesizes_error_results() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();
    let tool_name = "call_tool_message_test_length";
    let count = register_counting_tool(tool_name);

    // Wire CallToolMessageModule's message output to a probe so the synthetic
    // tool results can be observed.
    let patch_id = ma.new_patch().unwrap();
    let call_def = ma.get_module_definition(CALL_TOOL_MESSAGE_DEF).unwrap();
    let call_module_id = ma
        .add_module(patch_id.clone(), call_def.to_spec())
        .await
        .unwrap();
    let probe_def = ma.get_module_definition(TestProbeModule::DEF_NAME).unwrap();
    let probe_module_id = ma
        .add_module(patch_id.clone(), probe_def.to_spec())
        .await
        .unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: call_module_id.clone(),
            source_handle: "message".into(),
            target: probe_module_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();
    ma.start_patch(&patch_id).await.unwrap();
    let probe_rx = probe_receiver(&ma, &probe_module_id).await.unwrap();

    let mut msg = Message::assistant(String::new());
    msg.stop_reason = Some("length".to_string());
    msg.tool_calls = Some(
        vec![
            ToolCall {
                function: ToolCallFunction {
                    id: Some("call1".to_string()),
                    name: tool_name.to_string(),
                    parameters: serde_json::json!({}),
                    parse_error: None,
                },
            },
            ToolCall {
                function: ToolCallFunction {
                    id: Some("call2".to_string()),
                    name: tool_name.to_string(),
                    parameters: serde_json::json!({}),
                    parse_error: None,
                },
            },
        ]
        .into(),
    );

    let module = ma.get_module(&call_module_id).unwrap();
    module
        .lock()
        .await
        .process(ModuleContext::new(), "message".into(), Value::message(msg))
        .await
        .unwrap();

    for expected_id in ["call1", "call2"] {
        let (_ctx, value) = recv_probe(&probe_rx).await.unwrap();
        let resp = value.as_message().unwrap();
        assert_eq!(resp.role, "tool");
        assert_eq!(resp.id.as_deref(), Some(expected_id));
        assert_eq!(resp.tool_name.as_deref(), Some(tool_name));
        assert_eq!(resp.is_error, Some(true));
        assert!(resp.text().contains("token limit"));
    }
    assert_eq!(count.load(Ordering::SeqCst), 0);

    unregister_tool(tool_name);
    ma.quit();
}

#[tokio::test]
async fn missing_call_id_is_not_deduped() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();
    let tool_name = "call_tool_message_test_no_id";
    let count = register_counting_tool(tool_name);
    let mut module = setup_module(&ma).await;

    // Calls without an id keep legacy behavior: every delivery executes.
    let ctx = ModuleContext::new();
    for _ in 0..2 {
        let value = tool_call_message(tool_name, None, false);
        Module::process(&mut module, ctx.clone(), "message".into(), value)
            .await
            .unwrap();
    }

    assert_eq!(count.load(Ordering::SeqCst), 2);

    unregister_tool(tool_name);
    ma.quit();
}
