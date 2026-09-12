//! Shared plumbing for `ondera-cli` and `ondera-mcp`: run registry commands either inside the
//! running Ondera window (live) or on a `.ondera` file in this process (headless).

use ondera_engine::{
    control::{self, wire::Client, Headless, Host},
    session_file::SessionFileLock,
    Result,
};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub enum Backend {
    /// Talks to the desktop app. The client reconnects on the next call after a lost connection.
    Live(Option<Client>),
    Headless(Headless, bool, Option<SessionFileLock>),
}
impl Backend {
    pub fn live() -> Result<Self> {
        Client::connect().map(|c| Backend::Live(Some(c)))
    }
    /// A headless host on `path` (which must exist unless `create` is set) or on an empty session.
    pub fn headless(path: Option<&Path>, create: bool) -> Result<Self> {
        let lock = path.map(SessionFileLock::acquire).transpose()?;
        let resolved = lock.as_ref().map(|lock| lock.path());
        let host = match resolved {
            Some(p) if p.exists() || !create => Headless::open(p)?,
            Some(p) => {
                let mut host = Headless::new();
                host.path = Some(p.to_path_buf());
                host
            }
            None => Headless::new(),
        };
        Ok(Backend::Headless(host, false, lock))
    }

    pub fn mode(&self) -> &'static str {
        match self {
            Backend::Live(_) => "live",
            Backend::Headless(_, _, _) => "headless",
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
            Backend::Headless(host, changed, ownership) => {
                control::validate_request(name, params)?;
                let pinned = host.path.clone();
                let before = host.store.revision;
                let mut params = params.clone();
                let requested_path = if matches!(name, "session.open" | "session.save") {
                    params
                        .get("path")
                        .and_then(Value::as_str)
                        .map(PathBuf::from)
                        .or_else(|| {
                            if name == "session.save" {
                                pinned.clone()
                            } else {
                                None
                            }
                        })
                } else {
                    None
                };
                let mut replacement = None;
                if let Some(path) = requested_path {
                    let resolved = SessionFileLock::resolve(&path)?;
                    if ownership
                        .as_ref()
                        .is_none_or(|lock| lock.path() != resolved)
                    {
                        replacement = Some(SessionFileLock::acquire(&resolved)?);
                    }
                    // Resolve symlinks consistently: locking a target and replacing its
                    // symlink would otherwise create two distinct session identities.
                    if !params.is_object() {
                        params = Value::Object(Map::new());
                    }
                    params["path"] = Value::String(resolved.to_string_lossy().into_owned());
                }
                let result = control::call(host, name, &params, agent);
                if result.is_ok() {
                    if replacement.is_some() {
                        *ownership = replacement;
                    }
                    if matches!(name, "session.open" | "session.save") {
                        *changed = false;
                    } else if host.store.revision != before
                        || matches!(
                            name,
                            "session.new"
                                | "transport.locate"
                                | "transport.returnToStart"
                                | "track.select"
                                | "clip.select"
                        )
                    {
                        *changed = true;
                    }
                }
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
            Backend::Headless(h, changed, _)
                if h.path
                    .as_deref()
                    .is_some_and(|p| *changed || h.store.dirty() || !p.exists()) =>
            {
                let path = Host::save(h, None)?;
                *changed = false;
                Ok(Some(path))
            }
            _ => Ok(None),
        }
    }
    pub fn path(&self) -> Option<PathBuf> {
        match self {
            Backend::Headless(h, _, _) => h.path.clone(),
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
        Some(control::Kind::Number) => {
            let number = raw
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .ok_or_else(|| format!("`{key}` must be a number (finite), got `{raw}`"))?;
            Value::from(number)
        }
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

/// Scanner subprocess mode must be available in both tools: the isolated scanner
/// re-executes the binary that owns the headless host.
pub fn scan_child(args: &[String]) -> Option<Result<()>> {
    if args.first().map(String::as_str) != Some("--scan-plugin") {
        return None;
    }
    Some((|| {
        if args.len() != 3 {
            return Err("Usage: --scan-plugin <clap|vst3> <bundle>".into());
        }
        let format = ondera_engine::plugin::Format::parse(&format!("{}:x", args[1]))
            .map(|(f, _)| f)
            .ok_or("Unknown plugin format")?;
        let result = ondera_engine::host::scan::probe(format, Path::new(&args[2]));
        println!(
            "{}",
            serde_json::to_string(&result).map_err(|e| e.to_string())?
        );
        Ok(())
    })())
}
