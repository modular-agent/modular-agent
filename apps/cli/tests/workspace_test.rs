//! Runs the `ma` binary against a copy of examples/workspace.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const MA: &str = env!("CARGO_BIN_EXE_ma");

/// A fresh copy of examples/workspace, so writes (data/) stay out of the repo.
fn example_workspace() -> tempfile::TempDir {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/workspace");
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&src, dir.path());
    dir
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// `ma run` with `input` on stdin. `--drain` keeps the outputs of the last
/// lines from being cut off by the shutdown that EOF starts.
fn run(workspace: &Path, args: &[&str], input: &str) -> Output {
    let mut child = Command::new(MA)
        .args(["run", "--drain", "1s", "--workspace"])
        .arg(workspace)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn run_echo() {
    let ws = example_workspace();
    let out = run(ws.path(), &["echo"], "hi\n");
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "echo: hi\n");
}

#[test]
fn run_json_keeps_types() {
    let ws = example_workspace();
    let out = run(
        ws.path(),
        &["--json", "-i", "parse", "-o", "parsed", "parse"],
        "\"{\\\"a\\\":[1,true]}\"\n",
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "{\"a\":[1,true]}\n");
}

#[test]
fn run_parse_error_is_logged() {
    let ws = example_workspace();
    let out = run(
        ws.path(),
        &["-i", "parse", "-o", "parsed", "parse"],
        "nope\n",
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "");
    let stderr = text(&out.stderr);
    assert!(
        stderr
            .lines()
            .any(|l| l.contains("ERROR") && l.contains("bad input")),
        "{stderr}"
    );
}

#[test]
fn run_journal_writes_under_the_workspace() {
    let ws = example_workspace();
    let out = run(ws.path(), &["-i", "journal", "journal"], "one\ntwo\n");
    assert!(out.status.success(), "{}", text(&out.stderr));
    let journal = std::fs::read_to_string(ws.path().join("data/journal.jsonl")).unwrap();
    assert_eq!(journal, "\"one\"\n\"two\"\n");
}

/// Settings for a module this build does not link (e.g. a desktop-only one)
/// are skipped with a warning.
#[test]
fn unknown_module_is_only_a_warning() {
    let ws = example_workspace();
    std::fs::write(
        ws.path().join("settings.json"),
        r#"{ "modules": { "modular_agent_not_linked::Module": { "key": "value" } } }"#,
    )
    .unwrap();
    let out = run(ws.path(), &["echo"], "hi\n");
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "echo: hi\n");
    assert!(text(&out.stderr).contains("modular_agent_not_linked::Module"));
}

/// Kills the server if a test fails before stopping it.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

#[test]
fn serve_runs_the_workspace() {
    let ws = example_workspace();
    let mut server = Server(
        Command::new(MA)
            .args([
                "--log-format",
                "json",
                "serve",
                "--listen",
                "127.0.0.1:0",
                "--workspace",
            ])
            .arg(ws.path())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let lines = log_lines(server.0.stderr.take().unwrap());

    let addr = wait_for(&lines, |l| {
        let rest = l["message"]
            .as_str()?
            .strip_prefix("listening on http://")?;
        rest.split_whitespace().next().map(str::to_string)
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    while healthz(&addr) != 200 {
        assert!(Instant::now() < deadline, "/healthz never became ready");
        std::thread::sleep(Duration::from_millis(100));
    }

    // IntervalTimer fires 5s after start.
    let heartbeat = wait_for(&lines, |l| {
        (l["target"] == "modular_agent::patch" && l["patch"] == "heartbeat")
            .then(|| l["message"].as_str().map(str::to_string))?
    });
    assert_eq!(heartbeat, "heartbeat 1");

    stop(&mut server.0);
    #[cfg(unix)]
    {
        wait_for(&lines, |l| (l["message"] == "patch stopped").then_some(()));
        assert!(server.0.wait().unwrap().success());
    }
}

#[cfg(unix)]
fn stop(child: &mut Child) {
    let status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
}

/// Windows has no SIGTERM to send; graceful shutdown is covered on unix.
#[cfg(not(unix))]
fn stop(child: &mut Child) {
    let _ = child.kill();
}

/// JSON log lines from the server's stderr, read on a thread.
fn log_lines(stderr: impl Read + Send + 'static) -> mpsc::Receiver<serde_json::Value> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Ok(json) = serde_json::from_str(&line)
                && tx.send(json).is_err()
            {
                return;
            }
        }
    });
    rx
}

fn wait_for<T>(
    lines: &mpsc::Receiver<serde_json::Value>,
    mut matches: impl FnMut(&serde_json::Value) -> Option<T>,
) -> T {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let line = lines
            .recv_timeout(remaining)
            .expect("expected log line did not appear");
        if let Some(found) = matches(&line) {
            return found;
        }
    }
}

/// Status code of GET /healthz, over a raw socket to avoid an HTTP client.
fn healthz(addr: &str) -> u16 {
    let Ok(mut stream) = TcpStream::connect(addr) else {
        return 0;
    };
    write!(
        stream,
        "GET /healthz HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    response
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0)
}
