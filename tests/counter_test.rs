extern crate agent_stream_kit as askit;
use askit::ASKit;

mod common;

#[test]
fn test_register_agents() {
    let askit = ASKit::init().unwrap();
    common::register_agents(&askit);

    // Check the properties of the counter agent
    let counter_def = askit.get_agent_definition("test_counter").unwrap();
    assert_eq!(counter_def.title, Some("Counter".into()));
    assert_eq!(counter_def.inputs, Some(vec!["in".into(), "reset".into()]));
    assert_eq!(counter_def.outputs, Some(vec!["count".into()]));
}

#[test]
fn test_agent_new() {
    let askit = ASKit::init().unwrap();
    common::register_agents(&askit);

    let agent = askit::agent_new(askit.clone(), "agent_1".into(), "test_counter", None).unwrap();
    assert_eq!(agent.def_name(), "test_counter");
    assert_eq!(agent.id(), "agent_1");
    assert_eq!(agent.status(), &askit::AgentStatus::Init);
}

#[tokio::test]
async fn test_agent_start() {
    let askit = ASKit::init().unwrap();
    common::register_agents(&askit);

    let mut agent =
        askit::agent_new(askit.clone(), "agent_1".into(), "test_counter", None).unwrap();
    agent.start().await.unwrap();

    assert_eq!(agent.status(), &askit::AgentStatus::Start);
}

#[tokio::test]
async fn test_agent_process() {
    let askit = ASKit::init().unwrap();
    common::register_agents(&askit);

    askit.ready().await.unwrap();

    let mut agent =
        askit::agent_new(askit.clone(), "agent_1".into(), "test_counter", None).unwrap();
    agent.start().await.unwrap();

    assert!(agent.out_pin("count").is_none());

    let ctx = askit::AgentContext::new();
    agent
        .process(ctx, "in".into(), askit::AgentData::unit())
        .await
        .unwrap();

    assert_eq!(
        agent.out_pin("count").unwrap().data,
        askit::AgentData::integer(1)
    );

    let ctx = askit::AgentContext::new();
    agent
        .process(ctx, "in".into(), askit::AgentData::unit())
        .await
        .unwrap();

    assert_eq!(
        agent.out_pin("count").unwrap().data,
        askit::AgentData::integer(2)
    );
}

#[tokio::test]
async fn test_agent_stop() {
    let askit = ASKit::init().unwrap();
    common::register_agents(&askit);

    askit.ready().await.unwrap();

    let mut agent =
        askit::agent_new(askit.clone(), "agent_1".into(), "test_counter", None).unwrap();
    agent.start().await.unwrap();

    let ctx = askit::AgentContext::new();
    agent
        .process(ctx, "in".into(), askit::AgentData::unit())
        .await
        .unwrap();

    agent.stop().await.unwrap();
    assert_eq!(agent.status(), &askit::AgentStatus::Init);
}
