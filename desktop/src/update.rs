//! Updates from GitHub Releases. The check and the download run on a worker thread and the
//! interface only reads their results between frames; nothing here touches the audio callback.
//!
//! A release is published by `.github/workflows/release.yml` from a `vX.Y.Z` tag. It carries one
//! asset per platform plus a `SHA256SUMS` file. The app compares the latest tag with its own
//! version, downloads its asset, verifies the checksum, swaps the installed copy in place and
//! relaunches. Set `ONDERA_PRETEND_VERSION=0.0.1` to exercise the flow against a real release.

use crate::app::{Intent, Ondera};
use crate::theme::*;
use eframe::egui;
use ondera_engine::Result;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

pub const REPO: &str = "ludovic111/ondera";
const API: &str = "https://api.github.com/repos";
const CHECKSUMS: &str = "SHA256SUMS";
const MAX_ASSET: u64 = 512 * 1024 * 1024;

/// The running version, or the one `ONDERA_PRETEND_VERSION` asks us to pretend we are.
pub fn current_version() -> String {
    std::env::var("ONDERA_PRETEND_VERSION")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// The release asset built for this platform, or `None` where no release is built.
pub fn asset_name() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("Ondera-macos-arm64.zip")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("Ondera-macos-x86_64.zip")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("ondera-linux-x86_64")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("ondera-windows-x86_64.exe")
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub tag: String,
    pub notes: String,
    pub asset: String,
    pub url: String,
    pub size: u64,
    pub checksums_url: Option<String>,
    pub sha256: Option<String>,
}

pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    Some((major, minor, patch))
}

pub fn newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Read the release this platform can install out of a GitHub `releases/latest` document.
pub fn find(json: &Value, asset: &str) -> Result<Option<Release>> {
    let tag = json
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or("The release has no tag")?;
    let assets = json
        .get("assets")
        .and_then(Value::as_array)
        .ok_or("The release lists no assets")?;
    let by_name = |name: &str| {
        assets
            .iter()
            .find(|a| a.get("name").and_then(Value::as_str) == Some(name))
    };
    let Some(mine) = by_name(asset) else {
        return Ok(None);
    };
    let url = mine
        .get("browser_download_url")
        .and_then(Value::as_str)
        .ok_or("The asset has no download URL")?;
    Ok(Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        tag: tag.to_string(),
        notes: json
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        asset: asset.to_string(),
        url: url.to_string(),
        size: mine.get("size").and_then(Value::as_u64).unwrap_or(0),
        checksums_url: by_name(CHECKSUMS)
            .and_then(|a| a.get("browser_download_url"))
            .and_then(Value::as_str)
            .map(str::to_string),
        sha256: None,
    }))
}

/// The hex digest for `name` in a `sha256sum` style listing.
pub fn parse_checksum(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let file = parts.next()?.trim_start_matches('*');
        (file == name && hash.len() == 64).then(|| hash.to_ascii_lowercase())
    })
}

fn hex(digest: &[u8]) -> String {
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(600)))
        .user_agent(format!("Ondera/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn get_text(agent: &ureq::Agent, url: &str) -> Result<String> {
    agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Could not reach GitHub: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Could not read the reply from GitHub: {e}"))
}

/// Ask GitHub for the latest release and return it when it is newer than this build.
pub fn check() -> Result<Option<Release>> {
    let Some(asset) = asset_name() else {
        return Ok(None);
    };
    let agent = agent();
    let url = format!("{API}/{REPO}/releases/latest");
    let text = match agent
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(mut response) => response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Could not read the reply from GitHub: {e}"))?,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(e) => return Err(format!("Could not reach GitHub: {e}")),
    };
    let json: Value =
        serde_json::from_str(&text).map_err(|e| format!("GitHub sent an unexpected reply: {e}"))?;
    let Some(mut release) = find(&json, asset)? else {
        return Ok(None);
    };
    if !newer(&release.version, &current_version()) {
        return Ok(None);
    }
    let sums_url = release
        .checksums_url
        .clone()
        .ok_or_else(|| format!("Release {} has no {CHECKSUMS} file", release.tag))?;
    let sums = get_text(&agent, &sums_url)?;
    release.sha256 = Some(
        parse_checksum(&sums, asset)
            .ok_or_else(|| format!("{CHECKSUMS} in release {} lacks {asset}", release.tag))?,
    );
    Ok(Some(release))
}

fn download(agent: &ureq::Agent, release: &Release, to: &Path) -> Result<()> {
    let mut response = agent
        .get(&release.url)
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;
    let mut reader = response.body_mut().with_config().limit(MAX_ASSET).reader();
    let mut file = fs::File::create(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Download failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])
            .map_err(|e| format!("{}: {e}", to.display()))?;
    }
    file.sync_all().map_err(|e| e.to_string())?;
    let got = hex(&hasher.finalize());
    match &release.sha256 {
        Some(expected) if *expected == got => Ok(()),
        Some(_) => Err(format!(
            "The download of {} does not match its published checksum; nothing was changed",
            release.asset
        )),
        None => Err("The release has no checksum for this file; nothing was changed".into()),
    }
}

/// Where this process runs from, resolved through symlinks.
fn current_exe() -> Result<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    fs::canonicalize(&exe).map_err(|e| format!("{}: {e}", exe.display()))
}

#[cfg(target_os = "macos")]
fn bundle_of(exe: &Path) -> Result<PathBuf> {
    let bundle = exe
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|b| b.extension().is_some_and(|e| e == "app"))
        .ok_or_else(|| {
            format!(
                "Updates apply to an installed Ondera.app; this copy runs from {}",
                exe.display()
            )
        })?;
    Ok(bundle.to_path_buf())
}

#[cfg(target_os = "macos")]
fn previous_bundle(bundle: &Path) -> PathBuf {
    bundle.with_file_name(".Ondera-previous.app")
}

#[cfg(target_os = "macos")]
fn run(cmd: &mut std::process::Command, what: &str) -> Result<()> {
    let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{what}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Download, verify and swap the installed copy. Returns what to launch afterwards. The old
/// copy stays in place until the new one is fully unpacked and verified.
pub fn install(release: &Release) -> Result<PathBuf> {
    let exe = current_exe()?;
    let agent = agent();
    #[cfg(target_os = "macos")]
    {
        let bundle = bundle_of(&exe)?;
        let parent = bundle.parent().ok_or("The app has no parent folder")?;
        let staging = parent.join(format!(".ondera-update-{}", std::process::id()));
        let _ = fs::remove_dir_all(&staging);
        fs::create_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
        let result = (|| -> Result<()> {
            let zip = staging.join(&release.asset);
            download(&agent, release, &zip)?;
            let unpacked = staging.join("unpacked");
            run(
                std::process::Command::new("ditto")
                    .args(["-x", "-k"])
                    .arg(&zip)
                    .arg(&unpacked),
                "Could not unpack the update",
            )?;
            let fresh = fs::read_dir(&unpacked)
                .map_err(|e| e.to_string())?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .find(|p| p.extension().is_some_and(|e| e == "app"))
                .ok_or("The update archive holds no application")?;
            if !fresh.join("Contents/MacOS/ondera").is_file() {
                return Err("The update archive is incomplete".into());
            }
            run(
                std::process::Command::new("codesign")
                    .args(["--verify", "--deep", "--strict"])
                    .arg(&fresh),
                "The downloaded app failed signature verification",
            )?;
            let previous = previous_bundle(&bundle);
            let _ = fs::remove_dir_all(&previous);
            fs::rename(&bundle, &previous)
                .map_err(|e| format!("Could not replace {}: {e}", bundle.display()))?;
            if let Err(e) = fs::rename(&fresh, &bundle) {
                let _ = fs::rename(&previous, &bundle);
                return Err(format!("Could not install into {}: {e}", bundle.display()));
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&staging);
        result?;
        Ok(bundle)
    }
    #[cfg(target_os = "windows")]
    {
        let dir = exe.parent().ok_or("The executable has no parent folder")?;
        let fresh = dir.join("ondera-update.exe");
        let old = dir.join("ondera.old.exe");
        download(&agent, release, &fresh)?;
        let _ = fs::remove_file(&old);
        fs::rename(&exe, &old).map_err(|e| format!("Could not replace {}: {e}", exe.display()))?;
        if let Err(e) = fs::rename(&fresh, &exe) {
            let _ = fs::rename(&old, &exe);
            return Err(format!("Could not install into {}: {e}", exe.display()));
        }
        Ok(exe)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = exe.parent().ok_or("The executable has no parent folder")?;
        let fresh = dir.join(".ondera-update");
        download(&agent, release, &fresh)?;
        fs::set_permissions(&fresh, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        fs::rename(&fresh, &exe)
            .map_err(|e| format!("Could not replace {}: {e}", exe.display()))?;
        Ok(exe)
    }
}

/// Remove what a previous update left behind. Called once at startup; errors are ignored
/// because the old copy may still be shutting down.
pub fn cleanup() {
    let Ok(exe) = current_exe() else {
        return;
    };
    #[cfg(target_os = "macos")]
    if let Ok(bundle) = bundle_of(&exe) {
        let _ = fs::remove_dir_all(previous_bundle(&bundle));
    }
    #[cfg(target_os = "windows")]
    if let Some(dir) = exe.parent() {
        let _ = fs::remove_file(dir.join("ondera.old.exe"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(dir) = exe.parent() {
        let _ = fs::remove_file(dir.join(".ondera-update"));
    }
}

/// Start the freshly installed copy. The caller closes this one.
pub fn relaunch(target: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg("-n").arg(target);
        c
    };
    #[cfg(not(target_os = "macos"))]
    let mut cmd = std::process::Command::new(target);
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not relaunch {}: {e}", target.display()))
}

#[derive(Default)]
pub(crate) struct Updates {
    pub checking: Option<mpsc::Receiver<Result<Option<Release>>>>,
    pub manual: bool,
    pub available: Option<Release>,
    pub installing: Option<mpsc::Receiver<Result<PathBuf>>>,
    pub installed: Option<PathBuf>,
    pub show: bool,
}
impl Updates {
    pub fn busy(&self) -> bool {
        self.checking.is_some() || self.installing.is_some()
    }
}

impl Ondera {
    pub(crate) fn check_for_updates(&mut self, manual: bool) {
        if self.updates.busy() {
            return;
        }
        if self.updates.available.is_some() && manual {
            self.updates.show = true;
            return;
        }
        self.updates.manual = manual;
        let (tx, rx) = mpsc::sync_channel(1);
        self.updates.checking = Some(rx);
        if manual {
            self.status = "Checking for updates…".into();
        }
        std::thread::spawn(move || {
            let _ = tx.send(check());
        });
    }
    pub(crate) fn install_update(&mut self) {
        let Some(release) = self.updates.available.clone() else {
            return;
        };
        if self.updates.busy() || self.updates.installed.is_some() {
            return;
        }
        let (tx, rx) = mpsc::sync_channel(1);
        self.updates.installing = Some(rx);
        self.status = format!("Downloading Ondera {}…", release.version);
        std::thread::spawn(move || {
            let _ = tx.send(install(&release));
        });
    }
    pub(crate) fn poll_updates(&mut self) {
        if let Some(result) = self
            .updates
            .checking
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(v) => Some(v),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("Update check stopped".into())),
                Err(_) => None,
            })
        {
            self.updates.checking = None;
            match result {
                Ok(Some(release)) => {
                    self.status = format!("Ondera {} is available", release.version);
                    self.updates.available = Some(release);
                    self.updates.show = true;
                }
                Ok(None) if self.updates.manual => {
                    self.status = format!("Ondera {} is up to date", current_version());
                }
                Ok(None) => {}
                Err(e) if self.updates.manual => self.error = Some(e),
                Err(_) => {}
            }
        }
        if let Some(result) = self
            .updates
            .installing
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(v) => Some(v),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("Update stopped".into())),
                Err(_) => None,
            })
        {
            self.updates.installing = None;
            match result {
                Ok(target) => {
                    self.status = "Update installed".into();
                    self.updates.installed = Some(target);
                    self.updates.show = true;
                }
                Err(e) => {
                    self.status = "Update failed".into();
                    self.updates.show = false;
                    self.error = Some(e);
                }
            }
        }
    }
    /// Title-bar notice, shown once a newer release is known.
    pub(crate) fn update_button(&mut self, ui: &mut egui::Ui) {
        let label = match (&self.updates.available, &self.updates.installed) {
            (_, Some(_)) => "Relaunch to update".to_string(),
            (Some(r), None) => format!("Update to {}", r.version),
            (None, None) => return,
        };
        if text_button(ui, &label, Face::Lit).clicked() {
            self.updates.show = true;
        }
    }
    pub(crate) fn update_dialog(&mut self, ctx: &egui::Context) {
        if !self.updates.show {
            return;
        }
        let Some(release) = self.updates.available.clone() else {
            self.updates.show = false;
            return;
        };
        egui::Modal::new(egui::Id::new("update")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            let title = if self.updates.installed.is_some() {
                format!("Ondera {} is installed", release.version)
            } else {
                format!("Ondera {} is available", release.version)
            };
            ui.label(text(title, FS_PANEL_TITLE, Weight::Bold, INK));
            ui.add_space(6.0);
            let body = if self.updates.installed.is_some() {
                "Relaunch to start using it. Your session is saved first if it has changes."
                    .to_string()
            } else if self.updates.installing.is_some() {
                format!("Downloading {}…", release.asset)
            } else {
                format!(
                    "You have {}. The update downloads from GitHub, is verified, replaces this copy and relaunches.",
                    current_version()
                )
            };
            ui.label(text(body, FS_BODY, Weight::Medium, DIM));
            let notes: Vec<&str> = release
                .notes
                .lines()
                .filter(|l| !l.trim().is_empty())
                .take(8)
                .collect();
            if !notes.is_empty() && self.updates.installed.is_none() {
                ui.add_space(8.0);
                for line in notes {
                    ui.label(text(line, FS_SECONDARY, Weight::Medium, INK_CONTROL));
                }
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if self.updates.installed.is_some() {
                    if text_button(ui, "Relaunch now", Face::Lit).clicked() {
                        self.updates.show = false;
                        self.request(Intent::Relaunch);
                    }
                    if text_button(ui, "Later", Face::Raised).clicked() {
                        self.updates.show = false;
                    }
                } else if self.updates.installing.is_none() {
                    if text_button(ui, "Install and relaunch", Face::Lit).clicked() {
                        self.install_update();
                    }
                    if text_button(ui, "Later", Face::Raised).clicked() {
                        self.updates.show = false;
                    }
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("0.10"), Some((0, 10, 0)));
        assert_eq!(parse_version("v1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(parse_version("nightly"), None);
        assert!(newer("v0.2.0", "0.1.9"));
        assert!(newer("v0.10.0", "0.9.0"));
        assert!(!newer("v0.1.0", "0.1.0"));
        assert!(!newer("garbage", "0.1.0"));
    }

    #[test]
    fn release_document_yields_this_platforms_asset() {
        let json: Value = serde_json::json!({
            "tag_name": "v0.2.0",
            "body": "Notes",
            "assets": [
                {"name": "SHA256SUMS", "browser_download_url": "https://x/SHA256SUMS", "size": 300},
                {"name": "Ondera-macos-arm64.zip", "browser_download_url": "https://x/a.zip", "size": 10},
            ]
        });
        let r = find(&json, "Ondera-macos-arm64.zip").unwrap().unwrap();
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.url, "https://x/a.zip");
        assert_eq!(r.checksums_url.as_deref(), Some("https://x/SHA256SUMS"));
        assert_eq!(r.notes, "Notes");
        assert!(find(&json, "ondera-linux-x86_64").unwrap().is_none());
        assert!(find(&serde_json::json!({}), "x").is_err());
    }

    #[test]
    fn checksum_listing_is_parsed() {
        let sums = format!(
            "{}  Ondera-macos-arm64.zip\n{} *ondera-windows-x86_64.exe\n",
            "a".repeat(64),
            "B".repeat(64)
        );
        assert_eq!(
            parse_checksum(&sums, "Ondera-macos-arm64.zip").as_deref(),
            Some("a".repeat(64).as_str())
        );
        assert_eq!(
            parse_checksum(&sums, "ondera-windows-x86_64.exe").as_deref(),
            Some("b".repeat(64).as_str())
        );
        assert_eq!(parse_checksum(&sums, "other"), None);
        assert_eq!(
            hex(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn this_platform_has_a_release_asset() {
        assert!(asset_name().is_some());
    }
}
