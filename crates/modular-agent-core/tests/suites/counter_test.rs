extern crate modular_agent_core as ma;

use ma::{ModularAgent, AgentContext, AgentStatus, AgentValue};

use crate::common;

const COUNTER_DEF: &str = common::agents::CounterAgent::DEF_NAME;

#[test]
fn test_register_agent_definiton() {
    let ma = ModularAgent::init().unwrap();

    // Check the properties of the counter agent
    let counter_def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    assert_eq!(counter_def.title, Some("Counter".into()));
    assert_eq!(counter_def.inputs, Some(vec!["in".into(), "reset".into()]));
    assert_eq!(counter_def.outputs, Some(vec!["count".into()]));

    ma.quit();
}

#[test]
fn test_agent_new() {
    let ma = ModularAgent::init().unwrap();
    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    let spec = def.to_spec();
    let agent = ma::agent_new(ma.clone(), "agent_1".into(), spec).unwrap();
    assert_eq!(agent.def_name(), COUNTER_DEF);
    assert_eq!(agent.id(), "agent_1");
    assert_eq!(agent.status(), &AgentStatus::Init);

    ma.quit();
}

#[tokio::test]
async fn test_agent_start() {
    let ma = ModularAgent::init().unwrap();
    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    let spec = def.to_spec();
    let mut agent = ma::agent_new(ma.clone(), "agent_1".into(), spec).unwrap();
    agent.start().await.unwrap();

    assert_eq!(agent.status(), &AgentStatus::Start);

    ma.quit();
}

#[tokio::test]
async fn test_agent_process() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let counter_def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    let counter_spec = counter_def.to_spec();

    let mut counter_agent =
        ma::agent_new(ma.clone(), "agent_1".into(), counter_spec).unwrap();
    counter_agent.start().await.unwrap();

    let ctx = AgentContext::new();
    counter_agent
        .process(ctx, "in".into(), AgentValue::unit())
        .await
        .unwrap();

    let counter_agent = counter_agent
        .as_any()
        .downcast_ref::<common::agents::CounterAgent>()
        .unwrap();
    assert_eq!(counter_agent.count, 1);

    ma.quit();
}

#[tokio::test]
async fn test_agent_stop() {
    let ma = ModularAgent::init().unwrap();

    ma.ready().await.unwrap();

    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    let spec = def.to_spec();
    let mut agent = ma::agent_new(ma.clone(), "agent_1".into(), spec).unwrap();
    agent.start().await.unwrap();

    let ctx = AgentContext::new();
    agent
        .process(ctx, "in".into(), AgentValue::unit())
        .await
        .unwrap();

    agent.stop().await.unwrap();
    assert_eq!(agent.status(), &AgentStatus::Init);

    ma.quit();
}
