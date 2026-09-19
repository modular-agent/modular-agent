extern crate modular_agent_core as ma;

use im::hashmap;
use ma::{Value, test_utils};

#[tokio::test]
async fn test_boolean_input() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "boolean_trig", Value::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "boolean_out", &Value::boolean(true))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_integer_input() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "integer_trig", Value::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "integer_out", &Value::integer(1))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_number_input() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "number_trig", Value::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "number_out", &Value::number(3.14))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_string_input() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "string_trig", Value::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "string_out",
        &Value::string("Hello, world!".to_string()),
    )
    .await
    .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_text_input() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "text_trig", Value::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "text_out",
        &Value::string("Old pond\nFrogs jumped in\nSound of water.\n"),
    )
    .await
    .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_object_input() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_Input_test.json")
        .await
        .unwrap();

    test_utils::write_and_expect_local_value(&ma, &patch_id, "object_trig", Value::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "object_out",
        &Value::object(hashmap! {
            "name".to_string() => Value::string("Alice".to_string()),
            "is_busy".to_string() => Value::boolean(false),
        }),
    )
    .await
    .unwrap();

    ma.quit();
}
