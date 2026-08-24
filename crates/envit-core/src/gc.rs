//! `envit gc` (PRD §8, §11): delete store checkouts no project references.
//!
//! Live = the union of every registered project's lockfile. Bare repos are
//! never touched (history is cheap; checkouts are the bulk). Never runs
//! automatically.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use crate::lockfile::Lockfile;
use crate::store::Store;
use crate::Error;

/// Checkouts younger than this are skipped: they may belong to a sync that
/// hasn't written its lockfile yet.
const MIN_AGE: Duration = Duration::from_secs(15 * 60);

pub struct GcReport {
    /// (path, bytes) actually or would-be removed.
    pub removed: Vec<(PathBuf, u64)>,
    pub kept: usize,
    /// Registered projects that no longer exist and were dropped.
    pub pruned_projects: usize,
}

impl GcReport {
    pub fn freed_bytes(&self) -> u64 {
        self.removed.iter().map(|(_, b)| b).sum()
    }
}

pub fn gc(store: &Store, dry_run: bool) -> Result<GcReport, Error> {
    // 1. Live projects and their referenced checkouts.
    let mut live = HashSet::new();
    let mut kept_projects = Vec::new();
    let projects = store.projects()?;
    let total_projects = projects.len();
    for root in projects {
        let Ok(Some(lock)) = Lockfile::load(&root) else {
            // Project gone (or never locked): it holds no references.
            // Only prune the registry entry if the directory itself is gone.
            if root.is_dir() {
                kept_projects.push(root);
            }
            continue;
        };
        for repo in lock.repos().iter().chain(lock.skill_sources()) {
            live.insert(store.checkout_dir(&repo.source, &repo.commit, &[])?);
        }
        kept_projects.push(root);
    }
    let pruned_projects = total_projects - kept_projects.len();
    if pruned_projects > 0 && !dry_run {
        store.write_projects(&kept_projects)?;
    }

    // 2. Walk actual checkouts; each leaf keyed by a commit SHA.
    let mut removed = Vec::new();
    let mut kept = 0;
    for dir in store.all_checkouts()? {
        if live.contains(&dir) {
            kept += 1;
            continue;
        }
        let too_fresh = dir
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|m| m.elapsed().unwrap_or_default() < MIN_AGE);
        if too_fresh {
            kept += 1; // possibly a sync in flight that hasn't locked yet
            continue;
        }
        let bytes = tree_size(&dir);
        if !dry_run {
            std::fs::remove_dir_all(&dir)?;
        }
        removed.push((dir, bytes));
    }

    // Crash-orphaned extraction temp dirs: dead weight, safe to sweep
    // past the same age window.
    for tmp in store.stale_tmp_dirs(MIN_AGE)? {
        let bytes = tree_size(&tmp);
        if !dry_run {
            std::fs::remove_dir_all(&tmp)?;
        }
        removed.push((tmp, bytes));
    }

    Ok(GcReport { removed, kept, pruned_projects })
}

fn tree_size(dir: &std::path::Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                stack.push(e.path());
            } else if let Ok(m) = e.metadata() {
                total += m.len();
            }
        }
    }
    total
}
