extern crate modular_agent_core as ma;

use ma::{Agent, AgentContext, AgentStatus, AgentValue, AsAgent, ModularAgent};

use crate::common;
use common::agents::CounterAgent;

const COUNTER_DEF: &str = CounterAgent::DEF_NAME;

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
    let agent = <CounterAgent as AsAgent>::new(ma.clone(), "agent_1".into(), spec).unwrap();
    assert_eq!(Agent::def_name(&agent), COUNTER_DEF);
    assert_eq!(Agent::id(&agent), "agent_1");
    assert_eq!(Agent::status(&agent), &AgentStatus::Init);

    ma.quit();
}

#[tokio::test]
async fn test_agent_start() {
    let ma = ModularAgent::init().unwrap();
    let def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    let spec = def.to_spec();
    let mut agent =
        <CounterAgent as AsAgent>::new(ma.clone(), "agent_1".into(), spec).unwrap();
    Agent::start(&mut agent).await.unwrap();

    assert_eq!(Agent::status(&agent), &AgentStatus::Start);

    ma.quit();
}

#[tokio::test]
async fn test_agent_process() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let counter_def = ma.get_agent_definition(COUNTER_DEF).unwrap();
    let counter_spec = counter_def.to_spec();

    let mut counter_agent =
        <CounterAgent as AsAgent>::new(ma.clone(), "agent_1".into(), counter_spec).unwrap();
    Agent::start(&mut counter_agent).await.unwrap();

    let ctx = AgentContext::new();
    Agent::process(&mut counter_agent, ctx, "in".into(), AgentValue::unit())
        .await
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
    let mut agent = <CounterAgent as AsAgent>::new(ma.clone(), "agent_1".into(), spec).unwrap();
    Agent::start(&mut agent).await.unwrap();

    let ctx = AgentContext::new();
    Agent::process(&mut agent, ctx, "in".into(), AgentValue::unit())
        .await
        .unwrap();

    Agent::stop(&mut agent).await.unwrap();
    assert_eq!(Agent::status(&agent), &AgentStatus::Init);

    ma.quit();
}
