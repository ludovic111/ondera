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

#[test]
fn cli_composes_mixes_exports_and_resets_an_existing_song() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ondera");
    let mix = dir.path().join("mix.wav");
    let file = song.to_str().unwrap();
    let run = |name: &str, params: Value| -> Value {
        let json = params.to_string();
        let (code, out, err) = cli(dir.path(), &["--file", file, "--params", &json, name]);
        assert_eq!(code, 0, "{name}: {err}");
        serde_json::from_str(&out).unwrap()
    };
    run("session.new", serde_json::json!({}));
    run("transport.setTempo", serde_json::json!({"bpm":120}));
    let mut lead = Value::Null;
    for (instrument, pitches) in [
        ("Glass Keys", [60, 64, 67, 72]),
        ("Sub Bass 808", [36, 36, 43, 41]),
    ] {
        let track = run(
            "track.add",
            serde_json::json!({"kind":"midi","instrument":instrument}),
        )["id"]
            .clone();
        lead = track.clone();
        let notes: Vec<Value> = pitches
            .iter()
            .enumerate()
            .map(|(i, p)| serde_json::json!({"start":i*2,"length":1.5,"pitch":p,"velocity":90}))
            .collect();
        let clip = run(
            "clip.create",
            serde_json::json!({"trackId":track,"startBar":0,"lengthBars":2,"notes":notes}),
        );
        run("clip.duplicate", serde_json::json!({"clipId":clip["id"]}));
        run(
            "strip.setSendLevel",
            serde_json::json!({"trackId":track,"send":0,"levelDb":-18}),
        );
        run(
            "track.setVolume",
            serde_json::json!({"trackId":track,"volume":0.65}),
        );
    }
    run(
        "strip.setPlugin",
        serde_json::json!({"trackId":"master","slot":7,"pluginId":"stock:Space"}),
    );
    run("master.setVolume", serde_json::json!({"volume":0.6}));
    run("track.select", serde_json::json!({"trackId":lead}));
    run("transport.locate", serde_json::json!({"bar":2}));
    let loaded = ondera_engine::document::load(&song).unwrap().0;
    assert_eq!(
        loaded.transport.position_beats, 8.0,
        "file mode persists locate commands even though they are transient"
    );
    assert_eq!(loaded.view.selected_track_id.as_deref(), lead.as_str());
    run("session.bounce", serde_json::json!({"path":mix}));
    let audio = ondera_engine::audio::decode(std::fs::read(&mix).unwrap(), Some("wav")).unwrap();
    assert_eq!(audio.sample_rate, 48000);
    assert!(audio.duration() >= 11.0, "four bars plus effect tail");
    assert!(audio.frames.iter().flatten().all(|s| s.is_finite()));
    let energy: f64 = audio
        .frames
        .iter()
        .flatten()
        .map(|s| (*s as f64).powi(2))
        .sum();
    assert!(energy > 1.0, "export contains audible music");
    run("session.new", serde_json::json!({}));
    assert_eq!(
        ondera_engine::document::load(&song).unwrap().0.clips.len(),
        0,
        "session.new overwrites an existing file even though the new store starts clean"
    );
}

#[test]
fn cli_rejects_nonfinite_numbers_without_modifying_files() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ondera");
    let file = song.to_str().unwrap();
    assert_eq!(cli(dir.path(), &["--file", file, "session.new"]).0, 0);
    let before = std::fs::read(&song).unwrap();
    let (code, _, err) = cli(
        dir.path(),
        &["--file", file, "transport.setTempo", "--bpm", "NaN"],
    );
    assert_eq!(code, 1);
    assert!(err.contains("finite"));
    assert_eq!(std::fs::read(&song).unwrap(), before);
}

#[test]
fn cli_midi_interchange_and_configured_wav_export() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("interchange.ondera");
    let midi = dir.path().join("notes.mid");
    let wav = dir.path().join("range.wav");
    let run = |name: &str, params: Value| -> Value {
        let (code, out, err) = cli(
            dir.path(),
            &[
                "--file",
                song.to_str().unwrap(),
                "--params",
                &params.to_string(),
                name,
            ],
        );
        assert_eq!(code, 0, "{name}: {err}");
        serde_json::from_str(&out).unwrap()
    };
    run("session.new", serde_json::json!({}));
    run("transport.setTempo", serde_json::json!({"bpm":120}));
    let track = run("track.add", serde_json::json!({"kind":"midi"}))["id"].clone();
    run(
        "clip.create",
        serde_json::json!({"trackId":track,"startBar":0,"lengthBars":1,"notes":[{"start":0,"length":2,"pitch":60}]}),
    );
    let report = run(
        "session.exportMidi",
        serde_json::json!({"path":midi,"trackIds":[track]}),
    );
    assert_eq!(report["noteCount"], 1);
    let imported = run(
        "session.importMidi",
        serde_json::json!({"path":midi,"startBar":2,"importTempo":true}),
    );
    assert_eq!(imported["notes"], 1);
    assert_eq!(imported["tempoImported"], true);
    let report = run(
        "session.exportAudio",
        serde_json::json!({"path":wav,"sampleRate":44100,"format":"float32","startBeat":0,"endBeat":1,"tailSeconds":0}),
    );
    assert_eq!(report["frames"], 22050);
    let audio = ondera_engine::audio::decode(std::fs::read(wav).unwrap(), Some("wav")).unwrap();
    assert_eq!(audio.sample_rate, 44100);
    assert!((audio.duration() - 0.5).abs() < 0.0001);
}

#[test]
fn batch_mode_runs_json_lines_against_a_file_and_stops_at_the_first_error() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("batch.ondera");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ondera-cli"))
        .args(["--file", song.to_str().unwrap(), "batch"])
        .env("ONDERA_CONTROL", dir.path().join("absent-control.json"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "# comment lines and blank lines are skipped").unwrap();
        writeln!(stdin).unwrap();
        writeln!(stdin, r#"{{"command":"session.new"}}"#).unwrap();
        writeln!(
            stdin,
            r#"{{"method":"track.add","params":{{"kind":"midi","name":"Batch keys"}}}}"#
        )
        .unwrap();
        writeln!(
            stdin,
            r#"{{"command":"track.add","params":{{"kind":"drums"}}}}"#
        )
        .unwrap();
        writeln!(
            stdin,
            r#"{{"command":"session.rename","params":{{"name":"never reached"}}}}"#
        )
        .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let lines: Vec<Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[0]["ok"], true);
    assert_eq!(lines[1]["result"]["name"], "Batch keys");
    assert!(lines[1]["saved"]
        .as_str()
        .unwrap()
        .ends_with("batch.ondera"));
    assert_eq!(lines[2]["ok"], false);
    assert!(lines[2]["error"].as_str().unwrap().contains("midi"));
    let loaded = ondera_engine::document::load(&song).unwrap().0;
    assert_eq!(
        loaded.tracks.len(),
        4,
        "the three starter tracks and Batch keys"
    );
    assert_ne!(loaded.name, "never reached");
}

#[test]
fn doctor_and_json_command_listing_work_without_the_app() {
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = cli(dir.path(), &["commands", "--json"]);
    assert_eq!(code, 0);
    let listed: Value = serde_json::from_str(&out).unwrap();
    let names: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    for name in [
        "view.set",
        "clip.quantize",
        "preset.load",
        "settings.set",
        "ui.screenshot",
        "agent.send",
    ] {
        assert!(names.contains(&name), "{name}");
    }
    let (code, out, _) = cli(dir.path(), &["doctor", "--json"]);
    assert_eq!(
        code, 1,
        "the app is not running, so doctor reports a failure"
    );
    let report: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(report["ok"], false);
    let checks = report["checks"].as_array().unwrap();
    assert!(checks
        .iter()
        .any(|c| c["check"] == "discovery" && c["ok"] == false));
    assert!(checks
        .iter()
        .any(|c| c["check"] == "plugins" && c["ok"] == true));
    let (code, out, _) = cli(dir.path(), &["help", "settings.set"]);
    assert_eq!(code, 0);
    assert!(out.contains("--value") && out.contains("any"));
}

#[test]
fn file_mode_overview_and_plugin_parameters_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ondera");
    let file = song.to_str().unwrap();
    let (code, _, err) = cli(
        dir.path(),
        &["--file", file, "session.new", "--demo", "true"],
    );
    assert_eq!(code, 0, "{err}");
    let (code, out, err) = cli(dir.path(), &["--file", file, "session.overview"]);
    assert_eq!(code, 0, "{err}");
    let overview: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(overview["song"]["tempo"], 120.0);
    assert!(overview["next"]["parameters"].is_string());
    let (code, out, err) = cli(
        dir.path(),
        &[
            "--file",
            file,
            "strip.parameters",
            "--trackId",
            "Bass",
            "--slot",
            "0",
            "--query",
            "ratio",
        ],
    );
    assert_eq!(code, 0, "{err}");
    let found: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(found["parameters"][0]["name"], "Ratio");
    let (code, _, err) = cli(
        dir.path(),
        &[
            "--file",
            file,
            "track.setSolo",
            "--trackId",
            "Drms",
            "--solo",
            "true",
        ],
    );
    assert_eq!(code, 1);
    assert!(err.contains("Did you mean Drums?"), "{err}");
}
