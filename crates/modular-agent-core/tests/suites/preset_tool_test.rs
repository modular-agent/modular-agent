extern crate modular_agent_core as ma;

use ma::tool::get_tool;
use ma::{AgentValue, ModularAgent};

const PRESET_TOOL_DEF: &str = "modular_agent_core::tool::PresetToolAgent";

#[tokio::test]
async fn configs_changed_reregisters_running_tool() {
    // Unique names to avoid clashes in the process-global registry
    // shared with other tests running in parallel.
    let old_name = "preset_tool_test_cfg_old";
    let new_name = "preset_tool_test_cfg_new";

    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let preset_id = ma.new_preset().unwrap();
    let def = ma.get_agent_definition(PRESET_TOOL_DEF).unwrap();
    let spec = def.to_spec();
    let agent_id = ma.add_agent(preset_id.clone(), spec).await.unwrap();

    // Drive the agent lifecycle directly through its handle so each step is
    // observable synchronously (start_agent spawns the start asynchronously).
    let agent = ma.get_agent(&agent_id).unwrap();

    {
        let mut guard = agent.lock().await;
        guard
            .set_config("name".into(), AgentValue::string(old_name))
            .unwrap();
        guard
            .set_config("description".into(), AgentValue::string("old description"))
            .unwrap();
    }
    // Config changes before start must not register anything.
    assert!(get_tool(old_name).is_none());

    agent.lock().await.start().await.unwrap();
    assert!(get_tool(old_name).is_some());

    let parameters = serde_json::json!({
        "type": "object",
        "properties": { "q": { "type": "string" } },
    });
    {
        let mut guard = agent.lock().await;
        guard
            .set_config("description".into(), AgentValue::string("new description"))
            .unwrap();
        guard
            .set_config(
                "parameters".into(),
                AgentValue::from_json(parameters.clone()).unwrap(),
            )
            .unwrap();
        guard
            .set_config("name".into(), AgentValue::string(new_name))
            .unwrap();
    }

    // The rename must drop the old registration and serve the new info.
    assert!(get_tool(old_name).is_none());
    let tool = get_tool(new_name).unwrap();
    assert_eq!(tool.info().name, new_name);
    assert_eq!(tool.info().description, "new description");
    assert_eq!(tool.info().parameters, parameters);

    agent.lock().await.stop().await.unwrap();
    assert!(get_tool(new_name).is_none());

    ma.quit();
}
