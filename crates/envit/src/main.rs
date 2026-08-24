#![forbid(unsafe_code)]
use envit_core::manifest::{Manifest, NewRepo};
use envit_core::{ident, project};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = ident::TOOL_NAME, version, about = "Repo context for AI agents")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a manifest in the current directory (or ~/.envit with -g)
    Init {
        /// Create the global skills manifest instead
        #[arg(short, long)]
        global: bool,
    },
    /// Add a repo to the manifest (does not sync)
    Add {
        /// Source: github:owner/repo, gitlab:group/repo, or an https:// git URL
        source: String,
        /// Branch, tag, or commit sha
        #[arg(long = "ref")]
        git_ref: Option<String>,
        /// Folder name under the context dir (defaults to the repo name)
        #[arg(long)]
        name: Option<String>,
        /// Checkout only these paths (repeatable)
        #[arg(long)]
        sparse: Vec<String>,
    },
    /// Remove a repo from the manifest
    Remove {
        /// Repo name as it appears in the manifest
        name: String,
    },
    /// Materialize the declared context: fetch, check out, link, lock
    Sync {
        /// Use the lockfile exactly; never resolve refs; error on drift
        #[arg(long)]
        frozen: bool,
        /// Never touch the network
        #[arg(long)]
        offline: bool,
        /// How to place repos in the project: symlink (default) or copy
        #[arg(long = "link-mode", value_parser = ["symlink", "copy"], default_value = "symlink")]
        link_mode: String,
        /// Sync only the global scope (~/.envit/envit.toml)
        #[arg(short, long)]
        global: bool,
    },
    /// Move branch-tracking repos/skills to their current remote heads
    Update {
        /// Repo or skill-source names to update (empty = all)
        names: Vec<String>,
        /// Update the global skills manifest instead
        #[arg(short, long)]
        global: bool,
    },
    /// Freeze a repo at its currently locked commit
    Pin {
        /// Repo name as it appears in the manifest
        name: String,
    },
    /// Resume tracking a pinned repo's original ref
    Unpin {
        /// Repo name as it appears in the manifest
        name: String,
    },
    /// Remove store checkouts not referenced by any project
    Gc {
        /// Show what would be removed without removing it
        #[arg(long)]
        dry_run: bool,
    },
    /// Show declared skills: source, picks, locked commit
    Skills {
        /// Show only the global scope
        #[arg(short, long)]
        global: bool,
    },
    /// Show each repo: ref, locked commit, link health
    Status,
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), envit_core::Error> {
    let cwd = std::env::current_dir()?;
    match cli.cmd {
        Cmd::Init { global } => {
            if global {
                let store = store::Store::open_default()?;
                let path = store.root().join(ident::MANIFEST_FILE);
                if path.exists() {
                    return Err(envit_core::Error::AlreadyInitialized(path));
                }
                std::fs::create_dir_all(store.root())?;
                std::fs::write(&path, "{\n  \"skills\": {}\n}\n")?;
                println!("✓ wrote {}", path.display());
            } else {
                let report = project::init(&cwd)?;
                println!("✓ wrote {}", ident::MANIFEST_FILE);
                if report.gitignore_updated {
                    println!("✓ added {}/ to .gitignore", ident::CONTEXT_DIR);
                }
                for f in project::inject_agent_notes(&cwd)? {
                    println!("✓ noted envit context in {f}");
                }
            }
        }
        Cmd::Add { source, git_ref, name, sparse } => {
            let root = project::find_root(&cwd)?;
            let mut m = Manifest::load(&root)?;
            let added = m.add_repo(NewRepo {
                source: &source,
                name: name.as_deref(),
                git_ref: git_ref.as_deref(),
                sparse: &sparse,
            })?;
            m.save()?;
            println!("✓ added {added} → {}", ident::MANIFEST_FILE);
        }
        Cmd::Remove { name } => {
            let root = project::find_root(&cwd)?;
            let mut m = Manifest::load(&root)?;
            m.remove_repo(&name)?;
            m.save()?;
            println!("✓ removed {name}");
        }
        Cmd::Sync { frozen, offline, link_mode, global } => {
            let store = store::Store::open_default()?;
            let opts = sync::SyncOptions {
                frozen,
                offline,
                link_mode: if link_mode == "copy" {
                    envit_core::link::LinkMode::Copy
                } else {
                    envit_core::link::LinkMode::Symlink
                },
            };
            // Scope: -g = global only. Otherwise project (plus global if a
            // global manifest exists); outside any project, global alone.
            let project_root = if global { Err(envit_core::Error::ManifestNotFound(cwd.clone())) } else { project::find_root(&cwd) };
            let report = match &project_root {
                Ok(root) => sync::sync_with(root, &store, opts)?,
                Err(_) if global || sync::global_manifest_exists(&store) => {
                    sync::SyncReport { repos: Vec::new(), skills: Vec::new() }
                }
                Err(e) => return Err(clone_err(e)),
            };
            let (mut total_files, mut total_bytes) = (0usize, 0u64);
            for r in &report.repos {
                let verb = match r.action {
                    sync::Action::Fetched => "fetched",
                    sync::Action::LinkedExisting => "linked ",
                    sync::Action::UpToDate => "fresh  ",
                };
                let detail = match r.stats {
                    Some(s) => {
                        total_files += s.files;
                        total_bytes += s.bytes;
                        format!("  {} files · {}", s.files, human_bytes(s.bytes))
                    }
                    None => String::new(),
                };
                let short = |s: &str| s[..12.min(s.len())].to_string();
                match &r.prev_commit {
                    Some(prev) if *prev != r.commit => println!(
                        "updated  {}  {} → {}{detail}",
                        r.name,
                        short(prev),
                        short(&r.commit)
                    ),
                    _ => println!("{verb}  {}  {}{detail}", r.name, short(&r.commit)),
                }
            }
            for s in &report.skills {
                println!("skill    {}  {}  ({})", s.name, &s.commit[..12.min(s.commit.len())], s.source);
            }
            // Global scope: always when -g; otherwise piggyback when the
            // global manifest exists. Every action printed, labeled.
            let mut global_count = 0;
            if global || sync::global_manifest_exists(&store) {
                let greport = sync::sync_global(&store, opts)?;
                for s in &greport.skills {
                    println!("global   {}  {}  ({})", s.name, &s.commit[..12.min(s.commit.len())], s.source);
                }
                global_count = greport.skills.len();
            }
            let mut notes = String::new();
            if !report.skills.is_empty() {
                notes.push_str(&format!(" · {} skill(s)", report.skills.len()));
            }
            if global_count > 0 {
                notes.push_str(&format!(" · {global_count} global skill(s)"));
            }
            if total_bytes > 0 {
                println!(
                    "✓ synced {} repo(s){notes} · {} files · {} added to store",
                    report.repos.len(),
                    total_files,
                    human_bytes(total_bytes)
                );
            } else {
                println!("✓ synced {} repo(s){notes} · store unchanged", report.repos.len());
            }
        }
        Cmd::Update { names, global } => {
            let store = store::Store::open_default()?;
            let report = if global {
                sync::update_global(&store, &names)?
            } else {
                let root = project::find_root(&cwd)?;
                sync::update(&root, &store, &names)?
            };
            for s in &report.skills {
                println!("skill    {}  {}  ({})", s.name, &s.commit[..12.min(s.commit.len())], s.source);
            }
            let mut moved = 0;
            for r in &report.repos {
                let short = |s: &str| s[..12.min(s.len())].to_string();
                match &r.prev_commit {
                    Some(prev) if *prev != r.commit => {
                        moved += 1;
                        println!("updated  {}  {} → {}", r.name, short(prev), short(&r.commit));
                    }
                    _ => println!("current  {}  {}", r.name, short(&r.commit)),
                }
            }
            println!("✓ {moved} repo(s) moved");
        }
        Cmd::Pin { name } => {
            let root = project::find_root(&cwd)?;
            let lock = lockfile::Lockfile::load(&root)?.unwrap_or_default();
            let commit = lock
                .get(&name)
                .map(|l| l.commit.clone())
                .ok_or_else(|| envit_core::Error::NeverSynced(name.clone()))?;
            let mut m = Manifest::load(&root)?;
            m.pin_repo(&name, &commit)?;
            m.save()?;
            println!("⏸ {name} frozen @ {}", &commit[..12.min(commit.len())]);
        }
        Cmd::Unpin { name } => {
            let root = project::find_root(&cwd)?;
            let mut m = Manifest::load(&root)?;
            let tracked = m.unpin_repo(&name)?;
            m.save()?;
            println!("▶ {name} tracking {tracked} again (run `envit update {name}` to move)");
        }
        Cmd::Skills { global } => {
            let store = store::Store::open_default()?;
            let mut shown = 0;
            let print_scope = |label: &str, root: &std::path::Path, shown: &mut usize| -> Result<(), envit_core::Error> {
                let m = Manifest::load(root)?;
                let lock = lockfile::Lockfile::load(root)?.unwrap_or_default();
                for e in m.skill_entries() {
                    let commit = lock
                        .get_skill_source(&e.source)
                        .map(|l| l.commit[..12.min(l.commit.len())].to_string())
                        .unwrap_or_else(|| "unsynced".to_string());
                    let picks = match &e.pick {
                        envit_core::manifest::Pick::All => "*".to_string(),
                        envit_core::manifest::Pick::Named(n) => n
                            .iter()
                            .map(|i| i.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    };
                    println!("{label}{}  [{}]  {}", e.source, picks, commit);
                    *shown += 1;
                }
                Ok(())
            };
            if !global && let Ok(root) = project::find_root(&cwd) {
                print_scope("", &root, &mut shown)?;
            }
            if sync::global_manifest_exists(&store) {
                print_scope("global   ", store.root(), &mut shown)?;
            }
            if shown == 0 {
                println!("no skills declared (add a \"skills\" object to envit.json, or `envit init -g`)");
            }
        }
        Cmd::Gc { dry_run } => {
            let store = store::Store::open_default()?;
            let report = envit_core::gc::gc(&store, dry_run)?;
            let verb = if dry_run { "would remove" } else { "removed" };
            for (path, bytes) in &report.removed {
                println!("{verb}  {}  {}", path.display(), human_bytes(*bytes));
            }
            println!(
                "✓ {verb} {} checkout(s) · {} · {} kept{}",
                report.removed.len(),
                human_bytes(report.freed_bytes()),
                report.kept,
                if report.pruned_projects > 0 {
                    format!(" · {} dead project(s) pruned", report.pruned_projects)
                } else {
                    String::new()
                }
            );
        }
        Cmd::Status => {
            let root = project::find_root(&cwd)?;
            let m = Manifest::load(&root)?;
            let lock = lockfile::Lockfile::load(&root)?.unwrap_or_default();
            let st = store::Store::open_default()?;
            for e in m.entries() {
                let commit = lock
                    .get(&e.name)
                    .map(|l| l.commit[..12.min(l.commit.len())].to_string())
                    .unwrap_or_else(|| "unsynced".to_string());
                let link = root.join(ident::CONTEXT_DIR).join("repos").join(&e.name);
                let health = match std::fs::read_link(&link) {
                    Ok(target) if target.is_dir() => "link ok",
                    Ok(_) => "BROKEN LINK",
                    Err(_) if link.is_dir() => {
                        // Copy mode: healthy when the marker matches the lock.
                        let marker = link.with_file_name(format!("{}.commit", e.name));
                        let marker_commit = std::fs::read_to_string(&marker).ok();
                        match (marker_commit, lock.get(&e.name)) {
                            (Some(m), Some(l)) if m == l.commit => "copy ok",
                            _ => "STALE COPY",
                        }
                    }
                    Err(_) => "not linked",
                };
                let policy = if e.frozen {
                    "  frozen".to_string()
                } else if e.manual {
                    "  manual".to_string()
                } else {
                    // Staleness is reported, never acted on, by status (PRD §12a).
                    match envit_core::source::expand(&e.source)
                        .ok()
                        .and_then(|url| st.remote_check_age(&url))
                    {
                        Some(age) if age.as_secs() < 3600 => String::new(),
                        Some(age) => format!("  checked {}h ago", age.as_secs() / 3600),
                        None => "  never checked".to_string(),
                    }
                };
                println!(
                    "{}  {}  {}  {}{policy}",
                    e.name,
                    e.git_ref.as_deref().unwrap_or("HEAD"),
                    commit,
                    health
                );
            }
        }
    }
    Ok(())
}

use envit_core::{lockfile, store, sync};

fn clone_err(e: &envit_core::Error) -> envit_core::Error {
    // Errors aren't Clone; recreate the only variant this path produces.
    match e {
        envit_core::Error::ManifestNotFound(p) => envit_core::Error::ManifestNotFound(p.clone()),
        _ => envit_core::Error::ManifestNotFound(std::path::PathBuf::new()),
    }
}

fn human_bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{b} B") } else { format!("{v:.1} {}", UNITS[u]) }
}
