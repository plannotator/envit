//! Integration tests for lockfile canonical emission and store paths.
//! Real files, real dirs, no mocks (AGENTS.md).

use envit_core::lockfile::{LockedRepo, Lockfile};
use envit_core::store::Store;

fn locked(name: &str, commit: &str) -> LockedRepo {
    LockedRepo {
        name: name.to_string(),
        source: format!("https://github.com/example/{name}"),
        git_ref: "main".to_string(),
        commit: commit.to_string(),
        tag: false,
    }
}

#[test]
fn lockfile_roundtrip_and_canonical_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // No lock yet → None, not an error.
    assert!(Lockfile::load(root).unwrap().is_none());

    // Insert in one order…
    let mut a = Lockfile::new();
    a.set(locked("zlib", "c0ffee00"));
    a.set(locked("effect", "3f2a9c1e"));
    a.save(root).unwrap();
    let bytes_a = std::fs::read_to_string(root.join("envit.lock.json")).unwrap();

    // …and the reverse order: identical bytes (canonical form).
    let mut b = Lockfile::new();
    b.set(locked("effect", "3f2a9c1e"));
    b.set(locked("zlib", "c0ffee00"));
    b.save(root).unwrap();
    let bytes_b = std::fs::read_to_string(root.join("envit.lock.json")).unwrap();
    assert_eq!(bytes_a, bytes_b);

    // Sorted by name, header present, machine-readable back.
    assert!(bytes_a.trim_start().starts_with("{"), "canonical JSON");
    assert!(bytes_a.find("effect").unwrap() < bytes_a.find("zlib").unwrap());
    let loaded = Lockfile::load(root).unwrap().unwrap();
    assert_eq!(loaded.get("effect").unwrap().commit, "3f2a9c1e");
    assert_eq!(loaded.repos().len(), 2);

    // Upsert replaces; retain prunes removed repos.
    let mut l = loaded;
    l.set(locked("effect", "deadbeef"));
    l.retain_named(&["effect".to_string()]);
    l.save(root).unwrap();
    let l = Lockfile::load(root).unwrap().unwrap();
    assert_eq!(l.repos().len(), 1);
    assert_eq!(l.get("effect").unwrap().commit, "deadbeef");

    // A corrupt entry errors loudly, not silently.
    std::fs::write(
        root.join("envit.lock.json"),
        "{\"version\":1,\"repos\":[{\"name\":\"x\"}]}",
    )
    .unwrap();
    assert!(Lockfile::load(root).is_err());
}

#[test]
fn store_paths_are_deterministic_and_shared() {
    let store = Store::at("/tmp/fake-store");

    // Bare repo: one per remote.
    assert_eq!(
        store.bare_dir("https://github.com/tokio-rs/tokio.git").unwrap(),
        std::path::Path::new("/tmp/fake-store/store/git/github.com/tokio-rs/tokio.git")
    );
    // Nested GitLab groups keep their full path.
    assert_eq!(
        store.bare_dir("https://gitlab.com/group/sub/repo.git").unwrap(),
        std::path::Path::new("/tmp/fake-store/store/git/gitlab.com/group/sub/repo.git")
    );

    // Checkouts keyed by SHA — same URL+SHA from two "projects" = same path.
    let c1 = store
        .checkout_dir("https://github.com/tokio-rs/tokio.git", "a1b2c3d", &[])
        .unwrap();
    let c2 = store
        .checkout_dir("https://github.com/tokio-rs/tokio", "a1b2c3d", &[])
        .unwrap();
    assert_eq!(c1, c2, ".git suffix must not split the store");
    assert!(c1.ends_with("store/checkouts/github.com/tokio-rs/tokio/a1b2c3d"));

    // Sparse checkouts get their own stable key, order-independent.
    let s1 = store
        .checkout_dir(
            "https://github.com/tokio-rs/tokio",
            "a1b2c3d",
            &["src/a".to_string(), "src/b".to_string()],
        )
        .unwrap();
    let s2 = store
        .checkout_dir(
            "https://github.com/tokio-rs/tokio",
            "a1b2c3d",
            &["src/b".to_string(), "src/a".to_string()],
        )
        .unwrap();
    assert_eq!(s1, s2, "sparse path order must not change the key");
    assert_ne!(s1, c1, "sparse and full checkouts must not collide");
    assert!(s1.to_string_lossy().contains("a1b2c3d-sparse-"));

    // sourcehut URLs (no .git, ~user) map cleanly.
    let sh = store
        .checkout_dir("https://git.sr.ht/~user/repo", "beef", &[])
        .unwrap();
    assert!(sh.ends_with("store/checkouts/git.sr.ht/~user/repo/beef"));

    // Garbage in, loud error out.
    assert!(store.bare_dir("git@github.com:x/y.git").is_err());
    assert!(store.bare_dir("https://nohostpath").is_err());
}

#[test]
fn project_registry_is_idempotent() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = Store::at(store_dir.path());

    let p1 = tempfile::tempdir().unwrap();
    let p2 = tempfile::tempdir().unwrap();

    assert!(store.register_project(p1.path()).unwrap());
    assert!(!store.register_project(p1.path()).unwrap(), "second add is a no-op");
    assert!(store.register_project(p2.path()).unwrap());

    let projects = store.projects().unwrap();
    assert_eq!(projects.len(), 2);
    let canon1 = std::fs::canonicalize(p1.path()).unwrap();
    assert!(projects.contains(&canon1));
}
