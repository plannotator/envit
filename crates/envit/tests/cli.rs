//! End-to-end tests: run the real binary against a real temp project.

use std::path::Path;
use std::process::{Command, Output};

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_envit"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("binary runs")
}

#[test]
fn full_manifest_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // init
    let out = run(root, &["init"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(root.join("envit.json").is_file());

    // add
    let out = run(root, &["add", "github:tokio-rs/tokio", "--ref", "tokio-1.40.0"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let manifest = std::fs::read_to_string(root.join("envit.json")).unwrap();
    assert!(manifest.contains("\"source\": \"github:tokio-rs/tokio\""));
    assert!(manifest.contains("\"ref\": \"tokio-1.40.0\""));

    // add from a nested directory finds the project root
    let nested = root.join("src");
    std::fs::create_dir(&nested).unwrap();
    let out = run(&nested, &["add", "github:Effect-TS/effect"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    // duplicate add fails loudly
    let out = run(root, &["add", "github:tokio-rs/tokio"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already exists"));

    // bad source fails loudly
    let out = run(root, &["add", "not-a-source"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unrecognized source"));

    // remove
    let out = run(root, &["remove", "tokio"]);
    assert!(out.status.success());
    let manifest = std::fs::read_to_string(root.join("envit.json")).unwrap();
    assert!(!manifest.contains("tokio"));
    assert!(manifest.contains("effect"));

    // second init refuses
    let out = run(root, &["init"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("already initialized"));

    // commands outside a project fail with guidance
    let orphan = tempfile::tempdir().unwrap();
    let out = run(orphan.path(), &["remove", "x"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no manifest found"));
}
