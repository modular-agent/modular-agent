extern crate modular_agent_core as ma;

use ma::{AgentError, ModularAgent};

#[tokio::test]
async fn test_duplicate_name_create_fails() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let id = ma.new_preset_with_name("dup".into()).unwrap();

    let err = ma.new_preset_with_name("dup".into()).unwrap_err();
    assert!(matches!(err, AgentError::PresetNameExists(ref name) if name == "dup"));

    // The failed create must not disturb the existing mapping.
    assert_eq!(ma.find_preset_id_by_name("dup"), Some(id));

    ma.quit();
}

#[tokio::test]
async fn test_rename_onto_used_name_fails() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let id_a = ma.new_preset_with_name("a".into()).unwrap();
    let id_b = ma.new_preset_with_name("b".into()).unwrap();

    let err = ma.rename_preset(&id_b, "a".into()).await.unwrap_err();
    assert!(matches!(err, AgentError::PresetNameExists(ref name) if name == "a"));

    // Both mappings survive the failed rename.
    assert_eq!(ma.find_preset_id_by_name("a"), Some(id_a));
    assert_eq!(ma.find_preset_id_by_name("b"), Some(id_b));

    ma.quit();
}

#[tokio::test]
async fn test_remove_preset_frees_name() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let id1 = ma.new_preset_with_name("reuse".into()).unwrap();
    ma.remove_preset(&id1).await.unwrap();
    assert_eq!(ma.find_preset_id_by_name("reuse"), None);

    let id2 = ma.new_preset_with_name("reuse".into()).unwrap();
    assert_ne!(id1, id2);
    assert_eq!(ma.find_preset_id_by_name("reuse"), Some(id2));

    ma.quit();
}

#[tokio::test]
async fn test_rename_to_own_name_succeeds() {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    let id = ma.new_preset_with_name("same".into()).unwrap();
    ma.rename_preset(&id, "same".into()).await.unwrap();
    assert_eq!(ma.find_preset_id_by_name("same"), Some(id));

    ma.quit();
}
