extern crate modular_agent_core as ma;

use ma::{AgentValue, test_utils};
use serial_test::serial;

#[serial(board_group)]
#[tokio::test]
async fn test_var_routing() {
    let ma = test_utils::setup_mak().await;

    // load var preset
    let var_preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Core_Var.json")
        .await
        .unwrap();

    ma.write_var_value(&var_preset_id, "var1", AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_var_value(&var_preset_id, "var1", &AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_var_value(&var_preset_id, "var2", &AgentValue::string("hello"))
        .await
        .unwrap();

    ma.quit();
}
