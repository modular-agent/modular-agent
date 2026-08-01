//! E2E loopback test: the built-in MCP server (streamable HTTP) is driven
//! through the MCP client connection pool via a url-form mcp.json entry.
//!
//! Exercises in one pass: paginated tool listing (`list_all_tools`), bearer
//! auth via `headers`, structured output on the server side (`Json<T>`
//! tools), and the client preferring `structuredContent` over the text
//! fallback when converting results.

#![cfg(all(feature = "mcp-server", feature = "mcp-http-client"))]

extern crate modular_agent_core as ma;

use std::net::Ipv4Addr;

use ma::mcp::{register_tools_from_mcp_json, shutdown_all_mcp_connections};
use ma::mcp_server::{McpServerConfig, start_mcp_server};
use ma::tool::get_tool;
use ma::{AgentContext, AgentValue, ModularAgent};
use serde_json::json;

const TOKEN: &str = "loopback-test-token";

#[tokio::test(flavor = "multi_thread")]
async fn http_loopback_lists_tools_and_returns_structured_content() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    // Reserve a free port; the window between drop and rebind is acceptable
    // for a test.
    let port = {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        listener.local_addr().unwrap().port()
    };

    let handle = start_mcp_server(
        ma.clone(),
        McpServerConfig {
            port,
            presets_dir: None,
            token: Some(TOKEN.into()),
        },
    )
    .await
    .unwrap();

    let scratch = tempfile::tempdir().unwrap();
    let config_path = scratch.path().join("mcp.json");
    let config = json!({
        "mcpServers": {
            "loopback": {
                "url": format!("http://127.0.0.1:{port}/mcp"),
                "headers": { "Authorization": format!("Bearer {TOKEN}") }
            }
        }
    });
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    // Registration walks the pooled client path: connect over streamable
    // HTTP with the auth header, then enumerate tools via list_all_tools.
    let tools = register_tools_from_mcp_json(&config_path).await.unwrap();
    for name in ["loopback::list_presets", "loopback::create_preset"] {
        assert!(tools.contains(&name.to_string()), "missing tool {name}");
    }

    // create_preset returns Json<CreatePresetResponse>: the client must
    // surface the structured payload as a parsed object, not as the text
    // fallback (which would arrive as an array of JSON strings).
    let create = get_tool("loopback::create_preset").unwrap();
    let created = create
        .call(
            AgentContext::new(),
            AgentValue::from_json(json!({"name": "loopback-e2e"})).unwrap(),
        )
        .await
        .unwrap();
    let preset_id = created
        .as_object()
        .expect("create_preset must return a structured object")
        .get("preset_id")
        .and_then(AgentValue::as_str)
        .expect("structured response must carry preset_id")
        .to_string();

    let list = get_tool("loopback::list_presets").unwrap();
    let listed = list
        .call(AgentContext::new(), AgentValue::object_default())
        .await
        .unwrap();
    let infos = listed
        .as_object()
        .and_then(|obj| obj.get("presets"))
        .and_then(AgentValue::as_array)
        .expect("list_presets must return a structured object with a presets array");
    assert!(
        infos.iter().any(|info| {
            info.as_object()
                .and_then(|obj| obj.get("id"))
                .and_then(AgentValue::as_str)
                == Some(preset_id.as_str())
        }),
        "created preset {preset_id} missing from list_presets result: {listed:?}"
    );

    shutdown_all_mcp_connections().await.unwrap();
    handle.stop().await;
    ma.quit();
}
