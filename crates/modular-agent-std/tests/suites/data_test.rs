extern crate modular_agent_core as ma;

use im::{hashmap, vector};
use ma::llm::Message;
use ma::{AgentValue, test_utils};

const PATCH: &str = "tests/patches/Std_Data_test.json";

/// Mattermost Listener shape: `{ message: Message, user, channel }`.
fn listener_value(text: &str) -> AgentValue {
    AgentValue::object(hashmap! {
        "message".to_string() => AgentValue::message(Message::user(text.to_string())),
        "user".to_string() => AgentValue::string("alice"),
        "channel".to_string() => AgentValue::string("town-square"),
    })
}

#[tokio::test]
async fn test_get_value_message_content() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // key `message.content` resolves through the Message value
    test_utils::write_and_expect_local_value(&ma, &patch_id, "get_in", listener_value("hello"))
        .await
        .unwrap();
    test_utils::expect_local_value(&patch_id, "get_out", &AgentValue::string("hello"))
        .await
        .unwrap();

    // A bare Message input works too
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "get_key",
        AgentValue::string("content"),
    )
    .await
    .unwrap();
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "get_in",
        AgentValue::message(Message::user("direct".to_string())),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(&patch_id, "get_out", &AgentValue::string("direct"))
        .await
        .unwrap();

    // An array applies the key to each element (e.g. an LLM history)
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "get_in",
        AgentValue::array(vector![
            AgentValue::message(Message::user("one".to_string())),
            AgentValue::message(Message::assistant("two".to_string())),
        ]),
    )
    .await
    .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "get_out",
        &AgentValue::array(vector![
            AgentValue::string("one"),
            AgentValue::string("two"),
        ]),
    )
    .await
    .unwrap();

    ma.quit();
}
