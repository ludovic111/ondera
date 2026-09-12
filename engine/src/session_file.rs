//! Cross-process ownership shared by the desktop and file-mode control clients.
use crate::Result;
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
};

/// A persistent sibling inode carries an advisory lock across atomic session-file
/// replacement. Never unlink it: another process could otherwise lock a different
/// inode while an existing owner still holds this one. Clones share one lease so a
/// background save keeps ownership even when its window closes.
#[derive(Clone)]
pub struct SessionFileLock {
    lease: Arc<Lease>,
}
struct Lease {
    file: File,
    path: PathBuf,
}
impl SessionFileLock {
    pub fn path(&self) -> &Path {
        &self.lease.path
    }
    pub fn resolve(path: &Path) -> Result<PathBuf> {
        if path.exists() {
            return std::fs::canonicalize(path).map_err(|e| e.to_string());
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = path.file_name().ok_or("Session path needs a filename")?;
        Ok(std::fs::canonicalize(parent)
            .map_err(|e| e.to_string())?
            .join(name))
    }
    /// Reuse only a lease supplied by this document's owner; other windows and
    /// processes must acquire their own and receive a busy error.
    pub fn acquire_or_reuse(path: &Path, current: Option<&Self>) -> Result<Self> {
        let path = Self::resolve(path)?;
        if let Some(current) = current.filter(|lock| lock.path() == path) {
            return Ok(current.clone());
        }
        Self::acquire(&path)
    }
    pub fn acquire(path: &Path) -> Result<Self> {
        let path = Self::resolve(path)?;
        let name = path.file_name().ok_or("Session path needs a filename")?;
        let mut lock_name = std::ffi::OsString::from(".");
        lock_name.push(name);
        lock_name.push(".ondera-lock");
        let lock_path = path.with_file_name(lock_name);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(&lock_path)
            .map_err(|e| format!("Cannot lock session {}: {e}", path.display()))?;
        file.try_lock_exclusive().map_err(|e| format!("Session {} is already in use by another Ondera window or file-mode client, or could not be locked ({e}). Close that owner or use --live for shared editing.", path.display()))?;
        Ok(Self {
            lease: Arc::new(Lease { file, path }),
        })
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}
