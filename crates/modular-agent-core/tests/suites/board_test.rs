extern crate modular_agent_core as ma;

use ma::{AgentValue, test_utils};
use serial_test::serial;

#[serial(board_group)]
#[tokio::test]
async fn test_board_routing() {
    let ma = test_utils::setup_mak().await;

    // load board presets
    test_utils::open_and_start_preset(&ma, "tests/presets/Core_Board1.json")
        .await
        .unwrap();
    test_utils::open_and_start_preset(&ma, "tests/presets/Core_Board2.json")
        .await
        .unwrap();

    ma
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

    ma.quit();
}
