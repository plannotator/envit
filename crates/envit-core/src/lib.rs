//! # envit-core
//!
//! All product logic for envit (working name — see PRD §19 Q1).
//! The CLI (`envit-cli`) must stay a thin shell over this crate; nothing
//! in here may depend on being invoked from a terminal (PRD §20 D2:
//! library-first).
//!
//! Implemented:
//! - [`ident`]    — the single home for name-derived strings
//! - [`source`]   — forge shorthand expansion (PRD §12b)
//! - [`manifest`] — manifest load / comment-preserving edit / save (PRD §9)
//! - [`project`]  — init, root discovery
//!
//! Next (M1, per PRD §11–§12a): `lockfile`, `store`, `git` (gix engine, D4),
//! `link`, `sync`, `freshness`.

mod error;

pub mod gc;
pub mod git;
pub mod ident;
pub mod link;
pub mod lockfile;
pub mod manifest;
pub mod project;
pub mod skills;
pub mod source;
pub mod store;
pub mod sync;

pub use error::Error;
