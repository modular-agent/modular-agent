extern crate modular_agent_core as ma;

use im::vector;
use ma::{AgentValue, test_utils};

#[tokio::test]
async fn test_is_string() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // Unit -> f
    test_utils::write_and_expect_local_value(&ma, &patch_id, "is_string_in", AgentValue::unit())
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "is_string_f", &AgentValue::unit())
        .await
        .unwrap();

    // String -> t
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "is_string_in",
        AgentValue::string("hello"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "is_string_t", &AgentValue::string("hello"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_is_empty_string() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // Empty -> t
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "is_empty_string_in",
        AgentValue::string(""),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "is_empty_string_t", &AgentValue::string(""))
        .await
        .unwrap();

    // Non-empty -> f
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "is_empty_string_in",
        AgentValue::string("hello"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "is_empty_string_f", &AgentValue::string("hello"))
        .await
        .unwrap();

    // Non-string (Unit) -> f
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "is_empty_string_in",
        AgentValue::unit(),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "is_empty_string_f", &AgentValue::unit())
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_string_join() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // Array join with default sep \\n -> \n
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "string_join_in",
        AgentValue::array(vector![
            AgentValue::string("Hello"),
            AgentValue::string("World"),
        ]),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "string_join_out",
        &AgentValue::string("Hello\nWorld"),
    )
    .await
    .unwrap();

    // Non-array passthrough
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "string_join_in",
        AgentValue::string("solo"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "string_join_out", &AgentValue::string("solo"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_string_length_split() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // Short string -> single element array
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "string_length_split_in",
        AgentValue::string("Hello, World!"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "string_length_split_out",
        &AgentValue::array(vector![AgentValue::string("Hello, World!")]),
    )
    .await
    .unwrap();

    // Long string -> split into multiple elements
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "string_length_split_len",
        AgentValue::integer(8),
    )
    .await
    .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "string_length_split_overlap",
        AgentValue::integer(2),
    )
    .await
    .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "string_length_split_in",
        AgentValue::string("Hello, World!"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &patch_id,
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
async fn test_regex_match() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // First match -> string
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "regex_match_in",
        AgentValue::string("abc123def456"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "regex_match_out", &AgentValue::string("123"))
        .await
        .unwrap();

    // No match -> unit on unmatched
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "regex_match_in",
        AgentValue::string("abcdef"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "regex_match_unmatched", &AgentValue::unit())
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_regex_match_all() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // All matches -> strings
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "regex_match_all_in",
        AgentValue::string("abc123def456"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "regex_match_all_out",
        &AgentValue::array(vector![
            AgentValue::string("123"),
            AgentValue::string("456")
        ]),
    )
    .await
    .unwrap();

    // No match -> unit on unmatched
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "regex_match_all_in",
        AgentValue::string("abcdef"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "regex_match_all_unmatched", &AgentValue::unit())
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_regex_replace() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // Only the first match is replaced
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "regex_replace_in",
        AgentValue::string("abc123def456"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "regex_replace_out",
        &AgentValue::string("abc#def456"),
    )
    .await
    .unwrap();

    // No match -> unchanged
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "regex_replace_in",
        AgentValue::string("abcdef"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "regex_replace_out",
        &AgentValue::string("abcdef"),
    )
    .await
    .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_regex_replace_all() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // Every match is replaced
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "regex_replace_all_in",
        AgentValue::string("abc123def456"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "regex_replace_all_out",
        &AgentValue::string("abc#def#"),
    )
    .await
    .unwrap();

    // No match -> unchanged
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "regex_replace_all_in",
        AgentValue::string("abcdef"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "regex_replace_all_out",
        &AgentValue::string("abcdef"),
    )
    .await
    .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_template_string() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // String with default {{value}} -> same string
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "template_string_in",
        AgentValue::string("hello"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &patch_id,
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

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // String with default {{value}} -> same string
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "template_text_in",
        AgentValue::string("world"),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "template_text_out", &AgentValue::string("world"))
        .await
        .unwrap();

    ma.quit();
}

#[tokio::test]
async fn test_template_array() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, "tests/patches/Std_String_test.json")
        .await
        .unwrap();

    // Override template, then send array
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "template_array_template",
        AgentValue::string("{{#each this}}{{this}}{{#unless @last}},{{/unless}}{{/each}}"),
    )
    .await
    .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
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
        &patch_id,
        "template_array_out",
        &AgentValue::string("x,y,z"),
    )
    .await
    .unwrap();

    ma.quit();
}
