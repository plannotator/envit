//! e2e: add without --ref (default branch via HEAD) and `envit update`
//! against an upstream that moves. Real binary, real git fixture.

use std::path::Path;
use std::process::{Command, Output};

fn envit(dir: &Path, store: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_envit"))
        .args(args)
        .current_dir(dir)
        .env("ENVIT_HOME", store)
        .output()
        .expect("binary runs")
}

fn ok(out: &Output) -> String {
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn default_ref_add_and_update_follow_the_branch() {
    let upstream = tempfile::tempdir().unwrap();
    git(upstream.path(), &["init", "-b", "main", "."]);
    std::fs::write(upstream.path().join("lib.rs"), "v1\n").unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "one"]);
    let first = git(upstream.path(), &["rev-parse", "HEAD"]);

    let url = format!("file://{}", upstream.path().display());
    let store = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    // Ergonomics: no --ref needed — resolves the remote's default branch.
    ok(&envit(proj.path(), store.path(), &["init"]));
    ok(&envit(proj.path(), store.path(), &["add", &url, "--name", "dep"]));
    let out = ok(&envit(proj.path(), store.path(), &["sync"]));
    assert!(out.contains(&first[..12]), "resolved default branch: {out}");
    assert_eq!(
        std::fs::read_to_string(proj.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "v1\n"
    );

    // Upstream moves.
    std::fs::write(upstream.path().join("lib.rs"), "v2\n").unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "two"]);
    let second = git(upstream.path(), &["rev-parse", "HEAD"]);

    // Plain sync must NOT move (lock reuse).
    let out = ok(&envit(proj.path(), store.path(), &["sync"]));
    assert!(out.contains("fresh"), "sync stays pinned: {out}");
    assert_eq!(
        std::fs::read_to_string(proj.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "v1\n"
    );

    // update moves to the new head and reports old → new.
    let out = ok(&envit(proj.path(), store.path(), &["update"]));
    assert!(out.contains("updated") && out.contains(&second[..12]), "{out}");
    assert!(out.contains("1 repo(s) moved"), "{out}");
    assert_eq!(
        std::fs::read_to_string(proj.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "v2\n"
    );

    // Lock now pins the new commit; old checkout still exists (gc's job later).
    let lock = std::fs::read_to_string(proj.path().join("envit.lock.json")).unwrap();
    assert!(lock.contains(&second) && !lock.contains(&first));

    // update again: nothing to do.
    let out = ok(&envit(proj.path(), store.path(), &["update"]));
    assert!(out.contains("current") && out.contains("0 repo(s) moved"), "{out}");

    // updating an unknown name errors loudly.
    let out = envit(proj.path(), store.path(), &["update", "nope"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("nope"));
}
