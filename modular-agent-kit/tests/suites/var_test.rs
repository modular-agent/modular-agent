extern crate modular_agent_kit as mak;

use mak::{AgentValue, test_utils};
use serial_test::serial;

#[serial(board_group)]
#[tokio::test]
async fn test_var_routing() {
    let mak = test_utils::setup_mak().await;

    // load var preset
    let var_preset_id = test_utils::open_and_start_preset(&mak, "tests/presets/Core_Var.json")
        .await
        .unwrap();

    mak.write_var_value(&var_preset_id, "var1", AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_var_value(&var_preset_id, "var1", &AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_var_value(&var_preset_id, "var2", &AgentValue::string("hello"))
        .await
        .unwrap();

    mak.quit();
}
