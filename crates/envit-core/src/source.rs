//! Source shorthand expansion (PRD §12b). Shorthands are pure URL sugar;
//! the manifest keeps what the user wrote, the lockfile stores the
//! expanded URL.

use crate::Error;

/// Expand a source (`github:owner/repo`, …, or a full `https://` URL) to a
/// canonical fetch URL. Errors on anything unrecognizable.
pub fn expand(src: &str) -> Result<String, Error> {
    if let Some(path) = src.strip_prefix("github:") {
        exact_segments("github.com", path, 2, src)
    } else if let Some(path) = src.strip_prefix("gitlab:") {
        // GitLab allows nested groups: gitlab:group/sub/repo
        min_segments("gitlab.com", path, 2, src)
    } else if let Some(path) = src.strip_prefix("bitbucket:") {
        exact_segments("bitbucket.org", path, 2, src)
    } else if let Some(path) = src.strip_prefix("codeberg:") {
        exact_segments("codeberg.org", path, 2, src)
    } else if let Some(path) = src.strip_prefix("sourcehut:") {
        // sourcehut:~user/repo → https://git.sr.ht/~user/repo (no .git)
        let ok = path.starts_with('~')
            && path.split('/').count() == 2
            && path.split('/').all(|s| !s.is_empty());
        if ok {
            Ok(format!("https://git.sr.ht/{path}"))
        } else {
            Err(Error::BadSource(src.to_string()))
        }
    } else if (src.starts_with("https://") && src.len() > "https://".len())
        || (src.starts_with("file://") && src.len() > "file://".len())
    {
        Ok(src.to_string())
    } else if !src.contains(':') {
        // Bare `owner/repo` means GitHub — the ecosystem norm. Exactly two
        // non-empty segments; anything else is a typo worth rejecting.
        exact_segments("github.com", src, 2, src)
    } else {
        Err(Error::BadSource(src.to_string()))
    }
}

/// Default repo name: the last path segment, minus a `.git` suffix.
pub fn default_name(src: &str) -> Option<String> {
    let tail = src.rsplit('/').next()?;
    // Handles the no-slash case defensively; shorthands always contain '/'.
    let tail = tail.rsplit(':').next()?;
    let name = tail.strip_suffix(".git").unwrap_or(tail);
    if name.is_empty() { None } else { Some(name.to_string()) }
}

fn exact_segments(host: &str, path: &str, n: usize, orig: &str) -> Result<String, Error> {
    let segs: Vec<&str> = path.split('/').collect();
    if segs.len() == n && segs.iter().all(|s| !s.is_empty()) {
        Ok(format!("https://{host}/{path}.git"))
    } else {
        Err(Error::BadSource(orig.to_string()))
    }
}

fn min_segments(host: &str, path: &str, n: usize, orig: &str) -> Result<String, Error> {
    let segs: Vec<&str> = path.split('/').collect();
    if segs.len() >= n && segs.iter().all(|s| !s.is_empty()) {
        Ok(format!("https://{host}/{path}.git"))
    } else {
        Err(Error::BadSource(orig.to_string()))
    }
}
