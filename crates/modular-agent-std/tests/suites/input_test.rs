extern crate agent_stream_kit as askit;

use askit::{AgentValue, test_utils};
use im::hashmap;

#[tokio::test]
async fn test_input() {
    let askit = test_utils::setup_askit().await;

    // load input stream
    let stream_id = test_utils::load_and_start_stream(&askit, "tests/streams/Std_Input_test.json")
        .await
        .unwrap();

    // Boolean Input
    askit
        .write_var_value(&stream_id, "boolean_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "boolean_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "boolean_out", &AgentValue::boolean(false))
        .await
        .unwrap();

    askit
        .write_var_value(&stream_id, "boolean_conf", AgentValue::boolean(true))
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "boolean_conf", &AgentValue::boolean(true))
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "boolean_out", &AgentValue::boolean(true))
        .await
        .unwrap();
    askit
        .write_var_value(&stream_id, "boolean_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "boolean_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "boolean_out", &AgentValue::boolean(true))
        .await
        .unwrap();

    // Integer Input
    askit
        .write_var_value(&stream_id, "integer_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "integer_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "integer_out", &AgentValue::integer(0))
        .await
        .unwrap();

    askit
        .write_var_value(&stream_id, "integer_conf", AgentValue::integer(42))
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "integer_conf", &AgentValue::integer(42))
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "integer_out", &AgentValue::integer(42))
        .await
        .unwrap();
    askit
        .write_var_value(&stream_id, "integer_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "integer_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "integer_out", &AgentValue::integer(42))
        .await
        .unwrap();

    // Number Input
    askit
        .write_var_value(&stream_id, "number_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "number_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "number_out", &AgentValue::number(0.0))
        .await
        .unwrap();

    askit
        .write_var_value(&stream_id, "number_conf", AgentValue::number(3.14))
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "number_conf", &AgentValue::number(3.14))
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "number_out", &AgentValue::number(3.14))
        .await
        .unwrap();
    askit
        .write_var_value(&stream_id, "number_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "number_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "number_out", &AgentValue::number(3.14))
        .await
        .unwrap();

    // String Input
    askit
        .write_var_value(&stream_id, "string_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "string_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(
        &stream_id,
        "string_out",
        &AgentValue::string("".to_string()),
    )
    .await
    .unwrap();

    askit
        .write_var_value(
            &stream_id,
            "string_conf",
            AgentValue::string("Hello, world!".to_string()),
        )
        .await
        .unwrap();
    test_utils::expect_var_value(
        &stream_id,
        "string_conf",
        &AgentValue::string("Hello, world!".to_string()),
    )
    .await
    .unwrap();
    test_utils::expect_var_value(
        &stream_id,
        "string_out",
        &AgentValue::string("Hello, world!".to_string()),
    )
    .await
    .unwrap();
    askit
        .write_var_value(&stream_id, "string_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "string_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(
        &stream_id,
        "string_out",
        &AgentValue::string("Hello, world!".to_string()),
    )
    .await
    .unwrap();

    // Text Input
    askit
        .write_var_value(&stream_id, "text_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "text_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "text_out", &AgentValue::string(""))
        .await
        .unwrap();

    askit
        .write_var_value(
            &stream_id,
            "text_conf",
            AgentValue::string("Old pond\nFrogs jumped in\nSound of water.\n"),
        )
        .await
        .unwrap();
    test_utils::expect_var_value(
        &stream_id,
        "text_conf",
        &AgentValue::string("Old pond\nFrogs jumped in\nSound of water.\n"),
    )
    .await
    .unwrap();
    test_utils::expect_var_value(
        &stream_id,
        "text_out",
        &AgentValue::string("Old pond\nFrogs jumped in\nSound of water.\n"),
    )
    .await
    .unwrap();
    askit
        .write_var_value(&stream_id, "text_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "text_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(
        &stream_id,
        "text_out",
        &AgentValue::string("Old pond\nFrogs jumped in\nSound of water.\n"),
    )
    .await
    .unwrap();

    // Object Input
    askit
        .write_var_value(&stream_id, "object_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "object_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "object_out", &AgentValue::object_default())
        .await
        .unwrap();

    askit
        .write_var_value(
            &stream_id,
            "object_conf",
            AgentValue::object(hashmap! {
                "name".to_string() => AgentValue::string("Alice".to_string()),
                "is_student".to_string() => AgentValue::boolean(false),
            }),
        )
        .await
        .unwrap();
    test_utils::expect_var_value(
        &stream_id,
        "object_conf",
        &AgentValue::object(hashmap! {
            "name".to_string() => AgentValue::string("Alice".to_string()),
            "is_student".to_string() => AgentValue::boolean(false),
        }),
    )
    .await
    .unwrap();
    test_utils::expect_var_value(
        &stream_id,
        "object_out",
        &AgentValue::object(hashmap! {
            "name".to_string() => AgentValue::string("Alice".to_string()),
            "is_student".to_string() => AgentValue::boolean(false),
        }),
    )
    .await
    .unwrap();
    askit
        .write_var_value(&stream_id, "object_trig", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(&stream_id, "object_trig", &AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_var_value(
        &stream_id,
        "object_out",
        &AgentValue::object(hashmap! {
            "name".to_string() => AgentValue::string("Alice".to_string()),
            "is_student".to_string() => AgentValue::boolean(false),
        }),
    )
    .await
    .unwrap();

    askit.quit();
}
