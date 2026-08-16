extern crate modular_agent_core as ma;

use std::path::{Path, PathBuf};
use std::time::Duration;

use ma::{AgentValue, test_utils};

const PATCH: &str = "tests/patches/Std_Watch_test.json";

// Watcher startup and OS event delivery are timing-dependent; keep this generous
// so slow machines don't flake.
const EVENT_TIMEOUT: Duration = Duration::from_secs(15);

fn make_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ma_watch_test_{}_{}", name, std::process::id()));
    // Start from a fresh directory so leftovers from a previous run cannot leak in
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Canonicalize so the watched path and the paths notify reports compare
    // equal as strings: on macOS temp_dir() sits behind the /var ->
    // /private/var symlink while FSEvents reports resolved paths.
    dir.canonicalize().unwrap()
}

// The path config travels through the agent graph asynchronously and the OS
// watch registration happens after that, so no fixed sleep can guarantee the
// watcher is live. Keep touching a sentinel file until its event arrives;
// from that point on the watcher is known to observe the directory.
async fn wait_for_watcher_ready(patch_id: &str, dir: &Path) {
    let expected_name = format!("%{}/watch_event", patch_id);
    let sentinel = dir.join("watcher_ready_sentinel");
    let sentinel_str = sentinel.to_string_lossy().to_string();
    let deadline = std::time::Instant::now() + EVENT_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(
            !remaining.is_zero(),
            "timed out waiting for the watcher to become ready in {}",
            dir.display()
        );
        std::fs::write(&sentinel, "ping").unwrap();
        let wait = remaining.min(Duration::from_millis(500));
        if let Ok((name, value)) = test_utils::recv_external_output_with_timeout(wait).await
            && name == expected_name
            && value.get_str("path") == Some(sentinel_str.as_str())
        {
            return;
        }
    }
}

// Receive watch events until one targets the given path. OS-dependent extra
// events (e.g. for the parent directory) are skipped instead of failing the
// test on an exact-match basis.
async fn recv_event_for_path(patch_id: &str, path: &Path) -> AgentValue {
    let expected_name = format!("%{}/watch_event", patch_id);
    let expected_path = path.to_string_lossy().to_string();
    let deadline = std::time::Instant::now() + EVENT_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(
            !remaining.is_zero(),
            "timed out waiting for watch event for {}",
            expected_path
        );
        let (name, value) = test_utils::recv_external_output_with_timeout(remaining)
            .await
            .expect("failed to receive watch event");
        if name == expected_name && value.get_str("path") == Some(expected_path.as_str()) {
            return value;
        }
    }
}

#[tokio::test]
async fn test_watch_create() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    let dir = make_test_dir("create");
    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "watch_path",
        AgentValue::string(dir.to_string_lossy().to_string()),
    )
    .await
    .unwrap();

    wait_for_watcher_ready(&patch_id, &dir).await;

    let file = dir.join("created.txt");
    let file_str = file.to_string_lossy().to_string();
    std::fs::write(&file, "hello").unwrap();

    let event = recv_event_for_path(&patch_id, &file).await;
    assert_eq!(event.get_str("kind"), Some("create"));
    let paths = event.get_array("paths").unwrap();
    assert!(paths.iter().any(|p| p.as_str() == Some(file_str.as_str())));

    ma.quit();
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_watch_remove() {
    let ma = test_utils::setup_modular_agent().await;

    let patch_id = test_utils::open_and_start_patch(&ma, PATCH).await.unwrap();

    // The file must exist before the watcher starts, so the removal is the only
    // change it can observe for this path.
    let dir = make_test_dir("remove");
    let file = dir.join("removed.txt");
    std::fs::write(&file, "x").unwrap();

    test_utils::write_and_expect_local_value(
        &ma,
        &patch_id,
        "watch_path",
        AgentValue::string(dir.to_string_lossy().to_string()),
    )
    .await
    .unwrap();

    wait_for_watcher_ready(&patch_id, &dir).await;

    std::fs::remove_file(&file).unwrap();

    let event = recv_event_for_path(&patch_id, &file).await;
    assert_eq!(event.get_str("kind"), Some("remove"));

    ma.quit();
    let _ = std::fs::remove_dir_all(&dir);
}
