extern crate agent_stream_kit as askit;

use askit::{AgentError, AgentValue, test_utils};
use serial_test::serial;

#[serial(board_group)]
#[tokio::test]
async fn test_var_disabled_routing() {
    let askit = test_utils::setup_askit().await;

    // load var streams
    let var_stream_id =
        test_utils::load_and_start_stream(&askit, "tests/streams/Core_Var_disabled.json")
            .await
            .unwrap();

    askit
        .write_var_value(&var_stream_id, "var1", AgentValue::string("hello"))
        .await
        .unwrap();

    // var1 is diabled, but we sent "hello" to it, so the notification should still sent.
    test_utils::expect_var_value(&var_stream_id, "var1", &AgentValue::string("hello"))
        .await
        .unwrap();

    // var2 is disabled, so the notification should fail.
    let res =
        test_utils::expect_var_value(&var_stream_id, "var2", &AgentValue::string("hello")).await;
    assert!(matches!(res, Err(AgentError::SendMessageFailed(_))));

    askit.quit();
}
