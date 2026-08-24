//! e2e: global scope (~/.envit/envit.json → ~/.agents/skills + ~/.claude/skills).
//! HOME is overridden to a temp dir, so nothing touches the real user.

use std::path::Path;
use std::process::{Command, Output};

fn envit(cwd: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_envit"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .env("ENVIT_HOME", home.join(".envit"))
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

fn skill_fixture(dir: &Path, name: &str) {
    let d = dir.join("skills").join(name);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: test\n---\nBody {name} v1\n"),
    )
    .unwrap();
}

#[test]
fn global_skills_lifecycle() {
    let upstream = tempfile::tempdir().unwrap();
    git(upstream.path(), &["init", "-b", "main", "."]);
    skill_fixture(upstream.path(), "alpha");
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "one"]);
    let url = format!("file://{}", upstream.path().display());

    let home = tempfile::tempdir().unwrap();
    let nowhere = tempfile::tempdir().unwrap(); // cwd with no project

    // init -g creates the template; sync outside any project = global scope.
    ok(&envit(nowhere.path(), home.path(), &["init", "-g"]));
    std::fs::write(
        home.path().join(".envit/envit.json"),
        format!("{{ \"skills\": {{ \"{url}\": \"alpha\" }} }}\n"),
    )
    .unwrap();
    let out = ok(&envit(nowhere.path(), home.path(), &["sync"]));
    assert!(out.contains("global   alpha"), "{out}");
    assert!(
        std::fs::read_to_string(home.path().join(".agents/skills/alpha/SKILL.md"))
            .unwrap()
            .contains("Body alpha v1")
    );
    assert!(home.path().join(".claude/skills/alpha").exists(), "claude fan-out at home");

    // A project sync piggybacks global, clearly labeled.
    let proj = tempfile::tempdir().unwrap();
    ok(&envit(proj.path(), home.path(), &["init"]));
    let out = ok(&envit(proj.path(), home.path(), &["sync"]));
    assert!(out.contains("global   alpha"), "project sync includes labeled global: {out}");

    // Plain update is project-scoped: upstream moves, global does NOT.
    std::fs::write(
        upstream.path().join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: test\n---\nBody alpha v2\n",
    )
    .unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "two"]);
    ok(&envit(proj.path(), home.path(), &["update"]));
    assert!(
        std::fs::read_to_string(home.path().join(".agents/skills/alpha/SKILL.md"))
            .unwrap()
            .contains("v1"),
        "plain update must not touch global"
    );

    // update -g moves it.
    ok(&envit(nowhere.path(), home.path(), &["update", "-g"]));
    assert!(
        std::fs::read_to_string(home.path().join(".agents/skills/alpha/SKILL.md"))
            .unwrap()
            .contains("v2"),
        "update -g moves global skills"
    );

    // skills lists the global scope, labeled.
    let out = ok(&envit(proj.path(), home.path(), &["skills"]));
    assert!(out.contains("global   ") && out.contains("alpha") || out.contains("global"), "{out}");

    // Repos in the global manifest: loud, specific error.
    std::fs::write(
        home.path().join(".envit/envit.json"),
        format!("{{ \"repos\": [\"{url}\"], \"skills\": {{ \"{url}\": \"alpha\" }} }}\n"),
    )
    .unwrap();
    let out = envit(nowhere.path(), home.path(), &["sync", "-g"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("skills-only"), "{:?}", out);

    // Removing the skill prunes the home links.
    std::fs::write(home.path().join(".envit/envit.json"), "{ \"skills\": {} }\n").unwrap();
    ok(&envit(nowhere.path(), home.path(), &["sync", "-g"]));
    assert!(!home.path().join(".agents/skills/alpha").exists(), "global skill pruned");
}
