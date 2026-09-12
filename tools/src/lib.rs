//! Shared plumbing for `ondera-cli` and `ondera-mcp`: run registry commands either inside the
//! running Ondera window (live) or on a `.ondera` file in this process (headless).

use ondera_engine::{
    control::{self, wire::Client, Headless, Host},
    Result,
};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub enum Backend {
    /// Talks to the desktop app. The client reconnects on the next call after a lost connection.
    Live(Option<Client>),
    Headless(Headless),
}
impl Backend {
    pub fn live() -> Result<Self> {
        Client::connect().map(|c| Backend::Live(Some(c)))
    }
    /// A headless host on `path` (which must exist unless `create` is set) or on an empty session.
    pub fn headless(path: Option<&Path>, create: bool) -> Result<Self> {
        Ok(Backend::Headless(match path {
            Some(p) if p.exists() || !create => Headless::open(p)?,
            Some(p) => {
                let mut h = Headless::new();
                h.path = Some(p.to_path_buf());
                h
            }
            None => Headless::new(),
        }))
    }
    pub fn mode(&self) -> &'static str {
        match self {
            Backend::Live(_) => "live",
            Backend::Headless(_) => "headless",
        }
    }
    pub fn call(&mut self, name: &str, params: &Value, agent: bool) -> Result<Value> {
        match self {
            Backend::Live(client) => {
                if client.is_none() {
                    *client = Some(Client::connect()?);
                }
                let result = client
                    .as_mut()
                    .expect("connected above")
                    .call(name, params, agent);
                if result
                    .as_ref()
                    .is_err_and(|e| e.starts_with("Lost connection"))
                {
                    *client = None;
                }
                result
            }
            Backend::Headless(host) => {
                // In file mode the file stays the state: session.new resets its contents
                // instead of detaching from it.
                let pinned = host.path.clone();
                let result = control::call(host, name, params, agent);
                if name == "session.new" && host.path.is_none() {
                    host.path = pinned;
                }
                result
            }
        }
    }
    /// Headless file hosts write every change back so the file is always the state. A file
    /// that does not exist yet (`session.new`) is written even though the store is clean.
    pub fn autosave(&mut self) -> Result<Option<PathBuf>> {
        match self {
            Backend::Headless(h)
                if h.path
                    .as_deref()
                    .is_some_and(|p| h.store.dirty() || !p.exists()) =>
            {
                Host::save(h, None).map(Some)
            }
            _ => Ok(None),
        }
    }
    pub fn path(&self) -> Option<PathBuf> {
        match self {
            Backend::Headless(h) => h.path.clone(),
            Backend::Live(_) => None,
        }
    }
}

/// Coerce a command-line value to the parameter's declared type. Unknown parameters fall back to
/// JSON-then-string so the registry can name them in its error.
pub fn coerce(command: &str, key: &str, raw: &str) -> Result<Value> {
    let kind = control::spec(command)
        .and_then(|s| s.params.iter().find(|p| p.name == key))
        .map(|p| p.kind);
    let json = || serde_json::from_str::<Value>(raw);
    Ok(match kind {
        Some(control::Kind::String) => Value::String(raw.into()),
        Some(control::Kind::Number) => Value::from(
            raw.parse::<f64>()
                .map_err(|_| format!("`{key}` must be a number, got `{raw}`"))?,
        ),
        Some(control::Kind::Integer) => Value::from(
            raw.parse::<i64>()
                .map_err(|_| format!("`{key}` must be an integer, got `{raw}`"))?,
        ),
        Some(control::Kind::Boolean) => match raw {
            "true" | "yes" | "on" | "1" => Value::Bool(true),
            "false" | "no" | "off" | "0" => Value::Bool(false),
            _ => return Err(format!("`{key}` must be true or false, got `{raw}`")),
        },
        Some(control::Kind::Array | control::Kind::Object) => {
            json().map_err(|e| format!("`{key}` must be JSON: {e}"))?
        }
        None => json().unwrap_or_else(|_| Value::String(raw.into())),
    })
}

/// Merge `--params` JSON with `--key value` / `key=value` arguments into one object.
pub fn merge(base: Option<&str>, pairs: &[(String, Value)]) -> Result<Value> {
    let mut map = match base {
        Some(text) => match serde_json::from_str::<Value>(text)
            .map_err(|e| format!("--params must be a JSON object: {e}"))?
        {
            Value::Object(m) => m,
            _ => return Err("--params must be a JSON object".into()),
        },
        None => Map::new(),
    };
    for (k, v) in pairs {
        map.insert(k.clone(), v.clone());
    }
    Ok(Value::Object(map))
}
