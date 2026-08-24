//! M2 e2e: --frozen, --offline, copy link mode, concurrent syncs.
//! Real binary, real fixtures, no mocks.

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

fn fail(out: &Output) -> String {
    assert!(!out.status.success(), "expected failure, got: {}", String::from_utf8_lossy(&out.stdout));
    String::from_utf8_lossy(&out.stderr).into_owned()
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
    // Allow fetch-by-sha over the file transport (--frozen reproduce path).
    run(&["config", "uploadpack.allowAnySHA1InWant", "true"]);
    std::fs::write(dir.join("lib.rs"), "pub fn hello() {}\n").unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "one"]);
    run(&["rev-parse", "HEAD"])
}

#[test]
fn frozen_reproduces_and_detects_drift() {
    let upstream = tempfile::tempdir().unwrap();
    let head = git_fixture(upstream.path());
    let url = format!("file://{}", upstream.path().display());
    let store = tempfile::tempdir().unwrap();

    // Author project: normal sync writes the lock.
    let author = tempfile::tempdir().unwrap();
    ok(&envit(author.path(), store.path(), &["init"]));
    ok(&envit(author.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));
    ok(&envit(author.path(), store.path(), &["sync"]));

    // "Teammate": manifest + lock copied to a fresh machine (fresh store).
    let teammate = tempfile::tempdir().unwrap();
    let fresh_store = tempfile::tempdir().unwrap();
    for f in ["envit.json", "envit.lock.json"] {
        std::fs::copy(author.path().join(f), teammate.path().join(f)).unwrap();
    }
    let out = ok(&envit(teammate.path(), fresh_store.path(), &["sync", "--frozen"]));
    assert!(out.contains(&head[..12]), "frozen sync pins the locked commit: {out}");
    assert_eq!(
        std::fs::read_to_string(teammate.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "pub fn hello() {}\n"
    );

    // Drift: manifest entry not covered by the lock → hard error.
    let drifter = tempfile::tempdir().unwrap();
    ok(&envit(drifter.path(), store.path(), &["init"]));
    ok(&envit(drifter.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));
    let err = fail(&envit(drifter.path(), store.path(), &["sync", "--frozen"]));
    assert!(err.contains("--frozen") && err.contains("dep"), "{err}");
}

#[test]
fn offline_links_warm_store_and_refuses_cold() {
    let upstream = tempfile::tempdir().unwrap();
    git_fixture(upstream.path());
    let url = format!("file://{}", upstream.path().display());

    // Warm the store via project A.
    let store = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    ok(&envit(a.path(), store.path(), &["init"]));
    ok(&envit(a.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));
    ok(&envit(a.path(), store.path(), &["sync"]));

    // Project B with the same lock: offline works from the warm store.
    let b = tempfile::tempdir().unwrap();
    for f in ["envit.json", "envit.lock.json"] {
        std::fs::copy(a.path().join(f), b.path().join(f)).unwrap();
    }
    ok(&envit(b.path(), store.path(), &["sync", "--offline"]));
    assert!(b.path().join(".envit/repos/dep/lib.rs").exists());

    // Cold store: offline refuses with a clear error.
    let cold = tempfile::tempdir().unwrap();
    let c = tempfile::tempdir().unwrap();
    for f in ["envit.json", "envit.lock.json"] {
        std::fs::copy(a.path().join(f), c.path().join(f)).unwrap();
    }
    let err = fail(&envit(c.path(), cold.path(), &["sync", "--offline"]));
    assert!(err.contains("--offline") && err.contains("dep"), "{err}");
}

#[test]
fn copy_link_mode_materializes_real_files() {
    let upstream = tempfile::tempdir().unwrap();
    git_fixture(upstream.path());
    let url = format!("file://{}", upstream.path().display());
    let store = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    ok(&envit(proj.path(), store.path(), &["init"]));
    ok(&envit(proj.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));
    ok(&envit(proj.path(), store.path(), &["sync", "--link-mode", "copy"]));

    let dep = proj.path().join(".envit/repos/dep");
    assert!(std::fs::read_link(&dep).is_err(), "copy mode must not symlink");
    assert!(dep.is_dir());
    assert_eq!(std::fs::read_to_string(dep.join("lib.rs")).unwrap(), "pub fn hello() {}\n");

    // Second copy-mode sync is a no-op (marker file matches).
    let out = ok(&envit(proj.path(), store.path(), &["sync", "--link-mode", "copy"]));
    assert!(out.contains("fresh"), "{out}");
}

#[test]
fn concurrent_syncs_share_one_fetch() {
    let upstream = tempfile::tempdir().unwrap();
    let head = git_fixture(upstream.path());
    let url = format!("file://{}", upstream.path().display());
    let store = tempfile::tempdir().unwrap();

    // Four projects, same repo, synced simultaneously against a cold store.
    let projects: Vec<_> = (0..4)
        .map(|_| {
            let p = tempfile::tempdir().unwrap();
            ok(&envit(p.path(), store.path(), &["init"]));
            ok(&envit(p.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));
            p
        })
        .collect();

    let handles: Vec<_> = projects
        .iter()
        .map(|p| {
            let dir = p.path().to_path_buf();
            let st = store.path().to_path_buf();
            std::thread::spawn(move || {
                Command::new(env!("CARGO_BIN_EXE_envit"))
                    .arg("sync")
                    .current_dir(&dir)
                    .env("ENVIT_HOME", &st)
                    .output()
                    .unwrap()
            })
        })
        .collect();
    for h in handles {
        let out = h.join().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }

    // All four link to the same single checkout.
    let mut targets: Vec<_> = projects
        .iter()
        .map(|p| std::fs::read_link(p.path().join(".envit/repos/dep")).unwrap())
        .collect();
    targets.dedup();
    assert_eq!(targets.len(), 1, "all projects share one checkout");
    assert!(targets[0].ends_with(&head));
}
