//! The lockfile (`envit.lock.json`): canonical, machine-emitted JSON
//! (PRD §10, §20 D6).
//!
//! Manifest = intent ("track main"). Lockfile = fact ("main was 3f2a9c1
//! when materialized"). Emission is deterministic: entries sorted by name,
//! stable field order, 2-space pretty print — identical state always
//! produces identical bytes.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{ident, Error};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedRepo {
    pub name: String,
    /// Fully expanded URL (never a shorthand) — reproducible on machines
    /// without the author's forge aliases (PRD §12b).
    pub source: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    /// Full commit SHA this repo is materialized at.
    pub commit: String,
    /// True when the ref resolved via `refs/tags/*` — tags never go stale.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tag: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LockDoc {
    version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    repos: Vec<LockedRepo>,
    #[serde(rename = "skillSources", default, skip_serializing_if = "Vec::is_empty")]
    skill_sources: Vec<LockedRepo>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Lockfile {
    repos: Vec<LockedRepo>,
    skill_sources: Vec<LockedRepo>,
}

impl Lockfile {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load from `root`. A missing lockfile is `None`, not an error —
    /// a project that has never synced simply has no lock yet.
    pub fn load(root: &Path) -> Result<Option<Self>, Error> {
        let path = root.join(ident::LOCK_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::Io(e)),
        };
        let doc: LockDoc = serde_json::from_str(&text)
            .map_err(|e| Error::LockfileShape(e.to_string()))?;
        Ok(Some(Self { repos: doc.repos, skill_sources: doc.skill_sources }))
    }

    pub fn get(&self, name: &str) -> Option<&LockedRepo> {
        self.repos.iter().find(|r| r.name == name)
    }

    pub fn repos(&self) -> &[LockedRepo] {
        &self.repos
    }

    pub fn get_skill_source(&self, name: &str) -> Option<&LockedRepo> {
        self.skill_sources.iter().find(|r| r.name == name)
    }

    pub fn skill_sources(&self) -> &[LockedRepo] {
        &self.skill_sources
    }

    pub fn set_skill_source(&mut self, entry: LockedRepo) {
        match self.skill_sources.iter_mut().find(|r| r.name == entry.name) {
            Some(existing) => *existing = entry,
            None => self.skill_sources.push(entry),
        }
    }

    pub fn retain_skill_sources(&mut self, names: &[String]) {
        self.skill_sources.retain(|r| names.contains(&r.name));
    }

    /// Insert or replace the entry with this name.
    pub fn set(&mut self, repo: LockedRepo) {
        match self.repos.iter_mut().find(|r| r.name == repo.name) {
            Some(existing) => *existing = repo,
            None => self.repos.push(repo),
        }
    }

    /// Drop entries whose names are not in `names` (repos removed from the
    /// manifest disappear from the lock on the next sync).
    pub fn retain_named(&mut self, names: &[String]) {
        self.repos.retain(|r| names.contains(&r.name));
    }

    /// Write canonical form to `root`.
    pub fn save(&self, root: &Path) -> Result<(), Error> {
        std::fs::write(root.join(ident::LOCK_FILE), self.to_canonical_string()?)?;
        Ok(())
    }

    fn to_canonical_string(&self) -> Result<String, Error> {
        let mut doc = LockDoc {
            version: 1,
            repos: self.repos.clone(),
            skill_sources: self.skill_sources.clone(),
        };
        doc.repos.sort_by(|a, b| a.name.cmp(&b.name));
        doc.skill_sources.sort_by(|a, b| a.name.cmp(&b.name));
        let mut text = serde_json::to_string_pretty(&doc)?;
        text.push('\n');
        Ok(text)
    }
}
