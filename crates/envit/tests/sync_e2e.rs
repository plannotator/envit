//! THE M1 test (PRD §17): two projects share one repo through the store
//! with zero duplicated bytes. Real binary, real git fixture, real store,
//! real symlinks. No mocks.

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

fn git_fixture(dir: &Path) -> String {
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
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    run(&["init", "-b", "main", "."]);
    std::fs::write(dir.join("lib.rs"), "pub fn hello() {}\n").unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "one"]);
    run(&["rev-parse", "HEAD"])
}

#[test]
fn two_projects_share_one_checkout() {
    let upstream = tempfile::tempdir().unwrap();
    let head = git_fixture(upstream.path());
    let url = format!("file://{}", upstream.path().display());

    let store = tempfile::tempdir().unwrap();
    let proj_a = tempfile::tempdir().unwrap();
    let proj_b = tempfile::tempdir().unwrap();

    // Project A: init → add → sync (network path).
    ok(&envit(proj_a.path(), store.path(), &["init"]));
    ok(&envit(proj_a.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));
    let out = ok(&envit(proj_a.path(), store.path(), &["sync"]));
    assert!(out.contains("fetched"), "first sync fetches: {out}");

    // The agent-facing surface exists and the link resolves to real content.
    let link_a = proj_a.path().join(".envit/repos/dep");
    assert_eq!(
        std::fs::read_to_string(link_a.join("lib.rs")).unwrap(),
        "pub fn hello() {}\n"
    );
    assert!(proj_a.path().join(".envit/AGENTS.md").is_file());
    assert_eq!(
        std::fs::read_link(proj_a.path().join(".envit/CLAUDE.md")).unwrap(),
        std::path::PathBuf::from("AGENTS.md"),
        "CLAUDE.md symlinks to AGENTS.md"
    );

    // Lock records the exact commit.
    let lock = std::fs::read_to_string(proj_a.path().join("envit.lock.json")).unwrap();
    assert!(lock.contains(&head), "lock must pin the fixture head");

    // Checkouts are read-only: writing through the link fails.
    assert!(
        std::fs::OpenOptions::new()
            .write(true)
            .open(link_a.join("lib.rs"))
            .is_err(),
        "context files must be read-only"
    );

    // Project B: same repo — must link the existing checkout, not refetch.
    ok(&envit(proj_b.path(), store.path(), &["init"]));
    ok(&envit(proj_b.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));
    let out_b = ok(&envit(proj_b.path(), store.path(), &["sync"]));
    assert!(out_b.contains("fetched"), "B resolves the branch itself (no shared lock): {out_b}");

    // THE M1 criterion: both links resolve to the SAME store directory.
    let target_a = std::fs::read_link(&link_a).unwrap();
    let target_b = std::fs::read_link(proj_b.path().join(".envit/repos/dep")).unwrap();
    assert_eq!(target_a, target_b, "same bytes on disk, zero duplication");

    // Exactly one checkout exists in the store for this repo.
    let checkouts = store.path().join("store/checkouts/local");
    let count = walk_count_dirs_named(&checkouts, &head);
    assert_eq!(count, 1, "exactly one checkout for {head}");

    // Re-sync is a no-op fast path.
    let out2 = ok(&envit(proj_a.path(), store.path(), &["sync"]));
    assert!(out2.contains("fresh"), "second sync is up-to-date: {out2}");

    // status reports health.
    let st = ok(&envit(proj_a.path(), store.path(), &["status"]));
    assert!(st.contains("dep") && st.contains("link ok"), "{st}");

    // Removing a repo prunes its link on next sync.
    ok(&envit(proj_a.path(), store.path(), &["remove", "dep"]));
    ok(&envit(proj_a.path(), store.path(), &["sync"]));
    assert!(
        std::fs::read_link(proj_a.path().join(".envit/repos/dep")).is_err(),
        "undeclared repo link pruned"
    );
    ok(&envit(proj_a.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));
    ok(&envit(proj_a.path(), store.path(), &["sync"]));

    // Both projects are registered for gc.
    let reg = std::fs::read_to_string(store.path().join("projects.json")).unwrap();
    let canon_a = std::fs::canonicalize(proj_a.path()).unwrap();
    let canon_b = std::fs::canonicalize(proj_b.path()).unwrap();
    assert!(reg.contains(&*canon_a.to_string_lossy()));
    assert!(reg.contains(&*canon_b.to_string_lossy()));
}

fn walk_count_dirs_named(root: &Path, name: &str) -> usize {
    let mut n = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if e.file_name().to_string_lossy() == name {
                    n += 1;
                } else {
                    stack.push(p);
                }
            }
        }
    }
    n
}
