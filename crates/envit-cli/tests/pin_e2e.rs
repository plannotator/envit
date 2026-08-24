//! e2e: pin freezes against update; unpin restores tracking (PRD §12a,
//! §18: "a frozen repo never changes, verifiably").

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
fn pin_survives_update_and_unpin_restores_tracking() {
    let upstream = tempfile::tempdir().unwrap();
    git(upstream.path(), &["init", "-b", "main", "."]);
    std::fs::write(upstream.path().join("lib.rs"), "v1\n").unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "one"]);
    let first = git(upstream.path(), &["rev-parse", "HEAD"]);

    let url = format!("file://{}", upstream.path().display());
    let store = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    ok(&envit(proj.path(), store.path(), &["init"]));
    ok(&envit(proj.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"]));

    // Pin before first sync: clean error telling the user what to do.
    let out = envit(proj.path(), store.path(), &["pin", "dep"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("never been synced"));

    ok(&envit(proj.path(), store.path(), &["sync"]));

    // Pin at the current commit. Manifest gets sha + frozen + tracks.
    let out = ok(&envit(proj.path(), store.path(), &["pin", "dep"]));
    assert!(out.contains(&first[..12]), "{out}");
    let manifest = std::fs::read_to_string(proj.path().join("envit.json")).unwrap();
    assert!(manifest.contains(&format!("\"ref\": \"{first}\"")));
    assert!(manifest.contains("\"update\": \"frozen\""));
    assert!(manifest.contains("\"tracks\": \"main\""));

    // Double pin: clean error.
    let out = envit(proj.path(), store.path(), &["pin", "dep"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already pinned"));

    // Upstream moves; update must NOT move the pinned repo.
    std::fs::write(upstream.path().join("lib.rs"), "v2\n").unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "two"]);
    let second = git(upstream.path(), &["rev-parse", "HEAD"]);

    let out = ok(&envit(proj.path(), store.path(), &["update"]));
    assert!(out.contains("0 repo(s) moved"), "frozen repo immune to update: {out}");
    assert_eq!(
        std::fs::read_to_string(proj.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "v1\n",
        "PRD §18: a frozen repo never changes"
    );

    // status shows the freeze.
    let st = ok(&envit(proj.path(), store.path(), &["status"]));
    assert!(st.contains("frozen"), "{st}");

    // Unpin restores tracking; update now moves.
    let out = ok(&envit(proj.path(), store.path(), &["unpin", "dep"]));
    assert!(out.contains("tracking main"), "{out}");
    let manifest = std::fs::read_to_string(proj.path().join("envit.json")).unwrap();
    assert!(manifest.contains("\"ref\": \"main\""));
    assert!(!manifest.contains("frozen") && !manifest.contains("tracks"));

    let out = ok(&envit(proj.path(), store.path(), &["update"]));
    assert!(out.contains(&second[..12]) && out.contains("1 repo(s) moved"), "{out}");
    assert_eq!(
        std::fs::read_to_string(proj.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "v2\n"
    );

    // Unpin when not pinned: clean error.
    let out = envit(proj.path(), store.path(), &["unpin", "dep"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not pinned"));
}
