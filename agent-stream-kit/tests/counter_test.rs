extern crate agent_stream_kit as askit;

use askit::{
    ASKit, AgentContext, AgentStatus, AgentStream, AgentStreamNode, AgentValue, ChannelSpec,
    test_utils::{TestProbeAgent, probe_receiver},
};

mod common;

const COUNTER_DEF: &str = common::CounterAgent::DEF_NAME;
const PROBE_DEF: &str = TestProbeAgent::DEF_NAME;

#[test]
fn test_register_agents() {
    let askit = ASKit::init().unwrap();

    // Check the properties of the counter agent
    let counter_def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    assert_eq!(counter_def.title, Some("Counter".into()));
    assert_eq!(counter_def.inputs, Some(vec!["in".into(), "reset".into()]));
    assert_eq!(counter_def.outputs, Some(vec!["count".into()]));

    askit.quit();
}

#[test]
fn test_agent_new() {
    let askit = ASKit::init().unwrap();
    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();

    let agent = askit::agent_new(askit.clone(), "agent_1".into(), def.to_spec()).unwrap();
    assert_eq!(agent.def_name(), COUNTER_DEF);
    assert_eq!(agent.id(), "agent_1");
    assert_eq!(agent.status(), &AgentStatus::Init);

    askit.quit();
}

#[tokio::test]
async fn test_agent_start() {
    let askit = ASKit::init().unwrap();
    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();

    let mut agent = askit::agent_new(askit.clone(), "agent_1".into(), def.to_spec()).unwrap();
    agent.start().await.unwrap();

    assert_eq!(agent.status(), &AgentStatus::Start);

    askit.quit();
}

#[tokio::test]
async fn test_agent_stop() {
    let askit = ASKit::init().unwrap();

    askit.ready().await.unwrap();

    let def = askit.get_agent_definition(COUNTER_DEF).unwrap();

    let mut agent = askit::agent_new(askit.clone(), "agent_1".into(), def.to_spec()).unwrap();
    agent.start().await.unwrap();

    let ctx = AgentContext::new();
    agent
        .process(ctx, "in".into(), AgentValue::unit())
        .await
        .unwrap();

    agent.stop().await.unwrap();
    assert_eq!(agent.status(), &AgentStatus::Init);

    askit.quit();
}

#[tokio::test]
async fn test_agent_process() {
    let askit = ASKit::init().unwrap();

    // build a flow: Counter -> TestProbe
    let counter_def = askit.get_agent_definition(COUNTER_DEF).unwrap();
    let mut counter_node = AgentStreamNode::new(&counter_def).unwrap();
    counter_node.enabled = true;

    let probe_def = askit.get_agent_definition(PROBE_DEF).unwrap();
    let mut probe_node = AgentStreamNode::new(&probe_def).unwrap();
    probe_node.enabled = true;

    let counter_id = counter_node.id.clone();
    let probe_id = probe_node.id.clone();

    let mut flow = AgentStream::new("counter_probe_flow".into());
    flow.add_agent(counter_node);
    flow.add_agent(probe_node);
    flow.add_channels(ChannelSpec {
        id: "edge_counter_probe".into(),
        source: counter_id.clone(),
        source_handle: "count".into(),
        target: probe_id.clone(),
        target_handle: "in".into(),
    });

    askit.add_agent_stream(&flow).unwrap();
    askit.ready().await.unwrap();

    askit
        .agent_input(
            counter_id.clone(),
            AgentContext::new(),
            "in".into(),
            AgentValue::unit(),
        )
        .await
        .unwrap();

    let probe_rec = probe_receiver(&askit, &probe_id).await.unwrap();

    let (_ctx, value) = probe_rec.recv().await.unwrap();
    assert_eq!(value, AgentValue::integer(1));

    askit
        .agent_input(
            counter_id.clone(),
            AgentContext::new(),
            "in".into(),
            AgentValue::unit(),
        )
        .await
        .unwrap();

    let (_ctx, value) = probe_rec.recv().await.unwrap();
    assert_eq!(value, AgentValue::integer(2));

    askit.quit();
}
