extern crate modular_agent_kit as mak;

use mak::{AgentValue, test_utils};
use serial_test::serial;

#[serial(board_group)]
#[tokio::test]
async fn test_board_routing() {
    let mak = test_utils::setup_mak().await;

    // load board presets
    test_utils::load_and_start_preset(&mak, "tests/presets/Core_Board1.json")
        .await
        .unwrap();
    test_utils::load_and_start_preset(&mak, "tests/presets/Core_Board2.json")
        .await
        .unwrap();

    mak
        .write_board_value("board1".to_string(), AgentValue::string("hello"))
        .await
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

    mak.quit();
}
