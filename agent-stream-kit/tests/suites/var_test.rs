extern crate agent_stream_kit as askit;

use askit::{AgentValue, test_utils};
use serial_test::serial;

#[serial(board_group)]
#[tokio::test]
async fn test_var_routing() {
    let askit = test_utils::setup_askit().await;

    // load var streams
    let var_stream_id = test_utils::load_and_start_stream(&askit, "tests/streams/Core_Var.json")
        .await
        .unwrap();

    askit
        .write_var_value(&var_stream_id, "var1", AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_var_value(&var_stream_id, "var1", &AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_var_value(&var_stream_id, "var2", &AgentValue::string("hello"))
        .await
        .unwrap();

    askit.quit();
}
