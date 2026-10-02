//! ryolune's entry in the lsuite discovery folder (`~/.lsuite/apps/ryolune.json`), so the
//! other apps of the suite (and the agents driving them) know it is installed, where its
//! CLI and MCP server are, and whether this window is running with a bridge to talk to.
//! The format is in `engine/src/lsuite.rs`; nothing secret goes in it.

use crate::app::Ryolune;
use ryolune_engine::{control, lsuite};
use serde_json::{json, Value};

impl Ryolune {
    /// The entry as of now: `running` is filled while this window is open.
    pub(crate) fn discovery_entry(&self, running: bool) -> Value {
        let executable = std::env::current_exe().ok();
        // The macOS bundle around the executable, when there is one.
        let bundle = executable.as_ref().and_then(|exe| {
            exe.ancestors()
                .find(|p| p.extension().is_some_and(|e| e == "app"))
                .map(|p| p.to_path_buf())
        });
        let bridge = self
            .control
            .as_ref()
            .map(|server| json!({ "port": server.port(), "discovery": server.path() }));
        json!({
            "format": lsuite::FORMAT,
            "app": "ryolune",
            "kind": "music",
            "version": env!("CARGO_PKG_VERSION"),
            "executable": executable,
            "bundle": bundle,
            "cli": installed("ryolune-cli"),
            "mcp": installed("ryolune-mcp"),
            "dataDir": ryolune_engine::host::scan::data_dir(),
            "documents": [ryolune_engine::document::EXTENSION],
            "commands": control::COMMANDS.len(),
            "handoffs": { "accepts": ["cut"], "sends": ["audio"] },
            "running": running.then(|| json!({
                "pid": std::process::id(),
                "since": self.started_at,
                "bridge": bridge,
                "document": self.path,
            })),
            "updated": lsuite::now_rfc3339(),
        })
    }

    /// Write the entry. Failing to is not worth an error dialog: the suite just will not
    /// see this window.
    pub(crate) fn publish_discovery(&mut self, running: bool) {
        if cfg!(test) {
            return;
        }
        let entry = self.discovery_entry(running);
        if self.published.as_ref() == Some(&strip_time(&entry)) {
            return;
        }
        if lsuite::write_entry(&entry).is_ok() {
            self.published = Some(strip_time(&entry));
        }
    }
}

/// A companion binary next to this one, or null when it is not there.
fn installed(name: &str) -> Value {
    let path = std::path::PathBuf::from(crate::agent::cli::companion(name));
    if path.is_absolute() && path.is_file() {
        json!(path)
    } else {
        Value::Null
    }
}

/// The entry without its timestamp, to tell whether anything worth writing changed.
fn strip_time(entry: &Value) -> Value {
    let mut entry = entry.clone();
    entry["updated"] = Value::Null;
    entry
}
