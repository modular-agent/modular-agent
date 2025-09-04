extern crate agent_stream_kit as askit;

mod common;

use askit::ASKit;
use common::register_agents;

#[test]
fn test_register_agents() {
    let askit = ASKit::init().unwrap();
    register_agents(&askit);

    // Check the properties of the counter agent
    let counter_def = askit.get_agent_definition("$counter").unwrap();
    assert_eq!(counter_def.title, Some("Counter".into()));
    assert_eq!(counter_def.inputs, Some(vec!["in".into(), "reset".into()]));
    assert_eq!(counter_def.outputs, Some(vec!["count".into()]));
}
