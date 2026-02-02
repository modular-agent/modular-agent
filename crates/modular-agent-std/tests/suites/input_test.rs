extern crate modular_agent_core as ma;

use im::hashmap;
use ma::{AgentValue, test_utils};

#[tokio::test]
async fn test_input() {
    let ma = test_utils::setup_modular_agent().await;

    // load input preset
    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_Input_test.json")
        .await
        .unwrap();

    // Boolean Input
    ma.write_local_input(&preset_id, "boolean_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "boolean_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "boolean_out", &AgentValue::boolean(false))
        .await
        .unwrap();

    ma.write_local_input(&preset_id, "boolean_conf", AgentValue::boolean(true))
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "boolean_conf", &AgentValue::boolean(true))
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "boolean_out", &AgentValue::boolean(true))
        .await
        .unwrap();
    ma.write_local_input(&preset_id, "boolean_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "boolean_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "boolean_out", &AgentValue::boolean(true))
        .await
        .unwrap();

    // Integer Input
    ma.write_local_input(&preset_id, "integer_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "integer_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "integer_out", &AgentValue::integer(0))
        .await
        .unwrap();

    ma.write_local_input(&preset_id, "integer_conf", AgentValue::integer(42))
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "integer_conf", &AgentValue::integer(42))
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "integer_out", &AgentValue::integer(42))
        .await
        .unwrap();
    ma.write_local_input(&preset_id, "integer_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "integer_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "integer_out", &AgentValue::integer(42))
        .await
        .unwrap();

    // Number Input
    ma.write_local_input(&preset_id, "number_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "number_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "number_out", &AgentValue::number(0.0))
        .await
        .unwrap();

    ma.write_local_input(&preset_id, "number_conf", AgentValue::number(3.14))
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "number_conf", &AgentValue::number(3.14))
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "number_out", &AgentValue::number(3.14))
        .await
        .unwrap();
    ma.write_local_input(&preset_id, "number_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "number_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "number_out", &AgentValue::number(3.14))
        .await
        .unwrap();

    // String Input
    ma.write_local_input(&preset_id, "string_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "string_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "string_out",
        &AgentValue::string("".to_string()),
    )
    .await
    .unwrap();

    ma.write_local_input(
        &preset_id,
        "string_conf",
        AgentValue::string("Hello, world!".to_string()),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "string_conf",
        &AgentValue::string("Hello, world!".to_string()),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "string_out",
        &AgentValue::string("Hello, world!".to_string()),
    )
    .await
    .unwrap();
    ma.write_local_input(&preset_id, "string_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "string_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "string_out",
        &AgentValue::string("Hello, world!".to_string()),
    )
    .await
    .unwrap();

    // Text Input
    ma.write_local_input(&preset_id, "text_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "text_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "text_out", &AgentValue::string(""))
        .await
        .unwrap();

    ma.write_local_input(
        &preset_id,
        "text_conf",
        AgentValue::string("Old pond\nFrogs jumped in\nSound of water.\n"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "text_conf",
        &AgentValue::string("Old pond\nFrogs jumped in\nSound of water.\n"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "text_out",
        &AgentValue::string("Old pond\nFrogs jumped in\nSound of water.\n"),
    )
    .await
    .unwrap();
    ma.write_local_input(&preset_id, "text_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "text_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "text_out",
        &AgentValue::string("Old pond\nFrogs jumped in\nSound of water.\n"),
    )
    .await
    .unwrap();

    // Object Input
    ma.write_local_input(&preset_id, "object_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "object_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "object_out", &AgentValue::object_default())
        .await
        .unwrap();

    ma.write_local_input(
        &preset_id,
        "object_conf",
        AgentValue::object(hashmap! {
            "name".to_string() => AgentValue::string("Alice".to_string()),
            "is_student".to_string() => AgentValue::boolean(false),
        }),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "object_conf",
        &AgentValue::object(hashmap! {
            "name".to_string() => AgentValue::string("Alice".to_string()),
            "is_student".to_string() => AgentValue::boolean(false),
        }),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "object_out",
        &AgentValue::object(hashmap! {
            "name".to_string() => AgentValue::string("Alice".to_string()),
            "is_student".to_string() => AgentValue::boolean(false),
        }),
    )
    .await
    .unwrap();
    ma.write_local_input(&preset_id, "object_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "object_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "object_out",
        &AgentValue::object(hashmap! {
            "name".to_string() => AgentValue::string("Alice".to_string()),
            "is_student".to_string() => AgentValue::boolean(false),
        }),
    )
    .await
    .unwrap();

    ma.quit();
}
