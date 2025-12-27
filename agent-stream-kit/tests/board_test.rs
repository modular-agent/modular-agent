extern crate agent_stream_kit as askit;

use askit::{AgentValue, test_utils};

#[tokio::test]
async fn test_board_routing() {
    let askit = test_utils::setup_askit().await;

    // load board streams
    test_utils::load_and_start_stream(&askit, "tests/streams/Core_Board1.json")
        .await
        .unwrap();
    test_utils::load_and_start_stream(&askit, "tests/streams/Core_Board2.json")
        .await
        .unwrap();

    askit
        .write_board_value("board1".to_string(), AgentValue::string("hello"))
        .unwrap();

    test_utils::expect_board_value("board1", &AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_board_value("board2", &AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_board_value("out", &AgentValue::string("hello"))
        .await
        .unwrap();

    askit.quit();
}
