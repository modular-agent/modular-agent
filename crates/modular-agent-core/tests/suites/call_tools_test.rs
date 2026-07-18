extern crate modular_agent_core as ma;

use ma::tool::{Tool, ToolInfo, call_tools, register_tool, unregister_tool};
use ma::{AgentContext, AgentError, AgentValue, Message, ToolCall, ToolCallFunction, async_trait};

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
    assert!(!msg.content.is_empty());
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
    let messages = call_tools(&ctx, &calls).await.unwrap();

    assert_eq!(messages.len(), 2);
    assert_error_result(&messages[0], failing, "call1");
    assert_eq!(messages[1].role, "tool");
    assert_eq!(messages[1].tool_name.as_deref(), Some(succeeding));
    assert_eq!(messages[1].id.as_deref(), Some("call2"));
    assert_eq!(messages[1].is_error, None);
    assert_eq!(messages[1].content, "\"ok\"");

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
    let messages = call_tools(&ctx, &calls).await.unwrap();

    assert_eq!(messages.len(), 1);
    assert_error_result(&messages[0], succeeding, "call1");
    // The tool must not have run: a SucceedingTool result would be "ok".
    assert_ne!(messages[0].content, "\"ok\"");

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
    let messages = call_tools(&ctx, &calls).await.unwrap();

    assert_eq!(messages.len(), 1);
    assert_error_result(&messages[0], "call_tools_test_not_registered", "call1");
}

#[tokio::test]
async fn error_results_survive_message_serde() {
    let failing = "call_tools_test_serde";
    register_tool(FailingTool {
        info: tool_info(failing),
    });

    let ctx = AgentContext::new();
    let calls = im::vector![tool_call(failing, "call1", serde_json::json!({}))];
    let messages = call_tools(&ctx, &calls).await.unwrap();
    assert_eq!(messages.len(), 1);

    let json = serde_json::to_value(&messages[0]).unwrap();
    let restored: Message = serde_json::from_value(json).unwrap();
    assert_error_result(&restored, failing, "call1");

    unregister_tool(failing);
}
