use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("not valid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("no manifest found in {} or any parent directory", .0.display())]
    ManifestNotFound(PathBuf),

    #[error("already initialized: {} exists", .0.display())]
    AlreadyInitialized(PathBuf),

    #[error("a repo named '{0}' already exists in the manifest")]
    DuplicateName(String),

    #[error("no repo named '{0}' in the manifest")]
    NameNotFound(String),

    #[error("unrecognized source '{0}' (expected forge:owner/repo or an https:// git URL)")]
    BadSource(String),

    #[error("cannot derive a name from '{0}' — pass --name")]
    NoDefaultName(String),

    #[error("manifest is malformed: {0}")]
    ManifestShape(String),

    #[error("lockfile is malformed: {0}")]
    LockfileShape(String),

    #[error("git: {0}")]
    Git(String),

    #[error("ref '{rref}' not found on {url}")]
    RefNotFound { rref: String, url: String },

    #[error("{} exists and was not installed by envit: remove it, or drop that entry from envit.json, then run `envit sync` again", .0.display())]
    LinkObstructed(std::path::PathBuf),

    #[error("--frozen: repo '{0}' is not in the lockfile (or its manifest entry changed); run `envit sync` without --frozen to update the lock")]
    FrozenDrift(String),

    #[error("--offline: repo '{0}' is not available locally")]
    OfflineMiss(String),

    #[error("repo '{0}' is already pinned")]
    AlreadyPinned(String),

    #[error("repo '{0}' is not pinned")]
    NotPinned(String),

    #[error("repo '{0}' has never been synced — run `envit sync` first, then pin")]
    NeverSynced(String),

    #[error("skill '{skill}' not found in {from} (available: {})", available.join(", "))]
    SkillNotFound { skill: String, from: String, available: Vec<String> },


    #[error("no valid SKILL.md frontmatter (missing 'name') in {}", .0.display())]
    SkillInvalid(std::path::PathBuf),

    #[error("the global manifest is skills-only — repos are project-scope (remove `repos` from {})", .0.display())]
    GlobalRepos(std::path::PathBuf),

    #[error("no global manifest — create one with `envit init -g` (it lives at {})", .0.display())]
    NoGlobalManifest(std::path::PathBuf),
}
