//! Project-level operations: init, root discovery.

use std::path::{Path, PathBuf};

use crate::{ident, Error};

pub struct InitReport {
    pub manifest_path: PathBuf,
    pub gitignore_updated: bool,
}

/// Create a fresh manifest in `dir`. Adds the context dir to `.gitignore`
/// if one exists (PRD §8). Errors if a manifest is already there.
pub fn init(dir: &Path) -> Result<InitReport, Error> {
    let manifest_path = dir.join(ident::MANIFEST_FILE);
    if manifest_path.exists() {
        return Err(Error::AlreadyInitialized(manifest_path));
    }
    let content = "{\n  \"repos\": []\n}\n".to_string();
    std::fs::write(&manifest_path, content)?;

    let gitignore_updated = ensure_gitignored(dir)?;
    Ok(InitReport { manifest_path, gitignore_updated })
}

/// Walk up from `start` to find the directory containing a manifest.
pub fn find_root(start: &Path) -> Result<PathBuf, Error> {
    let mut dir = start;
    loop {
        if dir.join(ident::MANIFEST_FILE).is_file() {
            return Ok(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return Err(Error::ManifestNotFound(start.to_path_buf())),
        }
    }
}

/// Append `<context-dir>/` to `.gitignore` if the file exists and doesn't
/// mention it yet. Never creates a `.gitignore`. Returns true if updated.
fn ensure_gitignored(dir: &Path) -> Result<bool, Error> {
    let path = dir.join(".gitignore");
    if !path.is_file() {
        return Ok(false);
    }
    let entry = format!("{}/", ident::CONTEXT_DIR);
    let text = std::fs::read_to_string(&path)?;
    let already = text
        .lines()
        .map(str::trim)
        .any(|l| l == entry || l == ident::CONTEXT_DIR);
    if already {
        return Ok(false);
    }
    let mut out = text;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&entry);
    out.push('\n');
    std::fs::write(&path, out)?;
    Ok(true)
}
