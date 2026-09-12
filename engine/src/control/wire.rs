//! Live control over a loopback socket.
//!
//! The running desktop app listens on 127.0.0.1 and writes `{port, token, pid}` to a discovery
//! file that only the current user can read. Clients send newline-delimited JSON-RPC 2.0: first
//! `auth` with the token, then any command name from the registry. Connection threads forward
//! each request to the thread that owns the store and block until it answers, so commands are
//! applied in order on the same thread as the interface, never concurrently with it.

use super::{call, Host};
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

pub const VERSION: u32 = 1;
/// One request or reply line, including a `clip.setNotes` with thousands of notes.
pub const MAX_LINE: usize = 64 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Discovery {
    pub version: u32,
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

/// `$ONDERA_CONTROL`, else `~/.ondera/control.json`.
pub fn discovery_path() -> PathBuf {
    if let Some(p) = std::env::var_os("ONDERA_CONTROL").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join(".ondera").join("control.json")
}
pub fn read_discovery(path: &Path) -> Result<Discovery> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "Ondera is not running (no control file at {}): {e}",
            path.display()
        )
    })?;
    let d: Discovery =
        serde_json::from_str(&text).map_err(|e| format!("Invalid control file: {e}"))?;
    if d.version != VERSION {
        return Err(format!(
            "Ondera control protocol {} is not supported by this client ({VERSION})",
            d.version
        ));
    }
    Ok(d)
}

fn token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    (0..4u64)
        .map(|i| {
            // RandomState keys come from the operating system's randomness.
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i);
            h.write_u128(nanos);
            h.write_u32(std::process::id());
            format!("{:016x}", h.finish())
        })
        .collect()
}
fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// One command waiting for the store's thread.
pub struct Request {
    pub method: String,
    pub params: Value,
    pub agent: bool,
    reply: mpsc::SyncSender<Result<Value>>,
}
impl Request {
    pub fn respond(self, result: Result<Value>) {
        let _ = self.reply.send(result);
    }
}
/// Answer a request with the registry.
pub fn serve(host: &mut dyn Host, request: Request) {
    let result = call(host, &request.method, &request.params, request.agent);
    request.respond(result);
}

/// Listener owned by the process that owns the store. Dropping it removes the discovery file.
pub struct Server {
    path: PathBuf,
    token: String,
    port: u16,
    receiver: mpsc::Receiver<Request>,
    stopping: Arc<AtomicBool>,
}
impl Server {
    /// `wake` runs on a connection thread after each request is queued; the desktop uses it to
    /// request a repaint so the store thread notices without polling quickly.
    pub fn start(wake: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        Self::start_at(discovery_path(), wake)
    }
    pub fn start_at(path: PathBuf, wake: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let token = token();
        write_discovery(
            &path,
            &Discovery {
                version: VERSION,
                port,
                token: token.clone(),
                pid: std::process::id(),
            },
        )?;
        let (sender, receiver) = mpsc::channel();
        let stopping = Arc::new(AtomicBool::new(false));
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let (stop, expected) = (stopping.clone(), token.clone());
        std::thread::Builder::new()
            .name("ondera-control".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    if !stream.peer_addr().is_ok_and(|a| a.ip().is_loopback()) {
                        continue;
                    }
                    let (sender, expected, wake) = (sender.clone(), expected.clone(), wake.clone());
                    let _ = std::thread::Builder::new()
                        .name("ondera-control-client".into())
                        .spawn(move || connection(stream, &expected, &sender, &wake));
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            token,
            port,
            receiver,
            stopping,
        })
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Every request queued so far, in arrival order.
    pub fn drain(&self) -> Vec<Request> {
        self.receiver.try_iter().collect()
    }
    /// Block until a request arrives or the timeout passes (headless hosts and tests).
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Request> {
        self.receiver.recv_timeout(timeout).ok()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        // Wake the blocking accept so the listener thread exits.
        let _ = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)),
            Duration::from_millis(200),
        );
        if read_discovery(&self.path).is_ok_and(|d| same_token(&d.token, &self.token)) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
fn write_discovery(path: &Path, discovery: &Discovery) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string(discovery).map_err(|e| e.to_string())?;
    crate::document::atomic_write(path, |f| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
        f.write_all(text.as_bytes()).map_err(|e| e.to_string())
    })
}

fn read_frame(reader: &mut BufReader<TcpStream>) -> Option<String> {
    let mut line = String::new();
    let mut limited = reader.by_ref().take(MAX_LINE as u64);
    match limited.read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) if !line.ends_with('\n') && line.len() >= MAX_LINE => None,
        Ok(_) => Some(line),
    }
}
fn error_frame(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
fn connection(
    stream: TcpStream,
    expected: &str,
    sender: &mpsc::Sender<Request>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) {
    let _ = stream.set_read_timeout(None);
    let Ok(mut out) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    let mut authenticated = false;
    while let Some(line) = read_frame(&mut reader) {
        if line.trim().is_empty() {
            continue;
        }
        let frame: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(
                    out,
                    "{}",
                    error_frame(Value::Null, -32700, &format!("Parse error: {e}"))
                );
                continue;
            }
        };
        let id = frame.get("id").cloned().unwrap_or(Value::Null);
        let method = frame.get("method").and_then(Value::as_str).unwrap_or("");
        let params = frame.get("params").cloned().unwrap_or(Value::Null);
        let response = if method == "auth" {
            let given = params.get("token").and_then(Value::as_str).unwrap_or("");
            if same_token(given, expected) {
                authenticated = true;
                json!({ "jsonrpc": "2.0", "id": id, "result": { "ok": true, "app": "ondera", "version": env!("CARGO_PKG_VERSION"), "protocol": VERSION } })
            } else {
                let _ = writeln!(out, "{}", error_frame(id, -32001, "Invalid control token"));
                return;
            }
        } else if !authenticated {
            let _ = writeln!(out, "{}", error_frame(id, -32001, "Authenticate first"));
            return;
        } else if method.is_empty() {
            error_frame(id, -32600, "Request needs a `method`")
        } else {
            let (reply, done) = mpsc::sync_channel(1);
            let request = Request {
                method: method.into(),
                params,
                agent: frame.get("agent").and_then(Value::as_bool).unwrap_or(false),
                reply,
            };
            if sender.send(request).is_err() {
                let _ = writeln!(
                    out,
                    "{}",
                    error_frame(id, -32002, "Ondera is shutting down")
                );
                return;
            }
            wake();
            match done.recv() {
                Ok(Ok(result)) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Ok(Err(message)) => error_frame(id, -32000, &message),
                Err(_) => error_frame(id, -32002, "Ondera dropped the request"),
            }
        };
        if writeln!(out, "{response}").is_err() {
            return;
        }
    }
}

/// Client side of the live protocol.
pub struct Client {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next_id: u64,
    pub pid: u32,
    pub app_version: String,
}
impl Client {
    pub fn connect() -> Result<Self> {
        Self::connect_at(&discovery_path())
    }
    pub fn connect_at(path: &Path) -> Result<Self> {
        let d = read_discovery(path)?;
        let stream = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, d.port)),
            CONNECT_TIMEOUT,
        )
        .map_err(|e| {
            format!(
                "Ondera is not running (port {} refused: {e}). Delete {} if the app has quit.",
                d.port,
                path.display()
            )
        })?;
        stream.set_nodelay(true).map_err(|e| e.to_string())?;
        let writer = stream.try_clone().map_err(|e| e.to_string())?;
        let mut client = Self {
            reader: BufReader::new(stream),
            writer,
            next_id: 1,
            pid: d.pid,
            app_version: String::new(),
        };
        let hello = client.exchange(
            json!({ "jsonrpc": "2.0", "id": 0, "method": "auth", "params": { "token": d.token } }),
        )?;
        client.app_version = hello
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into();
        Ok(client)
    }
    /// Run a registry command in the app. Command failures come back as `Err(message)`;
    /// a lost connection is reported with a "Lost connection" prefix.
    pub fn call(&mut self, method: &str, params: &Value, agent: bool) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let mut frame = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        if agent {
            frame["agent"] = json!(true);
        }
        self.exchange(frame)
    }
    fn exchange(&mut self, frame: Value) -> Result<Value> {
        let text = serde_json::to_string(&frame).map_err(|e| e.to_string())?;
        if text.len() >= MAX_LINE {
            return Err("Request exceeds 64 MiB".into());
        }
        writeln!(self.writer, "{text}").map_err(|e| format!("Lost connection to Ondera: {e}"))?;
        let mut line = String::new();
        let mut limited = self.reader.by_ref().take(MAX_LINE as u64);
        match limited.read_line(&mut line) {
            Ok(0) => return Err("Lost connection to Ondera: the app closed the socket".into()),
            Err(e) => return Err(format!("Lost connection to Ondera: {e}")),
            Ok(_) => {}
        }
        let response: Value =
            serde_json::from_str(&line).map_err(|e| format!("Invalid reply from Ondera: {e}"))?;
        if let Some(err) = response.get("error") {
            return Err(err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Unknown error")
                .to_string());
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }
}
