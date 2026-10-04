extern crate modular_agent_core as ma;

use std::sync::Arc;
use std::time::Duration;

use ma::{Error, ModularAgent, ModularAgentEvent, ModuleLogLevel, Value, test_utils};
use tokio::sync::mpsc;

const PATCH: &str = "tests/patches/Std_Log_test.json";

enum Record {
    Log(ModuleLogLevel, String),
    Error,
}

fn subscribe_records(ma: &ModularAgent) -> mpsc::UnboundedReceiver<Record> {
    ma.subscribe_to_event(|envelope| match envelope.event {
        ModularAgentEvent::ModuleLog { level, message, .. } => Some(Record::Log(level, message)),
        ModularAgentEvent::ModuleError(..) => Some(Record::Error),
        _ => None,
    })
}

async fn next_log(rx: &mut mpsc::UnboundedReceiver<Record>) -> (ModuleLogLevel, String) {
    match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
        Ok(Some(Record::Log(level, message))) => (level, message),
        Ok(Some(Record::Error)) => panic!("unexpected module error"),
        _ => panic!("no log record"),
    }
}

#[tokio::test]
async fn test_log_default() {
    let ma = test_utils::setup_modular_agent().await;
    let mut rx = subscribe_records(&ma);
    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    ma.write_local_input(&patch_id, "log_default_in", Value::string("hello"))
        .await
        .unwrap();
    assert_eq!(
        next_log(&mut rx).await,
        (ModuleLogLevel::Info, "hello".to_string())
    );

    // An error value logs its message rather than the null it serializes to.
    let err = Value::Error(Arc::new(Error::InvalidValue("boom".into())));
    ma.write_local_input(&patch_id, "log_default_in", err)
        .await
        .unwrap();
    let (_, message) = next_log(&mut rx).await;
    assert!(message.contains("boom"), "{message}");

    ma.quit();
}

#[tokio::test]
async fn test_log_level_and_template() {
    let ma = test_utils::setup_modular_agent().await;
    let mut rx = subscribe_records(&ma);
    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    let value =
        Value::from_json(serde_json::json!({ "thread_ts": "123.456", "text": "hi" })).unwrap();
    ma.write_local_input(&patch_id, "log_warn_in", value)
        .await
        .unwrap();
    assert_eq!(
        next_log(&mut rx).await,
        (ModuleLogLevel::Warn, "replied to 123.456".to_string())
    );

    ma.quit();
}

#[tokio::test]
async fn test_log_rejects_unknown_level() {
    let ma = test_utils::setup_modular_agent().await;
    let mut rx = subscribe_records(&ma);
    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    ma.write_local_input(&patch_id, "log_bad_level_in", Value::string("hello"))
        .await
        .unwrap();
    match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
        Ok(Some(Record::Error)) => {}
        _ => panic!("expected a module error"),
    }

    ma.quit();
}
