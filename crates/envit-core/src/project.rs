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

const NOTE_BEGIN: &str = "<!-- envit:begin -->";
const NOTE_END: &str = "<!-- envit:end -->";

/// The note envit maintains inside a project's `AGENTS.md` / `CLAUDE.md`.
fn agent_note() -> String {
    format!(
        "{NOTE_BEGIN}\n\
## envit: repo context\n\
\n\
Additional context is available in `{dir}/`. The source of each external\n\
repository declared in `{manifest}` is at `{dir}/repos/<name>/`, read-only,\n\
at a pinned commit. Read `{dir}/AGENTS.md` for the inventory.\n\
\n\
When you work with a dependency, read its actual code there instead of\n\
guessing from memory. The entries are symlinks: use `rg --follow` or\n\
`fd -L` when you search them.\n\
{NOTE_END}\n",
        dir = ident::CONTEXT_DIR,
        manifest = ident::MANIFEST_FILE,
    )
}

/// Add or refresh the envit note at the bottom of `AGENTS.md` and
/// `CLAUDE.md` when those files exist in `root`. Never creates them.
/// Idempotent: the fenced block is replaced in place, never duplicated.
/// Returns the filenames that were written.
pub fn inject_agent_notes(root: &Path) -> Result<Vec<String>, Error> {
    let note = agent_note();
    let mut written = Vec::new();
    for name in ["AGENTS.md", "CLAUDE.md"] {
        let path = root.join(name);
        if !path.is_file() || std::fs::read_link(&path).is_ok() {
            continue; // absent, or a symlink (e.g. CLAUDE.md -> AGENTS.md)
        }
        let text = std::fs::read_to_string(&path)?;
        let updated = match (text.find(NOTE_BEGIN), text.find(NOTE_END)) {
            (Some(b), Some(e)) if e > b => {
                let end = e + NOTE_END.len();
                let tail = text[end..].trim_start_matches('\n');
                format!("{}{}{}", &text[..b], note, if tail.is_empty() { String::new() } else { format!("\n{tail}") })
            }
            _ => {
                let mut s = text.clone();
                if !s.is_empty() && !s.ends_with('\n') {
                    s.push('\n');
                }
                if !s.is_empty() {
                    s.push('\n');
                }
                s.push_str(&note);
                s
            }
        };
        if updated != text {
            std::fs::write(&path, updated)?;
            written.push(name.to_string());
        }
    }
    Ok(written)
}

