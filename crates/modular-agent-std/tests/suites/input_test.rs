extern crate modular_agent_core as ma;

use im::hashmap;
use ma::{AgentValue, test_utils};

#[tokio::test]
async fn test_boolean_input() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &preset_id, "boolean_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "boolean_out", &AgentValue::boolean(true))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_integer_input() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &preset_id, "integer_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "integer_out", &AgentValue::integer(1))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_number_input() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &preset_id, "number_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "number_out", &AgentValue::number(3.14))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_string_input() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &preset_id, "string_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "string_out",
        &AgentValue::string("Hello, world!".to_string()),
    )
    .await
    .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_text_input() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &preset_id, "text_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "text_out",
        &AgentValue::string("Old pond\nFrogs jumped in\nSound of water.\n"),
    )
    .await
    .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_object_input() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &preset_id, "object_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "object_out",
        &AgentValue::object(hashmap! {
            "name".to_string() => AgentValue::string("Alice".to_string()),
            "is_busy".to_string() => AgentValue::boolean(false),
        }),
    )
    .await
    .unwrap();

    ma.quit();
}
