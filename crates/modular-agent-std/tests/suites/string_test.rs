extern crate modular_agent_core as ma;

use im::vector;
use ma::{AgentValue, test_utils};

#[tokio::test]
async fn test_is_string() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_String_test.json")
        .await
        .unwrap();

    // Unit -> f
    test_utils::write_and_expect_local_value(&ma, &preset_id, "is_string_in", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&preset_id, "is_string_f", &AgentValue::unit())
        .await
        .unwrap();

    // String -> t
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "is_string_in",
        AgentValue::string("hello"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&preset_id, "is_string_t", &AgentValue::string("hello"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_is_empty_string() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_String_test.json")
        .await
        .unwrap();

    // Empty -> t
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "is_empty_string_in",
        AgentValue::string(""),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&preset_id, "is_empty_string_t", &AgentValue::string(""))
        .await
        .unwrap();

    // Non-empty -> f
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "is_empty_string_in",
        AgentValue::string("hello"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "is_empty_string_f",
        &AgentValue::string("hello"),
    )
    .await
    .unwrap();

    // Non-string (Unit) -> f
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "is_empty_string_in",
        AgentValue::unit(),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&preset_id, "is_empty_string_f", &AgentValue::unit())
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_string_join() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_String_test.json")
        .await
        .unwrap();

    // Array join with default sep \\n -> \n
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "string_join_in",
        AgentValue::array(vector![
            AgentValue::string("Hello"),
            AgentValue::string("World"),
        ]),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "string_join_out",
        &AgentValue::string("Hello\nWorld"),
    )
    .await
    .unwrap();

    // Non-array passthrough
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "string_join_in",
        AgentValue::string("solo"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&preset_id, "string_join_out", &AgentValue::string("solo"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_string_length_split() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_String_test.json")
        .await
        .unwrap();

    // Short string -> single element array
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "string_length_split_in",
        AgentValue::string("Hello, World!"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "string_length_split_out",
        &AgentValue::array(vector![AgentValue::string("Hello, World!")]),
    )
    .await
    .unwrap();

    // Long string -> split into multiple elements
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "string_length_split_len",
        AgentValue::integer(8),
    )
    .await
    .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "string_length_split_overlap",
        AgentValue::integer(2),
    )
    .await
    .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "string_length_split_in",
        AgentValue::string("Hello, World!"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "string_length_split_out",
        &AgentValue::array(vector![
            AgentValue::string("Hello, W"),
            AgentValue::string(" World!")
        ]),
    )
    .await
    .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_template_string() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_String_test.json")
        .await
        .unwrap();

    // String with default {{value}} -> same string
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "template_string_in",
        AgentValue::string("hello"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "template_string_out",
        &AgentValue::string("hello"),
    )
    .await
    .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_template_text() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_String_test.json")
        .await
        .unwrap();

    // String with default {{value}} -> same string
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "template_text_in",
        AgentValue::string("world"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "template_text_out",
        &AgentValue::string("world"),
    )
    .await
    .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_template_array() {
    let ma = test_utils::setup_modular_agent().await;

    let preset_id = test_utils::open_and_start_preset(&ma, "tests/presets/Std_String_test.json")
        .await
        .unwrap();

    // Override template, then send array
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "template_array_template",
        AgentValue::string("{{#each this}}{{this}}{{#unless @last}},{{/unless}}{{/each}}"),
    )
    .await
    .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &preset_id,
        "template_array_in",
        AgentValue::array(vector![
            AgentValue::string("x"),
            AgentValue::string("y"),
            AgentValue::string("z"),
        ]),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &preset_id,
        "template_array_out",
        &AgentValue::string("x,y,z"),
    )
    .await
    .unwrap();

    ma.quit();
}
