extern crate modular_agent_core as ma;

use ma::{AgentValue, test_utils};
use serial_test::serial;

#[serial(external_group)]
#[tokio::test]
async fn test_var_routing() {
    let ma = test_utils::setup_modular_agent().await;

    // load var preset
    let var_preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Core_Var.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &var_preset_id, "var1", AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_local_value(&var_preset_id, "var2", &AgentValue::string("hello"))
        .await
        .unwrap();

    ma.quit();
}
