//! The machine-wide store (PRD §11).
//!
//! ```text
//! ~/.envit/
//! ├── store/
//! │   ├── git/<host>/<path>.git            # one bare repo per remote
//! │   └── checkouts/<host>/<path>/<key>/   # read-only tree per commit
//! └── projects.json                        # registry of synced projects (gc)
//! ```
//!
//! Checkout keys are commit SHAs — `<sha>` for a full tree,
//! `<sha>-sparse-<hash>` for a sparse one — so a moving branch can never
//! silently change what a project sees.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::{ident, Error};

pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Open the store at an explicit root (tests, `--store` overrides).
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Open the default store: `$ENVIT_HOME`, else `~/.envit`.
    pub fn open_default() -> Result<Self, Error> {
        let root = match std::env::var_os(ident::HOME_ENV) {
            Some(v) if !v.is_empty() => PathBuf::from(v),
            _ => home_dir()
                .ok_or_else(|| {
                    Error::Io(std::io::Error::other(
                        "cannot determine home directory; set ENVIT_HOME",
                    ))
                })?
                .join(format!(".{}", ident::TOOL_NAME)),
        };
        Ok(Self::at(root))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Bare repo directory for an expanded https URL.
    pub fn bare_dir(&self, url: &str) -> Result<PathBuf, Error> {
        let (host, path) = url_parts(url)?;
        Ok(self.root.join("store").join("git").join(host).join(format!("{path}.git")))
    }

    /// Checkout directory for (url, commit, optional sparse paths).
    pub fn checkout_dir(
        &self,
        url: &str,
        commit: &str,
        sparse: &[String],
    ) -> Result<PathBuf, Error> {
        let (host, path) = url_parts(url)?;
        let key = if sparse.is_empty() {
            commit.to_string()
        } else {
            let mut paths: Vec<&str> = sparse.iter().map(String::as_str).collect();
            paths.sort_unstable();
            format!("{commit}-sparse-{:016x}", fnv1a(paths.join("\n").as_bytes()))
        };
        Ok(self.root.join("store").join("checkouts").join(host).join(path).join(key))
    }

    /// Record a project root in the registry (used by `gc`). Idempotent.
    /// Returns true if newly added.
    pub fn register_project(&self, project_root: &Path) -> Result<bool, Error> {
        let project = std::fs::canonicalize(project_root)?;
        let project = project.to_string_lossy().into_owned();

        let mut roots = self.projects()?;
        if roots.iter().any(|r| r.to_string_lossy() == project) {
            return Ok(false);
        }
        roots.push(PathBuf::from(project));
        self.write_projects(&roots)?;
        Ok(true)
    }

    /// Take the machine-wide exclusive lock for one remote. Blocks until
    /// acquired; released when the returned `File` drops. This is the
    /// single-flight guarantee (PRD §12a): N concurrent syncs needing the
    /// same remote do the network work exactly once.
    pub fn lock_remote(&self, url: &str) -> Result<std::fs::File, Error> {
        // std's File::lock (stable since Rust 1.89) — no crate needed.
        let dir = self.root.join("locks");
        std::fs::create_dir_all(&dir)?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(format!("{:016x}.lock", fnv1a(url.as_bytes()))))?;
        file.lock()?;
        Ok(file)
    }

    /// Record "we just asked this remote about its refs" (TTL freshness,
    /// PRD §12a). The marker is a file whose mtime is the timestamp.
    pub fn mark_remote_checked(&self, url: &str) -> Result<(), Error> {
        let path = self.remote_marker(url)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, b"")?;
        Ok(())
    }

    /// How long since this remote was last checked. `None` = never.
    pub fn remote_check_age(&self, url: &str) -> Option<std::time::Duration> {
        self.remote_marker(url)
            .ok()?
            .metadata()
            .ok()?
            .modified()
            .ok()?
            .elapsed()
            .ok()
    }

    fn remote_marker(&self, url: &str) -> Result<PathBuf, Error> {
        Ok(self
            .root
            .join("checked")
            .join(format!("{:016x}", fnv1a(url.as_bytes()))))
    }

    /// Rewrite the project registry to exactly `roots`.
    pub fn write_projects(&self, roots: &[PathBuf]) -> Result<(), Error> {
        let doc = json!({
            "roots": roots.iter().map(|r| r.to_string_lossy()).collect::<Vec<_>>()
        });
        std::fs::create_dir_all(&self.root)?;
        let mut text = serde_json::to_string_pretty(&doc)?;
        text.push('\n');
        std::fs::write(self.root.join("projects.json"), text)?;
        Ok(())
    }

    /// Crash-orphaned `.tmp-*` extraction dirs older than `min_age`.
    /// A live extraction is seconds long; anything old is dead weight.
    pub fn stale_tmp_dirs(&self, min_age: std::time::Duration) -> Result<Vec<PathBuf>, Error> {
        let mut found = Vec::new();
        let mut stack = vec![self.root.join("store").join("checkouts")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let path = e.path();
                if !path.is_dir() {
                    continue;
                }
                if e.file_name().to_string_lossy().starts_with(".tmp-") {
                    let old = path
                        .metadata()
                        .and_then(|m| m.modified())
                        .is_ok_and(|m| m.elapsed().unwrap_or_default() > min_age);
                    if old {
                        found.push(path);
                    }
                } else {
                    stack.push(path);
                }
            }
        }
        Ok(found)
    }

    /// Every checkout directory in the store (leaf dirs keyed by SHA).
    pub fn all_checkouts(&self) -> Result<Vec<PathBuf>, Error> {
        fn is_checkout_key(name: &str) -> bool {
            let sha = name.split("-sparse-").next().unwrap_or(name);
            sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())
        }
        let mut found = Vec::new();
        let root = self.root.join("store").join("checkouts");
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let path = e.path();
                if !path.is_dir() {
                    continue;
                }
                let name = e.file_name().to_string_lossy().into_owned();
                if is_checkout_key(&name) {
                    found.push(path);
                } else if !name.starts_with(".tmp-") {
                    stack.push(path);
                }
            }
        }
        Ok(found)
    }

    /// All registered project roots.
    pub fn projects(&self) -> Result<Vec<PathBuf>, Error> {
        let reg_path = self.root.join("projects.json");
        let text = match std::fs::read_to_string(&reg_path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::Io(e)),
        };
        let doc: Value = serde_json::from_str(&text)?;
        Ok(doc
            .get("roots")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(PathBuf::from).collect())
            .unwrap_or_default())
    }
}

/// `https://host/some/path[.git]` → `(host, some/path)`;
/// `file:///abs/path[.git]` → `("local", abs/path)`.
fn url_parts(url: &str) -> Result<(&str, &str), Error> {
    if let Some(path) = url.strip_prefix("file://") {
        let path = path.trim_start_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        let path = path.trim_end_matches('/');
        if path.is_empty() {
            return Err(Error::BadSource(url.to_string()));
        }
        return Ok(("local", path));
    }
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| Error::BadSource(url.to_string()))?;
    let (host, path) = rest
        .split_once('/')
        .ok_or_else(|| Error::BadSource(url.to_string()))?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let path = path.trim_end_matches('/');
    if host.is_empty() || path.is_empty() {
        return Err(Error::BadSource(url.to_string()));
    }
    Ok((host, path))
}

/// FNV-1a: tiny, dependency-free, stable across releases (unlike
/// `DefaultHasher`, which is explicitly unstable and must never key
/// on-disk paths).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub(crate) fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}
