//! The ONLY place name-derived strings may live (see AGENTS.md).
//! When the product name lands, this file changes and nothing else does
//! (besides crate/binary renames in Cargo.toml).

/// CLI binary / brand name.
pub const TOOL_NAME: &str = "envit";
/// Manifest filename at the project root. Committed.
pub const MANIFEST_FILE: &str = "envit.json";
/// Lockfile filename at the project root. Committed.
pub const LOCK_FILE: &str = "envit.lock.json";
/// Context directory inside the project. Gitignored.
pub const CONTEXT_DIR: &str = ".envit";
/// Env var overriding the machine-wide store root (default `~/.envit`).
pub const HOME_ENV: &str = "ENVIT_HOME";
/// Env var that blocks all implicit refresh (PRD §12a kill switches).
pub const NO_REFRESH_ENV: &str = "ENVIT_NO_REFRESH";
/// Env var overriding the global refresh TTL (e.g. "1h", "30m", "7d").
pub const REFRESH_TTL_ENV: &str = "ENVIT_REFRESH_TTL";
