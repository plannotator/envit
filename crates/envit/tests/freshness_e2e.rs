//! e2e: TTL freshness (PRD §12a). Auto repos advance when stale on plain
//! sync; fresh/manual/pinned/no-refresh repos never move.

use std::path::Path;
use std::process::{Command, Output};

fn envit_env(dir: &Path, store: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_envit"));
    c.args(args).current_dir(dir).env("ENVIT_HOME", store);
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().expect("binary runs")
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
fn ttl_governs_when_auto_repos_move() {
    let upstream = tempfile::tempdir().unwrap();
    git(upstream.path(), &["init", "-b", "main", "."]);
    std::fs::write(upstream.path().join("lib.rs"), "v1\n").unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "one"]);

    let url = format!("file://{}", upstream.path().display());
    let store = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    ok(&envit_env(proj.path(), store.path(), &["init"], &[]));
    ok(&envit_env(proj.path(), store.path(), &["add", &url, "--ref", "main", "--name", "dep"], &[]));
    ok(&envit_env(proj.path(), store.path(), &["sync"], &[]));

    // Upstream moves.
    std::fs::write(upstream.path().join("lib.rs"), "v2\n").unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "two"]);
    let second = git(upstream.path(), &["rev-parse", "HEAD"]);

    // Within the TTL: plain sync stays pinned (default 24h; marker is fresh).
    let out = ok(&envit_env(proj.path(), store.path(), &["sync"], &[]));
    assert!(out.contains("fresh"), "within TTL nothing moves: {out}");
    assert_eq!(
        std::fs::read_to_string(proj.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "v1\n"
    );

    // TTL forced to zero: still blocked by ENVIT_NO_REFRESH kill switch...
    let out = ok(&envit_env(
        proj.path(),
        store.path(),
        &["sync"],
        &[("ENVIT_REFRESH_TTL", "0s"), ("ENVIT_NO_REFRESH", "1")],
    ));
    assert!(out.contains("fresh"), "kill switch beats staleness: {out}");

    // ...and by --frozen...
    let out = ok(&envit_env(
        proj.path(),
        store.path(),
        &["sync", "--frozen"],
        &[("ENVIT_REFRESH_TTL", "0s")],
    ));
    assert!(out.contains("fresh"), "--frozen beats staleness: {out}");

    // ...but a plain stale sync advances the auto repo and says so.
    let out = ok(&envit_env(
        proj.path(),
        store.path(),
        &["sync"],
        &[("ENVIT_REFRESH_TTL", "0s")],
    ));
    assert!(out.contains("updated") && out.contains(&second[..12]), "{out}");
    assert_eq!(
        std::fs::read_to_string(proj.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "v2\n"
    );

    // Third upstream move; manual policy blocks stale-sync movement.
    std::fs::write(upstream.path().join("lib.rs"), "v3\n").unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "three"]);
    let manifest = std::fs::read_to_string(proj.path().join("envit.json")).unwrap();
    std::fs::write(
        proj.path().join("envit.json"),
        manifest.replace("\"ref\": \"main\"", "\"ref\": \"main\", \"update\": \"manual\""),
    )
    .unwrap();
    let out = ok(&envit_env(
        proj.path(),
        store.path(),
        &["sync"],
        &[("ENVIT_REFRESH_TTL", "0s")],
    ));
    assert!(out.contains("fresh"), "manual repo ignores staleness: {out}");
    assert_eq!(
        std::fs::read_to_string(proj.path().join(".envit/repos/dep/lib.rs")).unwrap(),
        "v2\n"
    );

    // Per-repo ttl override: a huge ttl keeps an auto repo still even when
    // the global env TTL is zero (per-repo wins).
    let manifest = std::fs::read_to_string(proj.path().join("envit.json")).unwrap();
    std::fs::write(
        proj.path().join("envit.json"),
        manifest.replace(", \"update\": \"manual\"", ", \"ttl\": \"7d\""),
    )
    .unwrap();
    let out = ok(&envit_env(
        proj.path(),
        store.path(),
        &["sync"],
        &[("ENVIT_REFRESH_TTL", "0s")],
    ));
    assert!(out.contains("fresh"), "per-repo ttl beats env ttl: {out}");

    // status reports staleness info without fetching.
    let st = ok(&envit_env(proj.path(), store.path(), &["status"], &[]));
    assert!(st.contains("dep"), "{st}");
}

#[test]
fn tag_refs_never_go_stale() {
    let upstream = tempfile::tempdir().unwrap();
    git(upstream.path(), &["init", "-b", "main", "."]);
    std::fs::write(upstream.path().join("lib.rs"), "v1\n").unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "one"]);
    git(upstream.path(), &["tag", "v1.0.0"]);

    let url = format!("file://{}", upstream.path().display());
    let store = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    ok(&envit_env(proj.path(), store.path(), &["init"], &[]));
    ok(&envit_env(proj.path(), store.path(), &["add", &url, "--ref", "v1.0.0", "--name", "dep"], &[]));
    ok(&envit_env(proj.path(), store.path(), &["sync"], &[]));

    // The lock records the tag kind.
    let lock = std::fs::read_to_string(proj.path().join("envit.lock.json")).unwrap();
    assert!(lock.contains("\"tag\": true"), "{lock}");

    // Even with TTL zero, a tag ref does not re-resolve on plain sync.
    let out = ok(&envit_env(
        proj.path(),
        store.path(),
        &["sync"],
        &[("ENVIT_REFRESH_TTL", "0s")],
    ));
    assert!(out.contains("fresh"), "tags never go stale: {out}");
}
