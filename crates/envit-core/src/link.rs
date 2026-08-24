//! Linking (PRD §12). M1: symlinks (macOS/Linux) with atomic flips.
//! Windows junctions and the reflink/hardlink/copy fallback matrix land
//! in M2.

use std::path::Path;

use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOutcome {
    Created,
    /// Link existed but pointed elsewhere; atomically flipped.
    Updated,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkMode {
    #[default]
    Symlink,
    /// Escape hatch for filesystems where symlinks don't work (network
    /// shares, restrictive container mounts). Loses byte-sharing.
    Copy,
}

/// Point `link` at the store checkout `target` using `mode`. `commit` keys
/// copy-mode freshness (a `<link>.commit` marker file).
pub fn materialize_link(
    link: &Path,
    target: &Path,
    mode: LinkMode,
    commit: &str,
) -> Result<LinkOutcome, Error> {
    match mode {
        LinkMode::Symlink => link_dir(link, target),
        LinkMode::Copy => copy_dir(link, target, commit),
    }
}

/// True when the last materialize_copy for `link` wrote fresh content for
/// `key` (i.e. the marker now equals `key` and content was just copied).
/// Freshness is recorded by materialize_copy in a side channel: the marker
/// write itself. Callers patch files only on fresh copies.
pub fn copy_was_fresh(link: &Path, key: &str) -> bool {
    // materialize_copy sets the marker only after a fresh copy; an
    // unchanged marker means the previous (already patched) copy stands.
    std::fs::read_to_string(fresh_flag_path(link)).ok().as_deref() == Some(key)
}

fn fresh_flag_path(link: &Path) -> std::path::PathBuf {
    let mut name = link.file_name().unwrap_or_default().to_os_string();
    name.push(".fresh");
    link.with_file_name(name)
}

/// Copy `target` to `link` with marker `key`. No-op when the marker
/// already equals `key`. Leaves a `.fresh` flag when it actually copied,
/// so the caller knows to apply patches exactly once.
pub fn materialize_copy(link: &Path, target: &Path, key: &str) -> Result<(), Error> {
    let flag = fresh_flag_path(link);
    let outcome = copy_dir(link, target, key)?;
    if outcome == LinkOutcome::Unchanged {
        let _ = std::fs::remove_file(&flag);
    } else {
        std::fs::write(&flag, key)?;
    }
    Ok(())
}

fn marker_path(link: &Path) -> std::path::PathBuf {
    let mut name = link.file_name().unwrap_or_default().to_os_string();
    name.push(".commit");
    link.with_file_name(name)
}

fn copy_dir(link: &Path, target: &Path, commit: &str) -> Result<LinkOutcome, Error> {
    let marker = marker_path(link);
    if link.is_dir() {
        if std::fs::read_to_string(&marker).ok().as_deref() == Some(commit) {
            return Ok(LinkOutcome::Unchanged);
        }
        if std::fs::read_link(link).is_ok() || !marker.exists() {
            // A symlink (mode switch) is fine to replace; an unmarked real
            // dir is not ours to delete.
            if std::fs::read_link(link).is_err() {
                return Err(Error::LinkObstructed(link.to_path_buf()));
            }
        }
    }
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tmp_name(link);
    let _ = std::fs::remove_dir_all(&tmp);
    copy_tree(target, &tmp)?;
    let existed = link.exists() || std::fs::read_link(link).is_ok();
    if existed {
        if std::fs::read_link(link).is_ok() {
            std::fs::remove_file(link)?;
        } else {
            std::fs::remove_dir_all(link)?;
        }
    }
    std::fs::rename(&tmp, link)?;
    std::fs::write(&marker, commit)?;
    Ok(if existed { LinkOutcome::Updated } else { LinkOutcome::Created })
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ft = entry.file_type()?;
        if ft.is_dir() {
            copy_tree(&from, &to)?;
        } else if ft.is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(std::fs::read_link(&from)?, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Make `link` point at `target`. Existing correct links are left alone;
/// a stale link is replaced atomically (build at temp name, rename over)
/// so a concurrent reader never sees a missing or half-made link.
pub fn link_dir(link: &Path, target: &Path) -> Result<LinkOutcome, Error> {
    if let Ok(current) = std::fs::read_link(link) {
        if current == target {
            return Ok(LinkOutcome::Unchanged);
        }
        let tmp = tmp_name(link);
        let _ = std::fs::remove_file(&tmp);
        symlink_dir(target, &tmp)?;
        std::fs::rename(&tmp, link)?;
        return Ok(LinkOutcome::Updated);
    }
    if link.exists() {
        // A marker-backed dir is a managed copy (copy mode or a patched
        // skill): ours to replace on a mode switch. Anything else is the
        // user's; refuse to destroy it.
        let marker = marker_path(link);
        if marker.exists() {
            std::fs::remove_dir_all(link)?;
            let _ = std::fs::remove_file(&marker);
            let _ = std::fs::remove_file(fresh_flag_path(link));
        } else {
            return Err(Error::LinkObstructed(link.to_path_buf()));
        }
    }
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent)?;
    }
    symlink_dir(target, link)?;
    Ok(LinkOutcome::Created)
}

fn tmp_name(link: &Path) -> std::path::PathBuf {
    let mut name = link.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp-link");
    link.with_file_name(name)
}

#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) -> Result<(), Error> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(windows)]
fn symlink_dir(_target: &Path, _link: &Path) -> Result<(), Error> {
    // M2: directory junctions (no admin required).
    Err(Error::Git("Windows linking lands in M2".to_string()))
}
