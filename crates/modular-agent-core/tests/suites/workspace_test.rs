extern crate modular_agent_core as ma;

use ma::workspace::Workspace;
use ma::{Error, ModularAgent};

#[tokio::test]
async fn test_auto_start_and_open_patch() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("patches").join("folder");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::copy("tests/patches/Core_External1.json", folder.join("p1.json")).unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{ "auto_start_patches": ["folder/p1", "missing"] }"#,
    )
    .unwrap();

    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();
    let ws = Workspace::open(dir.path()).unwrap();

    let failures = ws.auto_start(&ma).await;
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].0, "missing");
    assert!(matches!(failures[0].1, Error::IoError(_)));

    let id = ma.find_patch_id_by_name("folder/p1").unwrap();
    assert!(ma.get_patch_info(&id).await.unwrap().running);
    assert_eq!(ws.open_patch(&ma, "folder/p1").await.unwrap(), id);

    ma.quit();
}
