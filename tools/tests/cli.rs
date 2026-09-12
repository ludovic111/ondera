use serde_json::Value;
use std::{path::Path, process::Command};

fn cli(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ondera-cli"))
        .args(args)
        // Never reach a real running app from tests.
        .env("ONDERA_CONTROL", dir.join("absent-control.json"))
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn help_and_registry_listing() {
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = cli(dir.path(), &["--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("--file"));
    let (code, out, _) = cli(dir.path(), &["commands"]);
    assert_eq!(code, 0);
    assert!(out.contains("track.add") && out.contains("--kind"));
    let (code, out, _) = cli(dir.path(), &["help", "clip.create"]);
    assert_eq!(code, 0);
    assert!(out.contains("--notes") && out.contains("(optional)"));
    let (code, _, err) = cli(dir.path(), &[]);
    assert_eq!(code, 2);
    assert!(err.contains("Missing command"));
}

#[test]
fn without_the_app_the_cli_explains_both_modes() {
    let dir = tempfile::tempdir().unwrap();
    let (code, _, err) = cli(dir.path(), &["session.info"]);
    assert_eq!(code, 1);
    assert!(
        err.contains("not running") && err.contains("--file"),
        "{err}"
    );
}

#[test]
fn file_mode_saves_after_every_change() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ondera");
    let file = song.to_str().unwrap();
    let (code, _, err) = cli(dir.path(), &["--file", file, "track.list"]);
    assert_eq!(
        code, 1,
        "a missing file is an error unless session.new creates it"
    );
    assert!(!err.is_empty());
    let (code, out, _) = cli(dir.path(), &["--file", file, "session.new"]);
    assert_eq!(code, 0, "{out}");
    assert!(song.exists());
    let (code, out, err) = cli(
        dir.path(),
        &[
            "--file",
            file,
            "track.add",
            "--kind",
            "midi",
            "--name",
            "Bass",
            "instrument=Sub Bass 808",
        ],
    );
    assert_eq!(code, 0, "{err}");
    let track: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(track["name"], "Bass");
    assert!(err.contains("saved"), "{err}");
    let id = track["id"].as_str().unwrap();
    let (code, out, err) = cli(
        dir.path(),
        &[
            "--file",
            file,
            "--compact",
            "clip.create",
            "--trackId",
            id,
            "--startBar",
            "0",
            "--lengthBars",
            "1",
            "--notes",
            r#"[{"start":0,"length":1,"pitch":36}]"#,
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert!(
        !out.trim().contains('\n'),
        "compact output is one line: {out}"
    );
    let clip: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(clip["noteCount"], 1);
    let (code, out, _) = cli(
        dir.path(),
        &[
            "--file",
            file,
            "--params",
            r#"{"trackId":"REPLACED"}"#,
            "clip.list",
            "--trackId",
            id,
        ],
    );
    assert_eq!(code, 0, "explicit --param values override --params");
    assert_eq!(
        serde_json::from_str::<Value>(&out)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let (code, _, err) = cli(
        dir.path(),
        &[
            "--file",
            file,
            "track.setVolume",
            "--trackId",
            id,
            "--volume",
            "loud",
        ],
    );
    assert_eq!(code, 1);
    assert!(err.contains("must be a number"), "{err}");
    let (code, _, err) = cli(
        dir.path(),
        &[
            "--file",
            file,
            "track.rename",
            "--trackId",
            id,
            "--nam",
            "x",
        ],
    );
    assert_eq!(code, 1);
    assert!(err.contains("Unknown parameter `nam`"), "{err}");
    let (code, out, _) = cli(dir.path(), &["--file", file, "session.info"]);
    assert_eq!(code, 0);
    let info: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(info["clipCount"], 1);
    assert_eq!(info["dirty"], false);
}
