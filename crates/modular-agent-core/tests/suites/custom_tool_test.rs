extern crate modular_agent_core as ma;

use ma::tool::get_tool;
use ma::{ModularAgent, Value};

const CUSTOM_TOOL_DEF: &str = "modular_agent_core::tool::CustomToolModule";

#[tokio::test]
async fn configs_changed_reregisters_running_tool() {
    // Unique names to avoid clashes in the process-global registry
    // shared with other tests running in parallel.
    let old_name = "custom_tool_test_cfg_old";
    let new_name = "custom_tool_test_cfg_new";

    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let patch_id = ma.new_patch().unwrap();
    let def = ma.get_module_definition(CUSTOM_TOOL_DEF).unwrap();
    let spec = def.to_spec();
    let module_id = ma.add_module(patch_id.clone(), spec).await.unwrap();

    // Drive the module lifecycle directly through its handle so each step is
    // observable synchronously (start_module spawns the start asynchronously).
    let module = ma.get_module(&module_id).unwrap();

    {
        let mut guard = module.lock().await;
        guard
            .set_config("name".into(), Value::string(old_name))
            .unwrap();
        guard
            .set_config("description".into(), Value::string("old description"))
            .unwrap();
    }
    // Config changes before start must not register anything.
    assert!(get_tool(old_name).is_none());

    module.lock().await.start().await.unwrap();
    assert!(get_tool(old_name).is_some());

    let parameters = serde_json::json!({
        "type": "object",
        "properties": { "q": { "type": "string" } },
    });
    {
        let mut guard = module.lock().await;
        guard
            .set_config("description".into(), Value::string("new description"))
            .unwrap();
        guard
            .set_config(
                "parameters".into(),
                Value::from_json(parameters.clone()).unwrap(),
            )
            .unwrap();
        guard
            .set_config("name".into(), Value::string(new_name))
            .unwrap();
    }

    // The rename must drop the old registration and serve the new info.
    assert!(get_tool(old_name).is_none());
    let tool = get_tool(new_name).unwrap();
    assert_eq!(tool.info().name, new_name);
    assert_eq!(tool.info().description, "new description");
    assert_eq!(tool.info().parameters, parameters);

    module.lock().await.stop().await.unwrap();
    assert!(get_tool(new_name).is_none());

    ma.quit();
}
