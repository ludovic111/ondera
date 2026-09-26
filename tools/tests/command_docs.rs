//! `docs/COMMANDS.md` is generated from the command registry, so the reference can never
//! drift from what the window, the CLI, MCP and the agent actually accept. When a command
//! changes, regenerate it:
//!
//! ```sh
//! ONDERA_BLESS=1 cargo test -p ondera-tools --test command_docs
//! ```
use ondera_engine::{control, control_app};
use serde_json::Value;
use std::fmt::Write;

fn reference() -> String {
    let commands = control::describe();
    let commands = commands.as_array().expect("describe() lists commands");
    let mut families: Vec<(&str, Vec<&Value>)> = Vec::new();
    for c in commands {
        let family = c["name"].as_str().unwrap().split('.').next().unwrap();
        match families.iter_mut().find(|(f, _)| *f == family) {
            Some((_, list)) => list.push(c),
            None => families.push((family, vec![c])),
        }
    }
    let mut out = String::new();
    out.push_str("# Command reference\n\n");
    out.push_str(
        "<!-- Generated from the command registry by tools/tests/command_docs.rs. \
         Do not edit by hand: run `ONDERA_BLESS=1 cargo test -p ondera-tools --test command_docs`. -->\n\n",
    );
    let _ = writeln!(
        out,
        "Ondera has {} commands. The window, `ondera-cli`, `ondera-mcp` and the built-in agent all \
         run these same commands, with the same undo history. On the CLI a command is \
         `ondera-cli <name> --param value`; in MCP it is the tool `<name>` with the dot replaced \
         by an underscore (`track.add` is `track_add`); the agent sees the same tools.\n",
        commands.len()
    );
    out.push_str(
        "Conventions: bars and beats are zero-based; note `start` and `length` are beats relative \
         to their clip; pitch 60 is C4; velocity is 1–127; a fader value of 0.75 is unity gain. \
         Strip commands accept a track id or `master`, `bus-a`, `bus-b`; insert slots are 0–7.\n\n",
    );
    out.push_str(
        "**Edits** marks a command that can change the song, the transport, settings or files \
         (it is one undo step when it changes the song). **Needs the app** marks a command only \
         the running window can serve; the others also work on a file (`ondera-cli --file song.ondera …`).\n\n",
    );
    out.push_str("## Families\n\n");
    for (family, list) in &families {
        let _ = writeln!(
            out,
            "- [{family}](#{family}) — {}",
            list.iter()
                .map(|c| format!("`{}`", c["name"].as_str().unwrap()))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    for (family, list) in &families {
        let _ = write!(out, "\n## {family}\n");
        for c in list {
            let name = c["name"].as_str().unwrap();
            let mut tags = Vec::new();
            if c["mutates"].as_bool().unwrap_or(false) {
                tags.push("Edits");
            }
            if control_app::is_live_only(name) {
                tags.push("Needs the app");
            }
            let _ = write!(out, "\n### `{name}`\n\n");
            if !tags.is_empty() {
                let _ = writeln!(out, "*{}*\n", tags.join(" · "));
            }
            let _ = writeln!(out, "{}", c["description"].as_str().unwrap_or("").trim());
            let params = c["params"].as_array().unwrap();
            if params.is_empty() {
                continue;
            }
            out.push_str("\n| Parameter | Type | Required | Description |\n|---|---|---|---|\n");
            for p in params {
                let _ = writeln!(
                    out,
                    "| `{}` | {} | {} | {} |",
                    p["name"].as_str().unwrap(),
                    p["type"].as_str().unwrap_or("any"),
                    if p["required"].as_bool().unwrap_or(false) {
                        "yes"
                    } else {
                        ""
                    },
                    p["description"]
                        .as_str()
                        .unwrap_or("")
                        .replace('|', "\\|")
                        .replace('\n', " ")
                );
            }
        }
    }
    out
}

#[test]
fn the_command_reference_matches_the_registry() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/COMMANDS.md");
    let fresh = reference();
    if std::env::var_os("ONDERA_BLESS").is_some() {
        std::fs::write(&path, &fresh).unwrap();
        return;
    }
    let current = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        current == fresh,
        "docs/COMMANDS.md is out of date: run `ONDERA_BLESS=1 cargo test -p ondera-tools --test command_docs`"
    );
}
