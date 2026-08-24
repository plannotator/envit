//! Git engine integration tests: real git fixture repos, real fetches over
//! the file transport, real checkouts. No mocks (AGENTS.md).
//! System `git` is used ONLY to author fixtures — the code under test is
//! pure gix.

use std::path::Path;
use std::process::Command;

use envit_core::{git, Error};

/// Create a real repo with a commit history, a branch, and a tag.
/// Returns (repo_dir, head_sha, tagged_sha).
fn fixture(dir: &Path) -> (String, String) {
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    run(&["init", "-b", "main", "."]);
    std::fs::write(dir.join("README.md"), "v1 readme\n").unwrap();
    std::fs::create_dir(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/lib.rs"), "pub fn one() {}\n").unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "first"]);
    run(&["tag", "v1.0.0"]);
    let tagged = run(&["rev-parse", "HEAD"]);
    std::fs::write(dir.join("src/lib.rs"), "pub fn two() {}\n").unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "second"]);
    let head = run(&["rev-parse", "HEAD"]);
    (head, tagged)
}

#[test]
fn fetch_branch_tag_and_extract_checkout() {
    let upstream = tempfile::tempdir().unwrap();
    let (head, tagged) = fixture(upstream.path());
    let url = format!("file://{}", upstream.path().display());

    let store = tempfile::tempdir().unwrap();
    let bare_path = store.path().join("repo.git");

    // Branch fetch resolves to the branch head.
    let bare = git::ensure_bare(&bare_path).unwrap();
    let r = git::fetch_ref(&bare, &url, "main", false).unwrap();
    assert_eq!(r.commit, head);
    assert!(!r.tag, "branch resolution is not a tag");

    // Tag fetch resolves to the tagged commit (peeled), sharing the bare repo.
    let r_tag = git::fetch_ref(&bare, &url, "v1.0.0", false).unwrap();
    assert_eq!(r_tag.commit, tagged);
    assert!(r_tag.tag, "tag resolution is flagged");

    // Extract the tag's tree and verify actual file content (v1, not v2).
    let co = store.path().join("checkout-v1");
    git::extract_checkout(&bare, &tagged, &co).unwrap();
    assert_eq!(
        std::fs::read_to_string(co.join("src/lib.rs")).unwrap(),
        "pub fn one() {}\n"
    );
    assert_eq!(std::fs::read_to_string(co.join("README.md")).unwrap(), "v1 readme\n");

    // Extract head too — different content from the same bare repo.
    let co2 = store.path().join("checkout-head");
    git::extract_checkout(&bare, &head, &co2).unwrap();
    assert_eq!(
        std::fs::read_to_string(co2.join("src/lib.rs")).unwrap(),
        "pub fn two() {}\n"
    );

    // A ref that doesn't exist fails loudly with the right error.
    match git::fetch_ref(&bare, &url, "does-not-exist", false) {
        Err(Error::RefNotFound { rref, .. }) => assert_eq!(rref, "does-not-exist"),
        other => panic!("expected RefNotFound, got {other:?}"),
    }

    // Reopening an existing bare repo works (second sync path).
    let again = git::ensure_bare(&bare_path).unwrap();
    let r2 = git::fetch_ref(&again, &url, "main", false).unwrap();
    assert_eq!(r2.commit, head);
}
