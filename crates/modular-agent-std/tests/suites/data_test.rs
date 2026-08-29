extern crate modular_agent_core as ma;

use im::{hashmap, vector};
use ma::llm::Message;
use ma::test_utils::{probe_receiver, recv_probe};
use ma::{AgentConfigs, AgentContext, AgentValue, ConnectionSpec, test_utils};

const PATCH: &str = "tests/patches/Std_Data_test.json";

const ZIP_OBJ_DEF: &str = "modular_agent_std::data::ZipToObjectAgent";
const SET_VALUE_DEF: &str = "modular_agent_std::data::SetValueAgent";
const LOCAL_IN_DEF: &str = "modular_agent_core::external_agent::LocalInputAgent";
const LOCAL_OUT_DEF: &str = "modular_agent_core::external_agent::LocalOutputAgent";
const PROBE_DEF: &str = "modular_agent_core::test_utils::TestProbeAgent";

/// Add one agent (configs adjusted by `configure`) wired to a probe, start the
/// patch, and return the agent id with the probe receiver.
async fn setup_agent_with_probe(
    ma: &ma::ModularAgent,
    def_name: &str,
    out_port: &str,
    configure: impl FnOnce(&mut AgentConfigs),
) -> (String, ma::test_utils::ProbeReceiver) {
    let patch_id = ma.new_patch().unwrap();
    let mut spec = ma.get_agent_definition(def_name).unwrap().to_spec();
    if let Some(configs) = spec.configs.as_mut() {
        configure(configs);
    }
    let agent_id = ma.add_agent(patch_id.clone(), spec).await.unwrap();
    let probe_id = ma
        .add_agent(
            patch_id.clone(),
            ma.get_agent_definition(PROBE_DEF).unwrap().to_spec(),
        )
        .await
        .unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: agent_id.clone(),
            source_handle: out_port.into(),
            target: probe_id.clone(),
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();
    ma.start_patch(&patch_id).await.unwrap();
    let probe = probe_receiver(ma, &probe_id).await.unwrap();
    (agent_id, probe)
}

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

#[tokio::test]
async fn test_set_value_writes_dot_path() {
    let ma = test_utils::setup_modular_agent().await;
    let (set_id, probe) = setup_agent_with_probe(&ma, SET_VALUE_DEF, "value", |cfg| {
        cfg.set("key".into(), AgentValue::string("user.name"));
        cfg.set("value".into(), AgentValue::string("Alice"));
    })
    .await;

    let agent = ma.get_agent(&set_id).unwrap();
    agent
        .lock()
        .await
        .process(
            AgentContext::new(),
            "value".into(),
            AgentValue::object(hashmap! {
                "user".to_string() => AgentValue::object(hashmap! {}),
            }),
        )
        .await
        .unwrap();

    let (_ctx, value) = recv_probe(&probe).await.unwrap();
    assert_eq!(
        value,
        AgentValue::object(hashmap! {
            "user".to_string() => AgentValue::object(hashmap! {
                "name".to_string() => AgentValue::string("Alice"),
            }),
        })
    );

    ma.quit();
}

#[tokio::test]
async fn test_zip_to_object_fifo() {
    let ma = test_utils::setup_modular_agent().await;
    // Default keys are the port indices "0" / "1"
    let (zip_id, probe) = setup_agent_with_probe(&ma, ZIP_OBJ_DEF, "object", |_| {}).await;

    let ctx = AgentContext::new();
    let agent = ma.get_agent(&zip_id).unwrap();
    {
        let mut guard = agent.lock().await;
        for (port, v) in [("0", 1), ("1", 2)] {
            guard
                .process(ctx.clone(), port.into(), AgentValue::integer(v))
                .await
                .unwrap();
        }
    }

    let (_ctx, value) = recv_probe(&probe).await.unwrap();
    assert_eq!(
        value,
        AgentValue::object(hashmap! {
            "0".to_string() => AgentValue::integer(1),
            "1".to_string() => AgentValue::integer(2),
        })
    );

    ma.quit();
}

#[tokio::test]
async fn test_zip_to_object_restores_parked_keys() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = ma.new_patch().unwrap();
    let def = ma.get_agent_definition(ZIP_OBJ_DEF).unwrap();

    // reconcile_spec parks the undeclared k0/k1 configs under a "_" prefix when a
    // patch is loaded; new() must pick the values back up.
    let mut spec = def.to_spec();
    let mut configs = spec.configs.take().unwrap();
    configs.set("_k0".into(), AgentValue::string("alpha"));
    configs.set("_k1".into(), AgentValue::string("beta"));
    spec.configs = Some(configs);
    let agent_id = ma.add_agent(patch_id.clone(), spec).await.unwrap();

    let created = ma.get_agent_spec(&agent_id).await.unwrap();
    let configs = created.configs.expect("configs must be present");
    assert_eq!(configs.get_string("k0").unwrap(), "alpha");
    assert_eq!(configs.get_string("k1").unwrap(), "beta");
    assert!(!configs.contains_key("_k0"), "the parked key must be gone");

    ma.quit();
}

#[tokio::test]
async fn test_zip_to_object_save_reload_roundtrip() {
    let ma = test_utils::setup_modular_agent().await;

    // LocalIn "zip_in0"/"zip_in1" -> ZipToObject(k0=alpha, k1=beta) -> LocalOut "zip_out"
    let patch_id = ma.new_patch().unwrap();
    let zip_id = ma
        .add_agent(
            patch_id.clone(),
            ma.get_agent_definition(ZIP_OBJ_DEF).unwrap().to_spec(),
        )
        .await
        .unwrap();
    let mut key_configs = AgentConfigs::new();
    key_configs.set("k0".into(), AgentValue::string("alpha"));
    key_configs.set("k1".into(), AgentValue::string("beta"));
    ma.set_agent_configs(zip_id.clone(), key_configs)
        .await
        .unwrap();

    for (i, name) in ["zip_in0", "zip_in1"].iter().enumerate() {
        let mut spec = ma.get_agent_definition(LOCAL_IN_DEF).unwrap().to_spec();
        if let Some(configs) = spec.configs.as_mut() {
            configs.set("name".into(), AgentValue::string(*name));
        }
        let in_id = ma.add_agent(patch_id.clone(), spec).await.unwrap();
        ma.add_connection(
            &patch_id,
            ConnectionSpec {
                source: in_id,
                source_handle: "value".into(),
                target: zip_id.clone(),
                target_handle: i.to_string(),
            },
        )
        .await
        .unwrap();
    }
    let mut out_spec = ma.get_agent_definition(LOCAL_OUT_DEF).unwrap().to_spec();
    if let Some(configs) = out_spec.configs.as_mut() {
        configs.set("name".into(), AgentValue::string("zip_out"));
    }
    let out_id = ma.add_agent(patch_id.clone(), out_spec).await.unwrap();
    ma.add_connection(
        &patch_id,
        ConnectionSpec {
            source: zip_id.clone(),
            source_handle: "object".into(),
            target: out_id,
            target_handle: "value".into(),
        },
    )
    .await
    .unwrap();

    let path = std::env::temp_dir().join(format!("ma_zip_roundtrip_{}.json", std::process::id()));
    let path_str = path.to_string_lossy().to_string();
    ma.save_patch(&patch_id, &path_str).await.unwrap();
    ma.quit();

    // Reload in a fresh instance: the saved key names must survive and drive the zip.
    // Agent ids are reassigned on load, so look the agent up by def_name.
    let ma = test_utils::setup_modular_agent().await;
    let patch_id = test_utils::open_and_start_patch(&ma, &path_str)
        .await
        .unwrap();

    let patch_spec = ma.get_patch_spec(&patch_id).await.unwrap();
    let zip_spec = patch_spec
        .agents
        .iter()
        .find(|a| a.def_name == ZIP_OBJ_DEF)
        .expect("zip agent must be in the reloaded patch");
    let configs = zip_spec.configs.clone().expect("configs must be present");
    assert_eq!(configs.get_string("k0").unwrap(), "alpha");
    assert_eq!(configs.get_string("k1").unwrap(), "beta");

    test_utils::write_and_expect_local_value(&ma, &patch_id, "zip_in0", AgentValue::integer(1))
        .await
        .unwrap();
    test_utils::write_and_expect_local_value(&ma, &patch_id, "zip_in1", AgentValue::integer(2))
        .await
        .unwrap();
    test_utils::expect_local_value(
        &patch_id,
        "zip_out",
        &AgentValue::object(hashmap! {
            "alpha".to_string() => AgentValue::integer(1),
            "beta".to_string() => AgentValue::integer(2),
        }),
    )
    .await
    .unwrap();

    ma.quit();
    let _ = std::fs::remove_file(&path);
}
