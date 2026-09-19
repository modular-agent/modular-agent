extern crate modular_agent_core as ma;

use ma::{AsModule, ModularAgent, Module, ModuleContext, ModuleStatus, Value};

use crate::common;
use common::modules::CounterModule;

const COUNTER_DEF: &str = CounterModule::DEF_NAME;

#[test]
fn test_register_module_definition() {
    let ma = ModularAgent::init().unwrap();

    // Check the properties of the counter module
    let counter_def = ma.get_module_definition(COUNTER_DEF).unwrap();
    assert_eq!(counter_def.title, Some("Counter".into()));
    assert_eq!(counter_def.inputs, Some(vec!["in".into(), "reset".into()]));
    assert_eq!(counter_def.outputs, Some(vec!["count".into()]));

    ma.quit();
}

#[test]
fn test_module_new() {
    let ma = ModularAgent::init().unwrap();
    let def = ma.get_module_definition(COUNTER_DEF).unwrap();
    let spec = def.to_spec();
    let module = <CounterModule as AsModule>::new(ma.clone(), "module_1".into(), spec).unwrap();
    assert_eq!(Module::def_name(&module), COUNTER_DEF);
    assert_eq!(Module::id(&module), "module_1");
    assert_eq!(Module::status(&module), &ModuleStatus::Init);

    ma.quit();
}

#[tokio::test]
async fn test_module_start() {
    let ma = ModularAgent::init().unwrap();
    let def = ma.get_module_definition(COUNTER_DEF).unwrap();
    let spec = def.to_spec();
    let mut module = <CounterModule as AsModule>::new(ma.clone(), "module_1".into(), spec).unwrap();
    Module::start(&mut module).await.unwrap();

    assert_eq!(Module::status(&module), &ModuleStatus::Start);

    ma.quit();
}

#[tokio::test]
async fn test_module_process() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let counter_def = ma.get_module_definition(COUNTER_DEF).unwrap();
    let counter_spec = counter_def.to_spec();

    let mut counter_module =
        <CounterModule as AsModule>::new(ma.clone(), "module_1".into(), counter_spec).unwrap();
    Module::start(&mut counter_module).await.unwrap();

    let ctx = ModuleContext::new();
    Module::process(&mut counter_module, ctx, "in".into(), Value::unit())
        .await
        .unwrap();

    assert_eq!(counter_module.count, 1);

    ma.quit();
}

#[tokio::test]
async fn test_module_stop() {
    let ma = ModularAgent::init().unwrap();

    ma.ready().await.unwrap();

    let def = ma.get_module_definition(COUNTER_DEF).unwrap();
    let spec = def.to_spec();
    let mut module = <CounterModule as AsModule>::new(ma.clone(), "module_1".into(), spec).unwrap();
    Module::start(&mut module).await.unwrap();

    let ctx = ModuleContext::new();
    Module::process(&mut module, ctx, "in".into(), Value::unit())
        .await
        .unwrap();

    Module::stop(&mut module).await.unwrap();
    assert_eq!(Module::status(&module), &ModuleStatus::Init);

    ma.quit();
}
