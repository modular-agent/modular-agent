extern crate modular_agent_core as ma;

use ma::{AgentValue, test_utils};
use serial_test::serial;

#[serial(external_group)]
#[tokio::test]
async fn test_external_routing() {
    let ma = test_utils::setup_modular_agent().await;

    // load external patches
    test_utils::open_and_start_patch(&ma, "tests/patches/Core_External1.json")
        .await
        .unwrap();
    test_utils::open_and_start_patch(&ma, "tests/patches/Core_External2.json")
        .await
        .unwrap();

    ma.write_external_input("channel1".to_string(), AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_external_output("channel1", &AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_external_output("channel2", &AgentValue::string("hello"))
        .await
        .unwrap();

    test_utils::expect_external_output("out", &AgentValue::string("hello"))
        .await
        .unwrap();

    ma.quit();
}
