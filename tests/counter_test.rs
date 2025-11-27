extern crate agent_stream_kit as askit;
use askit::ASKit;

mod common;
const COUNTER_DEF: &str = concat!(module_path!(), "::common::CounterAgent");

#[test]
fn test_register_agents() {
    let askit = ASKit::init().unwrap();

    // Check the properties of the counter agent
    let counter_def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    assert_eq!(counter_def.title, Some("Counter".into()));
    assert_eq!(counter_def.inputs, Some(vec!["in".into(), "reset".into()]));
    assert_eq!(counter_def.outputs, Some(vec!["count".into()]));
}

#[test]
fn test_agent_new() {
    let askit = ASKit::init().unwrap();

    let agent = askit::agent_new(askit.clone(), "agent_1".into(), COUNTER_DEF, None).unwrap();
    assert_eq!(agent.def_name(), COUNTER_DEF);
    assert_eq!(agent.id(), "agent_1");
    assert_eq!(agent.status(), &askit::AgentStatus::Init);
}

#[tokio::test]
async fn test_agent_start() {
    let askit = ASKit::init().unwrap();

    let mut agent =
        askit::agent_new(askit.clone(), "agent_1".into(), COUNTER_DEF, None).unwrap();
    agent.start().await.unwrap();

    assert_eq!(agent.status(), &askit::AgentStatus::Start);
}

#[tokio::test]
async fn test_agent_process() {
    let askit = ASKit::init().unwrap();

    askit.ready().await.unwrap();

    let mut agent =
        askit::agent_new(askit.clone(), "agent_1".into(), COUNTER_DEF, None).unwrap();
    agent.start().await.unwrap();

    assert!(agent.out_pin("count").is_none());

    let ctx = askit::AgentContext::new();
    agent
        .process(ctx, "in".into(), askit::AgentValue::unit())
        .await
        .unwrap();

    assert_eq!(
        agent.out_pin("count").unwrap().value,
        askit::AgentValue::integer(1)
    );

    let ctx = askit::AgentContext::new();
    agent
        .process(ctx, "in".into(), askit::AgentValue::unit())
        .await
        .unwrap();

    assert_eq!(
        agent.out_pin("count").unwrap().value,
        askit::AgentValue::integer(2)
    );
}

#[tokio::test]
async fn test_agent_stop() {
    let askit = ASKit::init().unwrap();

    askit.ready().await.unwrap();

    let mut agent =
        askit::agent_new(askit.clone(), "agent_1".into(), COUNTER_DEF, None).unwrap();
    agent.start().await.unwrap();

    let ctx = askit::AgentContext::new();
    agent
        .process(ctx, "in".into(), askit::AgentValue::unit())
        .await
        .unwrap();

    agent.stop().await.unwrap();
    assert_eq!(agent.status(), &askit::AgentStatus::Init);
}
