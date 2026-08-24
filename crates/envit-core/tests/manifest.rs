//! Integration tests: real files in real temp dirs. No mocks (AGENTS.md).
//! Manifest format (D6): JSON — `repos` array of strings/objects, `skills`
//! object keyed by source.

use envit_core::manifest::{Manifest, NewRepo, Pick};
use envit_core::{ident, project, source, Error};

fn add(m: &mut Manifest, src: &str, git_ref: Option<&str>) -> Result<String, Error> {
    m.add_repo(NewRepo { source: src, name: None, git_ref, sparse: &[] })
}

#[test]
fn json_format_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    project::init(root).unwrap();
    let path = root.join(ident::MANIFEST_FILE);

    // Bare add → plain string entry.
    let mut m = Manifest::load(root).unwrap();
    add(&mut m, "tokio-rs/tokio", None).unwrap();
    m.save().unwrap();
    let out = std::fs::read_to_string(&path).unwrap();
    assert!(out.contains("\"tokio-rs/tokio\""), "{out}");

    // Add with options → object entry.
    let mut m = Manifest::load(root).unwrap();
    add(&mut m, "Effect-TS/effect", Some("next")).unwrap();
    m.save().unwrap();
    let out = std::fs::read_to_string(&path).unwrap();
    assert!(out.contains("\"source\": \"Effect-TS/effect\""), "{out}");
    assert!(out.contains("\"ref\": \"next\""), "{out}");

    // Both entries parse with the right defaults.
    let m = Manifest::load(root).unwrap();
    let entries = m.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "tokio");
    assert_eq!(entries[0].git_ref, None, "string entry tracks the default branch");
    assert_eq!(entries[1].name, "effect");
    assert_eq!(entries[1].git_ref.as_deref(), Some("next"));

    // Duplicates rejected across both entry shapes.
    let mut m = Manifest::load(root).unwrap();
    assert!(matches!(
        add(&mut m, "other/tokio", None),
        Err(Error::DuplicateName(n)) if n == "tokio"
    ));

    // Remove a string entry; the other survives.
    let mut m = Manifest::load(root).unwrap();
    m.remove_repo("tokio").unwrap();
    m.save().unwrap();
    let out = std::fs::read_to_string(&path).unwrap();
    assert!(!out.contains("tokio"), "{out}");
    assert!(out.contains("effect"), "{out}");
    let mut m = Manifest::load(root).unwrap();
    assert!(matches!(m.remove_repo("tokio"), Err(Error::NameNotFound(_))));
}

#[test]
fn hand_written_minimal_manifest_just_works() {
    // The whole point: a human (or platform) writes plain JSON.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(ident::MANIFEST_FILE),
        r#"{
  "repos": [
    "tokio-rs/tokio",
    "https://git.corp.example/team/api.git"
  ],
  "skills": {
    "vercel-labs/agent-skills": ["web-design-guidelines", "react-best-practices"],
    "acme/team-skills": "*"
  }
}"#,
    )
    .unwrap();
    let m = Manifest::load(dir.path()).unwrap();
    let e = m.entries();
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].name, "tokio");
    assert_eq!(e[1].name, "api");

    let s = m.skill_entries();
    assert_eq!(s.len(), 2);
    let names: Vec<&str> = match &s[0].pick {
        Pick::Named(items) => items.iter().map(|i| i.name.as_str()).collect(),
        Pick::All => vec![],
    };
    assert_eq!(names, ["web-design-guidelines", "react-best-practices"]);
    assert_eq!(s[1].pick, Pick::All);

    // Malformed JSON errors loudly (trailing comma).
    std::fs::write(dir.path().join(ident::MANIFEST_FILE), "{\"repos\": [\"a/b\",]}").unwrap();
    assert!(Manifest::load(dir.path()).is_err());
}

#[test]
fn pin_unpin_shape_transitions() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    project::init(root).unwrap();
    let mut m = Manifest::load(root).unwrap();
    add(&mut m, "tokio-rs/tokio", None).unwrap();
    m.save().unwrap();

    // Pin a plain string entry → object with sha/frozen/tracks.
    let mut m = Manifest::load(root).unwrap();
    m.pin_repo("tokio", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
    m.save().unwrap();
    let out = std::fs::read_to_string(root.join(ident::MANIFEST_FILE)).unwrap();
    assert!(out.contains("\"update\": \"frozen\""), "{out}");
    assert!(out.contains("\"tracks\": \"HEAD\""), "{out}");
    let m2 = Manifest::load(root).unwrap();
    assert!(m2.entries()[0].frozen);

    // Unpin → collapses back to the plain string.
    let mut m = Manifest::load(root).unwrap();
    let tracked = m.unpin_repo("tokio").unwrap();
    assert_eq!(tracked, "HEAD");
    m.save().unwrap();
    let out = std::fs::read_to_string(root.join(ident::MANIFEST_FILE)).unwrap();
    assert!(out.contains("\"tokio-rs/tokio\""), "{out}");
    assert!(!out.contains("frozen") && !out.contains("tracks"), "{out}");
}

#[test]
fn init_is_not_repeatable_and_updates_existing_gitignore() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    project::init(root).unwrap();
    assert!(!root.join(".gitignore").exists());
    assert!(matches!(project::init(root), Err(Error::AlreadyInitialized(_))));

    let dir2 = tempfile::tempdir().unwrap();
    let root2 = dir2.path();
    std::fs::write(root2.join(".gitignore"), "target/\n").unwrap();
    let report = project::init(root2).unwrap();
    assert!(report.gitignore_updated);
    let gi = std::fs::read_to_string(root2.join(".gitignore")).unwrap();
    assert!(gi.contains(&format!("{}/", ident::CONTEXT_DIR)));
    assert!(gi.contains("target/"));
}

#[test]
fn find_root_walks_up_from_nested_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    project::init(root).unwrap();
    let nested = root.join("src").join("deep");
    std::fs::create_dir_all(&nested).unwrap();
    assert_eq!(project::find_root(&nested).unwrap(), root);

    let orphan = tempfile::tempdir().unwrap();
    assert!(matches!(
        project::find_root(orphan.path()),
        Err(Error::ManifestNotFound(_))
    ));
}

#[test]
fn source_expansion_and_default_names() {
    let cases = [
        ("tokio-rs/tokio", "https://github.com/tokio-rs/tokio.git", "tokio"),
        ("github:tokio-rs/tokio", "https://github.com/tokio-rs/tokio.git", "tokio"),
        ("gitlab:group/sub/repo", "https://gitlab.com/group/sub/repo.git", "repo"),
        ("bitbucket:owner/repo", "https://bitbucket.org/owner/repo.git", "repo"),
        ("codeberg:owner/repo", "https://codeberg.org/owner/repo.git", "repo"),
        ("sourcehut:~user/repo", "https://git.sr.ht/~user/repo", "repo"),
        ("https://git.corp.example/team/api.git", "https://git.corp.example/team/api.git", "api"),
    ];
    for (src, url, name) in cases {
        assert_eq!(source::expand(src).unwrap(), url, "expanding {src}");
        assert_eq!(source::default_name(src).unwrap(), name, "naming {src}");
    }

    for bad in ["github:justowner", "github:a/b/c", "a/b/c", "sourcehut:user/repo", "ssh://git@host/x", "tokio", "https://"] {
        assert!(source::expand(bad).is_err(), "{bad} should be rejected");
    }
}
