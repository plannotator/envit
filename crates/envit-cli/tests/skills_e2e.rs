//! e2e: declarative skills (docs/skills-design.md). Real skill repo
//! fixture, cherry-picking, canonical + Claude fan-out, frozen reproduce.

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

/// A skills collection repo: skills/alpha, skills/beta, plus a stray dir
/// without SKILL.md that must be ignored.
fn skills_fixture(dir: &Path) -> String {
    git(dir, &["init", "-b", "main", "."]);
    for name in ["alpha", "beta"] {
        let d = dir.join("skills").join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test skill {name}\n---\n\nBody of {name}.\n"),
        )
        .unwrap();
    }
    std::fs::create_dir_all(dir.join("skills/not-a-skill")).unwrap();
    std::fs::write(dir.join("skills/not-a-skill/notes.txt"), "no SKILL.md here\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "skills"]);
    git(dir, &["rev-parse", "HEAD"])
}

#[test]
fn cherry_pick_links_canonical_and_claude() {
    let upstream = tempfile::tempdir().unwrap();
    let head = skills_fixture(upstream.path());
    let url = format!("file://{}", upstream.path().display());
    let store = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    // Manifest: hand-written, the pretty format — pick ONE of the two.
    std::fs::write(
        proj.path().join("envit.json"),
        format!("{{\n  \"skills\": {{ \"{url}\": \"alpha\" }}\n}}\n"),
    )
    .unwrap();

    let out = ok(&envit(proj.path(), store.path(), &["sync"]));
    assert!(out.contains("skill    alpha"), "{out}");
    assert!(out.contains("1 skill(s)"), "{out}");

    // Canonical link exists and resolves to real content.
    let canonical = proj.path().join(".agents/skills/alpha");
    let body = std::fs::read_to_string(canonical.join("SKILL.md")).unwrap();
    assert!(body.contains("Body of alpha"));

    // Claude fan-out: .claude/skills/alpha → canonical.
    let claude = proj.path().join(".claude/skills/alpha");
    assert!(std::fs::read_link(&claude).is_ok(), "claude entry is a symlink");
    assert!(std::fs::read_to_string(claude.join("SKILL.md")).unwrap().contains("alpha"));

    // Cherry-pick means beta was NOT linked.
    assert!(!proj.path().join(".agents/skills/beta").exists(), "beta not picked");

    // Lock records the source pinned at the fixture head.
    let lock = std::fs::read_to_string(proj.path().join("envit.lock.json")).unwrap();
    assert!(lock.contains("skillSources"), "{lock}");
    assert!(lock.contains(&head), "{lock}");

    // `envit skills` answers "what skills do I have?".
    let out = ok(&envit(proj.path(), store.path(), &["skills"]));
    assert!(out.contains("alpha") && out.contains(&head[..12]), "{out}");

    // Warm re-sync: no store growth, still reports the skill.
    let out = ok(&envit(proj.path(), store.path(), &["sync"]));
    assert!(out.contains("store unchanged"), "{out}");

    // Frozen reproduce on a fresh store from the committed files alone.
    let team = tempfile::tempdir().unwrap();
    let fresh = tempfile::tempdir().unwrap();
    for f in ["envit.json", "envit.lock.json"] {
        std::fs::copy(proj.path().join(f), team.path().join(f)).unwrap();
    }
    // (fixture needs sha-in-want for frozen fetch)
    git(upstream.path(), &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    ok(&envit(team.path(), fresh.path(), &["sync", "--frozen"]));
    assert!(
        std::fs::read_to_string(team.path().join(".agents/skills/alpha/SKILL.md"))
            .unwrap()
            .contains("Body of alpha"),
        "skills reproduce bit-identically in fresh environments"
    );

    // Picking everything with "*" gets both.
    std::fs::write(
        proj.path().join("envit.json"),
        format!("{{\n  \"skills\": {{ \"{url}\": \"*\" }}\n}}\n"),
    )
    .unwrap();
    ok(&envit(proj.path(), store.path(), &["sync"]));
    assert!(proj.path().join(".agents/skills/beta").exists(), "star picks all");

    // Removing skills from the manifest prunes their links on next sync —
    // no residue in .agents/.claude (the user's dirs).
    std::fs::write(proj.path().join("envit.json"), "{\"repos\": []}\n").unwrap();
    ok(&envit(proj.path(), store.path(), &["sync"]));
    assert!(!proj.path().join(".agents/skills/alpha").exists(), "alpha pruned");
    assert!(!proj.path().join(".claude/skills/alpha").exists(), "claude link pruned");
    assert!(!proj.path().join(".agents").exists(), "empty managed dirs tidied away");
    let lock = std::fs::read_to_string(proj.path().join("envit.lock.json")).unwrap();
    assert!(!lock.contains("skillSources"), "lock section pruned: {lock}");

    // Re-declare for the error case below.
    // Asking for a skill that doesn't exist: loud, listing what does.
    std::fs::write(
        proj.path().join("envit.json"),
        format!("{{\n  \"skills\": {{ \"{url}\": \"gamma\" }}\n}}\n"),
    )
    .unwrap();
    let out = envit(proj.path(), store.path(), &["sync"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("gamma") && err.contains("alpha") && err.contains("beta"), "{err}");
}

#[test]
fn nested_categories_and_monorepo_path() {
    let upstream = tempfile::tempdir().unwrap();
    git(upstream.path(), &["init", "-b", "main", "."]);
    // Monorepo: plugin/skills/category/skill/SKILL.md (two real-world layers).
    let d = upstream.path().join("plugin/skills/productivity/grill-me");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("SKILL.md"),
        "---\nname: grill-me\ndescription: t\n---\nGrill body.\n",
    )
    .unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "one"]);
    let url = format!("file://{}", upstream.path().display());

    let store = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    std::fs::write(
        proj.path().join("envit.json"),
        format!(
            "{{ \"skills\": {{ \"{url}\": {{ \"path\": \"plugin\", \"pick\": [\"grill-me\"] }} }} }}\n"
        ),
    )
    .unwrap();
    ok(&envit(proj.path(), store.path(), &["sync"]));
    assert!(
        std::fs::read_to_string(proj.path().join(".agents/skills/grill-me/SKILL.md"))
            .unwrap()
            .contains("Grill body"),
        "path + nested category discovery works"
    );

    // A wrong path errors loudly.
    std::fs::write(
        proj.path().join("envit.json"),
        format!("{{ \"skills\": {{ \"{url}\": {{ \"path\": \"nope\", \"pick\": [\"grill-me\"] }} }} }}\n"),
    )
    .unwrap();
    let out = envit(proj.path(), store.path(), &["sync"]);
    assert!(!out.status.success());
}

#[test]
fn invocation_override_beats_author_settings() {
    let upstream = tempfile::tempdir().unwrap();
    git(upstream.path(), &["init", "-b", "main", "."]);
    // Author ships the skill with model invocation DISABLED and an
    // openai.yaml that allows implicit invocation. Both get overridden.
    let d = upstream.path().join("skills/alpha");
    std::fs::create_dir_all(d.join("agents")).unwrap();
    std::fs::write(
        d.join("SKILL.md"),
        "---\nname: alpha\ndescription: t\ndisable-model-invocation: true\n---\nBody.\n",
    )
    .unwrap();
    std::fs::write(
        d.join("agents/openai.yaml"),
        "interface:\n  display_name: Alpha\npolicy:\n  allow_implicit_invocation: false\n",
    )
    .unwrap();
    git(upstream.path(), &["add", "."]);
    git(upstream.path(), &["commit", "-m", "one"]);
    let url = format!("file://{}", upstream.path().display());

    let store = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();

    // Override to "auto": model invocation ON regardless of the author.
    std::fs::write(
        proj.path().join("envit.json"),
        format!("{{ \"skills\": {{ \"{url}\": {{ \"pick\": [\"alpha\"], \"modelInvocable\": true }} }} }}\n"),
    )
    .unwrap();
    ok(&envit(proj.path(), store.path(), &["sync"]));
    let md = std::fs::read_to_string(proj.path().join(".agents/skills/alpha/SKILL.md")).unwrap();
    assert!(md.contains("disable-model-invocation: false"), "{md}");
    let ya = std::fs::read_to_string(proj.path().join(".agents/skills/alpha/agents/openai.yaml")).unwrap();
    assert!(ya.contains("allow_implicit_invocation: true"), "{ya}");
    assert!(ya.contains("display_name: Alpha"), "author yaml keys preserved: {ya}");

    // Flip to per-skill "explicit": both provider configs flip.
    std::fs::write(
        proj.path().join("envit.json"),
        format!("{{ \"skills\": {{ \"{url}\": {{ \"pick\": [{{ \"name\": \"alpha\", \"modelInvocable\": false }}] }} }} }}\n"),
    )
    .unwrap();
    ok(&envit(proj.path(), store.path(), &["sync"]));
    let md = std::fs::read_to_string(proj.path().join(".agents/skills/alpha/SKILL.md")).unwrap();
    assert!(md.contains("disable-model-invocation: true"), "{md}");
    let ya = std::fs::read_to_string(proj.path().join(".agents/skills/alpha/agents/openai.yaml")).unwrap();
    assert!(ya.contains("allow_implicit_invocation: false"), "{ya}");

    // Remove the override: back to a plain symlink of the author skill.
    std::fs::write(
        proj.path().join("envit.json"),
        format!("{{ \"skills\": {{ \"{url}\": [\"alpha\"] }} }}\n"),
    )
    .unwrap();
    ok(&envit(proj.path(), store.path(), &["sync"]));
    let link = proj.path().join(".agents/skills/alpha");
    assert!(std::fs::read_link(&link).is_ok(), "no override = symlink");
    let md = std::fs::read_to_string(link.join("SKILL.md")).unwrap();
    assert!(md.contains("disable-model-invocation: true"), "author file untouched");

    // Re-apply the override, then deselect the skill: the patched copy and
    // its markers are pruned.
    std::fs::write(
        proj.path().join("envit.json"),
        format!("{{ \"skills\": {{ \"{url}\": {{ \"pick\": [\"alpha\"], \"modelInvocable\": false }} }} }}\n"),
    )
    .unwrap();
    ok(&envit(proj.path(), store.path(), &["sync"]));
    std::fs::write(proj.path().join("envit.json"), "{\"repos\": []}\n").unwrap();
    ok(&envit(proj.path(), store.path(), &["sync"]));
    assert!(!proj.path().join(".agents/skills/alpha").exists(), "patched copy pruned");
    assert!(
        !proj.path().join(".agents/skills/alpha.commit").exists(),
        "markers pruned"
    );
}

