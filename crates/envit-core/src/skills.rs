//! Skills materialization (docs/skills-design.md).
//!
//! A skill is a directory at (repo, commit, subpath) — the store's existing
//! primitive. Sync fetches the source like any repo, then links each picked
//! skill into the cross-client canonical dir `.agents/skills/<name>` and
//! fans out `.claude/skills/<name>` → canonical (Claude Code documents that
//! skill entries may be symlinks).

use std::path::{Path, PathBuf};

use crate::lockfile::LockedRepo;
use crate::manifest::{Invocation, Pick, SkillEntry};
use crate::store::Store;
use crate::sync::SyncOptions;
use crate::{git, link, source, Error};

/// Cross-client canonical project dir (agentskills.io client convention).
pub const CANONICAL_DIR: &str = ".agents/skills";
/// Claude Code's native project dir; entries symlink to canonical.
pub const CLAUDE_DIR: &str = ".claude/skills";

#[derive(Debug)]
pub struct SkillSync {
    pub name: String,
    pub source: String,
    pub commit: String,
    pub path: PathBuf,
}

/// A discovered skill inside a checkout: its directory and spec name.
struct Found {
    name: String,
    dir: PathBuf,
}

/// Materialize one `skills` source. Assumes the caller already resolved
/// and materialized the source's checkout (shared with repo syncing).
pub fn materialize_source(
    entry: &SkillEntry,
    checkout: &Path,
    commit: &str,
    project_root: &Path,
    opts: SyncOptions,
) -> Result<Vec<SkillSync>, Error> {
    let root = match &entry.path {
        Some(p) => checkout.join(p),
        None => checkout.to_path_buf(),
    };
    if !root.is_dir() {
        return Err(Error::SkillNotFound {
            skill: format!("path '{}'", entry.path.clone().unwrap_or_default()),
            from: entry.source.clone(),
            available: Vec::new(),
        });
    }
    let available = discover(&root)?;
    let picked: Vec<(&Found, Option<Invocation>)> = match &entry.pick {
        Pick::All => available.iter().map(|f| (f, entry.invocation)).collect(),
        Pick::Named(items) => {
            let mut sel = Vec::new();
            for want in items {
                let found =
                    available.iter().find(|f| f.name == want.name).ok_or_else(|| {
                        Error::SkillNotFound {
                            skill: want.name.clone(),
                            from: entry.source.clone(),
                            available: available.iter().map(|f| f.name.clone()).collect(),
                        }
                    })?;
                sel.push((found, want.invocation.or(entry.invocation)));
            }
            sel
        }
    };

    let mut out = Vec::new();
    for (f, invocation) in picked {
        let canonical = project_root.join(CANONICAL_DIR).join(&f.name);
        match invocation {
            // No override: link the author's skill untouched.
            None => {
                link::materialize_link(&canonical, &f.dir, opts.link_mode, commit)?;
            }
            // Override: materialize a patched copy so the invocation policy
            // is ours regardless of what the author set — for Claude
            // (SKILL.md frontmatter) and Codex (agents/openai.yaml) both.
            Some(mode) => {
                materialize_patched(&canonical, &f.dir, commit, mode)?;
            }
        }
        // Fan-out: Claude Code reads .claude/skills and follows symlinks.
        let claude = project_root.join(CLAUDE_DIR).join(&f.name);
        link::materialize_link(&claude, &canonical, opts.link_mode, commit)?;
        out.push(SkillSync {
            name: f.name.clone(),
            source: entry.source.clone(),
            commit: commit.to_string(),
            path: f.dir.clone(),
        });
    }
    Ok(out)
}

/// Copy the skill, then rewrite the invocation policy inside the copy:
/// `disable-model-invocation` in SKILL.md (Claude) and
/// `policy.allow_implicit_invocation` in agents/openai.yaml (Codex).
/// A marker file keyed by commit+mode makes re-syncs no-ops.
fn materialize_patched(
    canonical: &Path,
    skill_dir: &Path,
    commit: &str,
    mode: Invocation,
) -> Result<(), Error> {
    let key = format!(
        "{commit}+invocation:{}",
        if mode == Invocation::Explicit { "explicit" } else { "auto" }
    );
    link::materialize_copy(canonical, skill_dir, &key)?;
    if link::copy_was_fresh(canonical, &key) {
        patch_skill_md(&canonical.join("SKILL.md"), mode)?;
        patch_openai_yaml(&canonical.join("agents").join("openai.yaml"), mode)?;
    }
    Ok(())
}

/// Overwrite a file that was copied read-only from the store: lift the
/// write bit, write, then restore read-only.
fn write_over(path: &Path, content: String) -> Result<(), Error> {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            std::fs::set_permissions(path, perms)?;
        }
    }
    std::fs::write(path, content)?;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

/// Set `disable-model-invocation` in the frontmatter, replacing any value
/// the author shipped.
fn patch_skill_md(md: &Path, mode: Invocation) -> Result<(), Error> {
    let text = std::fs::read_to_string(md)?;
    let value = if mode == Invocation::Explicit { "true" } else { "false" };
    let mut out = Vec::new();
    let mut fences = 0;
    let mut written = false;
    for line in text.lines() {
        if line.trim() == "---" && fences < 2 {
            fences += 1;
            if fences == 2 && !written {
                out.push(format!("disable-model-invocation: {value}"));
                written = true;
            }
            out.push(line.to_string());
            continue;
        }
        if fences == 1 && line.trim_start().starts_with("disable-model-invocation:") {
            out.push(format!("disable-model-invocation: {value}"));
            written = true;
            continue;
        }
        out.push(line.to_string());
    }
    write_over(md, out.join("\n") + "\n")?;
    Ok(())
}

/// Set `policy.allow_implicit_invocation` in agents/openai.yaml, creating
/// the file when the author shipped none.
fn patch_openai_yaml(path: &Path, mode: Invocation) -> Result<(), Error> {
    let value = if mode == Invocation::Explicit { "false" } else { "true" };
    let line = format!("  allow_implicit_invocation: {value}");
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            write_over(path, format!("policy:\n{line}\n"))?;
            return Ok(());
        }
        Err(e) => return Err(Error::Io(e)),
    };
    let mut out = Vec::new();
    let mut replaced = false;
    let mut has_policy = false;
    for l in text.lines() {
        if l.trim_start().starts_with("allow_implicit_invocation:") {
            out.push(line.clone());
            replaced = true;
            continue;
        }
        if l.trim() == "policy:" {
            has_policy = true;
        }
        out.push(l.to_string());
    }
    if !replaced {
        if has_policy {
            let idx = out.iter().position(|l| l.trim() == "policy:").unwrap() + 1;
            out.insert(idx, line);
        } else {
            out.push("policy:".to_string());
            out.push(line);
        }
    }
    write_over(path, out.join("\n") + "\n")?;
    Ok(())
}

/// Resolve/fetch a skills source through the same pipeline repos use.
/// Returns (expanded url, commit, checkout path).
pub fn resolve_source(
    entry: &SkillEntry,
    locked: Option<&LockedRepo>,
    store: &Store,
    opts: SyncOptions,
) -> Result<(String, String, PathBuf, bool), Error> {
    let url = source::expand(&entry.source)?;
    let wanted = entry.git_ref.as_deref().unwrap_or("HEAD");

    let reusable = locked
        .filter(|l| l.source == url && l.git_ref == wanted)
        .map(|l| (l.commit.clone(), l.tag));

    if opts.frozen && reusable.is_none() {
        return Err(Error::FrozenDrift(format!("skills:{}", entry.source)));
    }

    if let Some((commit, tag)) = &reusable {
        let checkout = store.checkout_dir(&url, commit, &[])?;
        if checkout.is_dir() {
            return Ok((url, commit.clone(), checkout, *tag));
        }
    }

    // Slow path: single-flight, fetch, materialize (mirrors repo syncing).
    let _lock = store.lock_remote(&url)?;
    if let Some((commit, tag)) = &reusable {
        let checkout = store.checkout_dir(&url, commit, &[])?;
        if checkout.is_dir() {
            return Ok((url, commit.clone(), checkout, *tag));
        }
    }
    if opts.offline {
        return Err(Error::OfflineMiss(format!("skills:{}", entry.source)));
    }
    let bare = git::ensure_bare(&store.bare_dir(&url)?)?;
    let (commit, tag) = match &reusable {
        Some((c, tag)) => {
            let id = gix::ObjectId::from_hex(c.as_bytes()).map_err(|e| Error::Git(e.to_string()))?;
            if bare.find_object(id).is_err() {
                let target = if opts.frozen { c.as_str() } else { wanted };
                let r = git::fetch_ref(&bare, &url, target, false)?;
                store.mark_remote_checked(&url)?;
                (r.commit, r.tag || *tag)
            } else {
                (c.clone(), *tag)
            }
        }
        None => {
            let r = git::fetch_ref(&bare, &url, wanted, false)?;
            store.mark_remote_checked(&url)?;
            (r.commit, r.tag)
        }
    };
    let checkout = store.checkout_dir(&url, &commit, &[])?;
    if !checkout.is_dir() {
        crate::sync::materialize_checkout(&bare, &commit, &checkout)?;
    }
    Ok((url, commit, checkout, tag))
}

/// Find skills in a checkout: root `SKILL.md`, or directories under
/// `skills/` that contain a `SKILL.md` — up to two levels deep, because
/// real repos group skills in category directories
/// (`skills/productivity/grill-me/SKILL.md`). The frontmatter `name` is
/// the skill's identity; real-world repos (including
/// vercel-labs/agent-skills itself) don't always match dir names to it.
fn discover(checkout: &Path) -> Result<Vec<Found>, Error> {
    let mut found = Vec::new();
    let root_md = checkout.join("SKILL.md");
    if root_md.is_file() {
        let name = frontmatter_name(&root_md)?;
        found.push(Found { name, dir: checkout.to_path_buf() });
    }
    let skills_dir = checkout.join("skills");
    if skills_dir.is_dir() {
        collect_skills(&skills_dir, 2, &mut found)?;
    }
    Ok(found)
}

/// A directory with a SKILL.md is a skill; otherwise recurse `depth` more
/// levels. A skill's own subdirectories are never scanned.
fn collect_skills(dir: &Path, depth: u32, found: &mut Vec<Found>) -> Result<(), Error> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let sub = e.path();
        if !sub.is_dir() {
            continue;
        }
        let md = sub.join("SKILL.md");
        if md.is_file() {
            let name = frontmatter_name(&md)?;
            found.push(Found { name, dir: sub });
        } else if depth > 1 {
            collect_skills(&sub, depth - 1, found)?;
        }
    }
    Ok(())
}

/// Minimal frontmatter read: the `name:` key between the `---` fences.
/// (Full YAML is not needed for one flat string field.)
fn frontmatter_name(md: &Path) -> Result<String, Error> {
    let text = std::fs::read_to_string(md)?;
    let mut in_fm = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if in_fm {
                break;
            }
            in_fm = true;
            continue;
        }
        if in_fm && let Some(rest) = trimmed.strip_prefix("name:") {
            return Ok(rest.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    Err(Error::SkillInvalid(md.to_path_buf()))
}
