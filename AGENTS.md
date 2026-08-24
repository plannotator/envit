# AGENTS.md

Guidance for AI agents working in this repository.

## What this is

**envit**: a virtualenv for agent context. Product requirements, decision
records (D1-D6), and design docs live in the private repo
`plannotator/envit-internal`. Code comments that cite `PRD §…` refer to
that document.

## Build

```sh
cargo build          # workspace: envit-core (lib) + envit (bin `envit`)
cargo test
cargo clippy
```

All product logic goes in `crates/envit-core`; the CLI stays a thin shell
(decision D2, library-first). Do not add dependencies before the code that
needs them exists.

## Engineering principles

- **Lean.** No speculative abstraction, no trait for a single impl, no
  config for things with one sane value. Code earns its place by being
  needed now.
- **Fast.** Cold-start and warm-sync latency are product features
  (a product requirement). Prefer doing less over optimizing more.
- **Name-isolated.** Every name-derived string (binary name,
  manifest/lock filenames, dirs, env vars) lives ONLY in
  `envit-core/src/ident.rs`. Never inline `"envit"` in other code. Any
  future rebrand stays a one-file change plus crate renames. (Proven:
  the agentenv-to-envit rename was mechanical.)
- **Tests: real or not at all.**
  - Integration and end-to-end tests only: real temp dirs, real files,
    real git repositories (create fixtures with git in the test), real
    symlinks.
  - **No mocks, no test doubles, no unit tests for trivia.** If a test
    does not exercise behavior a user or the PRD can observe, delete it.
  - CLI behavior is tested by running the actual binary
    (`CARGO_BIN_EXE_envit`) against a temp project.
  - Tests must stay fast (the suite is a pre-commit habit, not a
    CI-only ceremony).

## Reference repos (read these instead of guessing)

**envit manages its own context (self-hosting since 2026-08-23).** The
repos below are declared in this repo's `envit.json` and materialized
under `.envit/repos/`. Read them there: they are read-only,
commit-pinned, and listed in the generated `.envit/AGENTS.md`. Run
`target/release/envit sync` if links are missing. Note: the entries are
symlinks, so use `rg --follow` / `fd -L`. The `~/oss/` clones remain as
full-history fallbacks.

| Repo | Read it for |
|---|---|
| `.envit/repos/cargo` | Global cache locking/GC patterns: closest overall model |
| `.envit/repos/toml` | Historical (D6 moved manifests to JSON); TOML-era context only |
| `.envit/repos/uv` | Store architecture: content-addressed cache, link fallback chain, `--offline`/`--frozen` semantics |
| `.envit/repos/gitoxide` | `gix`, our sole git engine (decision D4) |
| `.envit/repos/reflink-copy` | APFS/btrfs/ReFS reflink specifics |
| `.envit/repos/junction` | Windows directory junctions without admin |
| `.envit/repos/fd-lock`, `.envit/repos/fs4` | Cross-platform advisory file locks |
| `.envit/repos/mise` | TTL freshness checks, forge shorthands, no-daemon background behavior |
| `.envit/repos/clap` | CLI framework (docs usually suffice) |
| `.envit/repos/agentskills` | Agent Skills spec (shipped) |

**Skill:** `skills/envit/SKILL.md` is the distributable Agent Skill that
teaches any agent how to use envit (commands, manifest format, when to
pin/update). Portable per the agentskills spec. Keep it in lockstep with
the CLI surface when commands change.

**Self-hosting is the standing integration test:** this reference list IS
this project's `envit.json`. If `envit sync` here ever breaks, the
product is broken.

<!-- envit:begin -->
## envit: repo context

Additional context is available in `.envit/`. The source of each external
repository declared in `envit.json` is at `.envit/repos/<name>/`, read-only,
at a pinned commit. Read `.envit/AGENTS.md` for the inventory.

When you work with a dependency, read its actual code there instead of
guessing from memory. The entries are symlinks: use `rg --follow` or
`fd -L` when you search them.
<!-- envit:end -->
