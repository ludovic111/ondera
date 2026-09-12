use ondera_tools::Backend;
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

fn file(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    dir.join(name)
}
#[test]
fn file_ownership_covers_atomic_save_new_and_path_transitions() {
    let dir = tempfile::tempdir().unwrap();
    let a = file(dir.path(), "a.ondera");
    let b = file(dir.path(), "b.ondera");
    let c = file(dir.path(), "c.ondera");
    let mut first = Backend::headless(Some(&a), true).unwrap();
    first.autosave().unwrap();
    let mut second = Backend::headless(Some(&b), true).unwrap();
    second.autosave().unwrap();
    assert!(Backend::headless(Some(&a), false).is_err());
    first
        .call("session.rename", &json!({"name":"Owner A"}), false)
        .unwrap();
    first.autosave().unwrap();
    assert!(
        Backend::headless(Some(&a), false).is_err(),
        "atomic file replacement must not release its ownership lock"
    );
    assert!(first
        .call("session.open", &json!({"path":b}), false)
        .unwrap_err()
        .contains("already in use"));
    assert_eq!(
        first.call("session.info", &json!({}), false).unwrap()["name"],
        "Owner A"
    );
    assert!(first
        .call("session.save", &json!({"path":b}), false)
        .is_err());
    assert_eq!(
        ondera_engine::document::load(&b).unwrap().0.name,
        "Untitled.ondera"
    );
    first
        .call("session.save", &json!({"path":c}), false)
        .unwrap();
    assert!(
        Backend::headless(Some(&a), false).is_ok(),
        "successful Save As releases the previous file"
    );
    assert!(Backend::headless(Some(&c), false).is_err());
    first.call("session.new", &json!({}), false).unwrap();
    first.autosave().unwrap();
    assert!(
        Backend::headless(Some(&c), false).is_err(),
        "session.new keeps file-mode ownership"
    );
    drop(second);
    first
        .call("session.open", &json!({"path":b}), false)
        .unwrap();
    assert!(Backend::headless(Some(&c), false).is_ok());
    assert!(Backend::headless(Some(&b), false).is_err());
    drop(first);
    assert!(Backend::headless(Some(&b), false).is_ok());
}

#[cfg(unix)]
#[test]
fn symlink_aliases_share_the_same_file_lock() {
    let dir = tempfile::tempdir().unwrap();
    let song = file(dir.path(), "song.ondera");
    let alias = file(dir.path(), "alias.ondera");
    let mut first = Backend::headless(Some(&song), true).unwrap();
    first.autosave().unwrap();
    std::os::unix::fs::symlink(&song, &alias).unwrap();
    assert!(Backend::headless(Some(&alias), false).is_err());
    drop(first);
    let mut alias_owner = Backend::headless(Some(&alias), false).unwrap();
    alias_owner
        .call("session.rename", &json!({"name":"Through alias"}), false)
        .unwrap();
    alias_owner.autosave().unwrap();
    assert!(std::fs::symlink_metadata(&alias)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        ondera_engine::document::load(&song).unwrap().0.name,
        "Through alias"
    );
}

#[test]
fn failed_open_preserves_current_lock_and_releases_failed_target() {
    let dir = tempfile::tempdir().unwrap();
    let song = file(dir.path(), "song.ondera");
    let bad = file(dir.path(), "bad.ondera");
    let mut first = Backend::headless(Some(&song), true).unwrap();
    first.autosave().unwrap();
    std::fs::write(&bad, b"corrupt document").unwrap();
    assert!(first
        .call("session.open", &json!({"path":bad}), false)
        .is_err());
    assert!(Backend::headless(Some(&song), false).is_err());
    let mut independent = Backend::headless(None, false).unwrap();
    independent
        .call("session.save", &json!({"path":bad}), false)
        .unwrap();
    assert!(Backend::headless(Some(&bad), false).is_err());
}

#[test]
fn a_running_mcp_file_owner_blocks_cli_and_crash_releases_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let song = file(dir.path(), "song.ondera");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ondera-mcp"))
        .args(["--file", song.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    writeln!(
        child.stdin.as_mut().unwrap(),
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"session_info"}}}}"#
    )
    .unwrap();
    let mut reply = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut reply)
        .unwrap();
    let attempt = Command::new(env!("CARGO_BIN_EXE_ondera-cli"))
        .args([
            "--file",
            song.to_str().unwrap(),
            "session.rename",
            "--name",
            "Concurrent overwrite",
        ])
        .output()
        .unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!attempt.status.success());
    assert!(String::from_utf8_lossy(&attempt.stderr).contains("already in use"));
    let after = Command::new(env!("CARGO_BIN_EXE_ondera-cli"))
        .args([
            "--file",
            song.to_str().unwrap(),
            "session.rename",
            "--name",
            "After exit",
        ])
        .output()
        .unwrap();
    assert!(
        after.status.success(),
        "{}",
        String::from_utf8_lossy(&after.stderr)
    );
    assert_eq!(
        ondera_engine::document::load(&song).unwrap().0.name,
        "After exit"
    );
}
