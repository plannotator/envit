//! The git engine (PRD §15, §20 D4): pure gix.
//!
//! Three operations, each bounded, no daemon:
//! - [`ensure_bare`]: open or create the store's bare repo for a remote
//! - [`fetch_ref`]: fetch one ref (branch/tag/sha) into the bare repo,
//!   shallow by default; returns the resolved commit SHA
//! - [`extract_checkout`]: materialize a commit's tree into a directory
//!
//! gix is the sole engine (PRD §20 D4): deterministic by construction and
//! works with no `git` binary installed. A system-git lane is added only if
//! a D4 flip condition fires (private-repo auth fidelity, sparse monorepos).

use std::path::Path;

use crate::Error;

/// Open the bare repo at `path`, creating it if missing.
pub fn ensure_bare(path: &Path) -> Result<gix::Repository, Error> {
    if path.is_dir() {
        Ok(gix::open(path).map_err(|e| Error::Git(e.to_string()))?)
    } else {
        std::fs::create_dir_all(path)?;
        Ok(gix::init_bare(path).map_err(|e| Error::Git(e.to_string()))?)
    }
}

/// What a ref resolved to.
#[derive(Debug)]
pub struct Resolved {
    pub commit: String,
    /// True when the ref matched `refs/tags/*` — tags don't move, so the
    /// freshness system treats them as frozen (PRD §12a defaults).
    pub tag: bool,
}

/// Fetch `wanted` (branch name, tag name, or full hex SHA) from `url` into
/// the bare repo. Shallow (depth 1) unless `full_history`.
pub fn fetch_ref(
    bare: &gix::Repository,
    url: &str,
    wanted: &str,
    full_history: bool,
) -> Result<Resolved, Error> {
    let is_sha = wanted.len() == 40 && wanted.bytes().all(|b| b.is_ascii_hexdigit());

    // Source-only refspecs (like `git fetch <url> <ref>`): objects land in
    // the odb, the ref-map tells us what the remote resolved.
    let refspec = wanted.to_string();

    let remote = bare
        .remote_at(url)
        .map_err(|e| Error::Git(e.to_string()))?
        .with_refspecs([refspec.as_str()], gix::remote::Direction::Fetch)
        .map_err(|e| Error::Git(e.to_string()))?
        // No implicit refs/tags/* — we fetch exactly what was asked.
        .with_fetch_tags(gix::remote::fetch::Tags::None);

    // gix reports "refspec matched nothing" as a plain error; translate it
    // into our typed RefNotFound so the CLI can say something useful.
    let not_found = |e: String| {
        if e.contains("matched any of the") {
            Error::RefNotFound { rref: wanted.to_string(), url: url.to_string() }
        } else {
            Error::Git(e)
        }
    };

    let connection = remote
        .connect(gix::remote::Direction::Fetch)
        .map_err(|e| Error::Git(e.to_string()))?;
    let mut prepare = connection
        .prepare_fetch(gix::progress::Discard, Default::default())
        .map_err(|e| not_found(e.to_string()))?;
    if !full_history {
        prepare = prepare.with_shallow(gix::remote::fetch::Shallow::DepthAtRemote(
            1.try_into().expect("1 is non-zero"),
        ));
    }
    let outcome = prepare
        .receive(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
        .map_err(|e| not_found(e.to_string()))?;

    // Find the mapping for what we asked for.
    for mapping in &outcome.ref_map.mappings {
        match &mapping.remote {
            gix::remote::fetch::refmap::Source::ObjectId(id) => {
                if is_sha {
                    return Ok(Resolved { commit: id.to_string(), tag: false });
                }
            }
            gix::remote::fetch::refmap::Source::Ref(r) => {
                let (name, target, peeled) = r.unpack();
                let matches = name == wanted
                    || name == format!("refs/heads/{wanted}")
                    || name == format!("refs/tags/{wanted}");
                if matches {
                    let id = peeled.or(target).ok_or_else(|| {
                        Error::Git(format!("remote ref '{name}' has no target"))
                    })?;
                    return Ok(Resolved {
                        commit: id.to_string(),
                        tag: name.starts_with(b"refs/tags/" as &[u8]),
                    });
                }
            }
        }
    }
    Err(Error::RefNotFound { rref: wanted.to_string(), url: url.to_string() })
}

/// Bytes/files written by a checkout — free from gix's own accounting.
#[derive(Debug, Clone, Copy)]
pub struct CheckoutStats {
    pub files: usize,
    pub bytes: u64,
}

/// Materialize the tree of `commit_sha` from the bare repo into `dest`.
/// `dest` must not exist; the caller owns temp-dir + atomic-rename strategy.
pub fn extract_checkout(
    bare: &gix::Repository,
    commit_sha: &str,
    dest: &Path,
) -> Result<CheckoutStats, Error> {
    let id = gix::ObjectId::from_hex(commit_sha.as_bytes())
        .map_err(|e| Error::Git(format!("bad sha '{commit_sha}': {e}")))?;
    let commit = bare
        .find_object(id)
        .map_err(|e| Error::Git(e.to_string()))?
        .try_into_commit()
        .map_err(|e| Error::Git(e.to_string()))?;
    let tree_id = commit.tree_id().map_err(|e| Error::Git(e.to_string()))?;

    let mut index = bare
        .index_from_tree(&tree_id)
        .map_err(|e| Error::Git(e.to_string()))?;

    std::fs::create_dir_all(dest)?;
    let objects = bare
        .objects
        .clone()
        .into_arc()
        .map_err(|e| Error::Git(e.to_string()))?;
    let options = gix::worktree::state::checkout::Options {
        fs: gix::fs::Capabilities::probe(dest),
        ..Default::default()
    };
    let outcome = gix::worktree::state::checkout(
        &mut index,
        dest,
        objects,
        &gix::progress::Discard,
        &gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
        options,
    )
    .map_err(|e| Error::Git(e.to_string()))?;
    if !outcome.errors.is_empty() {
        return Err(Error::Git(format!(
            "checkout of {commit_sha} had {} file error(s); first: {}",
            outcome.errors.len(),
            outcome.errors[0].path
        )));
    }
    Ok(CheckoutStats { files: outcome.files_updated, bytes: outcome.bytes_written })
}
