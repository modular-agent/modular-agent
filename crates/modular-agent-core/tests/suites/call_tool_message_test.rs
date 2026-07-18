extern crate modular_agent_core as ma;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ma::test_utils::{TestProbeAgent, probe_receiver, recv_probe};
use ma::tool::{CallToolMessageAgent, Tool, ToolInfo, register_tool, unregister_tool};
use ma::{
    Agent, AgentContext, AgentError, AgentValue, AsAgent, ConnectionSpec, Message, ModularAgent,
    ToolCall, ToolCallFunction, async_trait,
};

const CALL_TOOL_MESSAGE_DEF: &str = CallToolMessageAgent::DEF_NAME;

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

    async fn call(&self, _ctx: AgentContext, _args: AgentValue) -> Result<AgentValue, AgentError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(AgentValue::string("ok"))
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
fn tool_call_message(tool_name: &str, id: Option<&str>, streaming: bool) -> AgentValue {
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
    AgentValue::message(msg)
}

async fn setup_agent(ma: &ModularAgent) -> CallToolMessageAgent {
    let def = ma.get_agent_definition(CALL_TOOL_MESSAGE_DEF).unwrap();
    let spec = def.to_spec();
    let mut agent =
        <CallToolMessageAgent as AsAgent>::new(ma.clone(), "call_tool_message".into(), spec)
            .unwrap();
    Agent::start(&mut agent).await.unwrap();
    agent
}

#[tokio::test]
async fn streaming_message_does_not_execute_tool() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();
    let tool_name = "call_tool_message_test_streaming";
    let count = register_counting_tool(tool_name);
    let mut agent = setup_agent(&ma).await;

    let ctx = AgentContext::new();
    let value = tool_call_message(tool_name, Some("call1"), true);
    Agent::process(&mut agent, ctx, "message".into(), value)
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
    let mut agent = setup_agent(&ma).await;

    // Same ctx and same call id delivered twice (e.g. Claude's duplicate final emit).
    let ctx = AgentContext::new();
    for _ in 0..2 {
        let value = tool_call_message(tool_name, Some("call1"), false);
        Agent::process(&mut agent, ctx.clone(), "message".into(), value)
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

    // Wire CallToolMessageAgent's message output to a probe so the synthetic
    // tool results can be observed.
    let preset_id = ma.new_preset().unwrap();
    let call_def = ma.get_agent_definition(CALL_TOOL_MESSAGE_DEF).unwrap();
    let call_agent_id = ma
        .add_agent(preset_id.clone(), call_def.to_spec())
        .await
        .unwrap();
    let probe_def = ma.get_agent_definition(TestProbeAgent::DEF_NAME).unwrap();
    let probe_agent_id = ma
        .add_agent(preset_id.clone(), probe_def.to_spec())
        .await
        .unwrap();
    ma.add_connection(
        &preset_id,
        ConnectionSpec {
            source: call_agent_id.clone(),
            source_handle: "message".into(),
            target: probe_agent_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();
    ma.start_preset(&preset_id).await.unwrap();
    let probe_rx = probe_receiver(&ma, &probe_agent_id).await.unwrap();

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

    let agent = ma.get_agent(&call_agent_id).unwrap();
    agent
        .lock()
        .await
        .process(
            AgentContext::new(),
            "message".into(),
            AgentValue::message(msg),
        )
        .await
        .unwrap();

    for expected_id in ["call1", "call2"] {
        let (_ctx, value) = recv_probe(&probe_rx).await.unwrap();
        let resp = value.as_message().unwrap();
        assert_eq!(resp.role, "tool");
        assert_eq!(resp.id.as_deref(), Some(expected_id));
        assert_eq!(resp.tool_name.as_deref(), Some(tool_name));
        assert_eq!(resp.is_error, Some(true));
        assert!(resp.content.contains("token limit"));
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
    let mut agent = setup_agent(&ma).await;

    // Calls without an id keep legacy behavior: every delivery executes.
    let ctx = AgentContext::new();
    for _ in 0..2 {
        let value = tool_call_message(tool_name, None, false);
        Agent::process(&mut agent, ctx.clone(), "message".into(), value)
            .await
            .unwrap();
    }

    assert_eq!(count.load(Ordering::SeqCst), 2);

    unregister_tool(tool_name);
    ma.quit();
}
