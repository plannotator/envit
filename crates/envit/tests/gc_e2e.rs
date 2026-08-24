//! e2e: gc removes only unreferenced checkouts, respects every project's
//! lock, prunes dead projects, and dry-run touches nothing.

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

/// Make every checkout in the store look old enough for gc's safety window.
fn age_checkouts(store: &Path) {
    let mut stack = vec![store.join("store/checkouts")];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                Command::new("touch")
                    .args(["-m", "-t", "202501010000"])
                    .arg(&p)
                    .output()
                    .unwrap();
                stack.push(p);
            }
        }
    }
}

#[test]
fn gc_keeps_referenced_removes_orphaned_prunes_dead() {
    let upstream = tempfile::tempdir().unwrap();
    git(upstream.path(), &["init", "-b", "main", "."]);
    std::fs::write(upstream.path().join("lib.rs"), "v1\n").unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "one"]);
    let first = git(upstream.path(), &["rev-parse", "HEAD"]);

    let url = format!("file://{}", upstream.path().display());
    let store = tempfile::tempdir().unwrap();

    // Project A syncs v1; project B syncs v1 too (shares the checkout).
    let a = tempfile::tempdir().unwrap();
    ok(&envit(a.path(), store.path(), &["init"]));
    ok(&envit(a.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));
    ok(&envit(a.path(), store.path(), &["sync"]));
    let b = tempfile::tempdir().unwrap();
    ok(&envit(b.path(), store.path(), &["init"]));
    ok(&envit(b.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));
    ok(&envit(b.path(), store.path(), &["sync"]));

    // Upstream moves; only A updates. Store now holds v1 (B) and v2 (A).
    std::fs::write(upstream.path().join("lib.rs"), "v2\n").unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "two"]);
    ok(&envit(a.path(), store.path(), &["update"]));

    age_checkouts(store.path());

    // Both checkouts referenced → gc removes nothing.
    let out = ok(&envit(a.path(), store.path(), &["gc"]));
    assert!(out.contains("removed 0 checkout(s)"), "{out}");
    assert!(out.contains("2 kept"), "{out}");

    // B moves forward too → v1 becomes orphaned.
    ok(&envit(b.path(), store.path(), &["update"]));
    age_checkouts(store.path());

    // Dry run: reports v1, deletes nothing.
    let out = ok(&envit(a.path(), store.path(), &["gc", "--dry-run"]));
    assert!(out.contains("would remove 1 checkout(s)"), "{out}");
    assert!(out.contains(&first), "{out}");
    let out2 = ok(&envit(a.path(), store.path(), &["gc", "--dry-run"]));
    assert!(out2.contains("would remove 1 checkout(s)"), "dry-run must not delete: {out2}");

    // Real gc: v1 gone, v2 kept, links still healthy.
    let out = ok(&envit(a.path(), store.path(), &["gc"]));
    assert!(out.contains("removed 1 checkout(s)"), "{out}");
    assert_eq!(
        std::fs::read_to_string(a.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "v2\n"
    );
    assert_eq!(
        std::fs::read_to_string(b.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "v2\n"
    );

    // A project directory vanishes entirely → registry pruned, its refs die.
    let b_path = b.path().to_path_buf();
    drop(b);
    assert!(!b_path.exists());
    let out = ok(&envit(a.path(), store.path(), &["gc"]));
    assert!(out.contains("1 dead project(s) pruned"), "{out}");
    // v2 still referenced by A, so still 1 kept.
    assert!(out.contains("1 kept"), "{out}");
}
