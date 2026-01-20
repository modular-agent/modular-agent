extern crate modular_agent_kit as mak;

use mak::{AgentError, AgentValue, test_utils};
use serial_test::serial;

#[serial(board_group)]
#[tokio::test]
async fn test_var_disabled_routing() {
    let mak = test_utils::setup_mak().await;

    // load var preset
    let var_preset_id =
        test_utils::load_and_start_preset(&mak, "tests/presets/Core_Var_disabled.json")
            .await
            .unwrap();

    mak
        .write_var_value(&var_preset_id, "var1", AgentValue::string("hello"))
        .await
        .unwrap();

    // var1 is diabled, but we sent "hello" to it, so the notification should still sent.
    test_utils::expect_var_value(&var_preset_id, "var1", &AgentValue::string("hello"))
        .await
        .unwrap();

    // var2 is disabled, so the notification should fail.
    let res =
        test_utils::expect_var_value(&var_preset_id, "var2", &AgentValue::string("hello")).await;
    assert!(matches!(res, Err(AgentError::SendMessageFailed(_))));

    mak.quit();
}
