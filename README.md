<p align="center"><img src="brand/banner.svg" alt="envit" width="100%"></p>

# envit

Declarative repository context and skills for agents on your local or
cloud machines. Site: https://envit.dev

`envit.json`

```json
{
  "repos": [
    "sqlite/sqlite",
    "PostHog/posthog-js",
    "getsentry/sentry-javascript",
    "earendil-works/pi",
    "pierrecomputer/pierre"
  ],
  "skills": {
    "mattpocock/skills": ["grill-me", "grilling"],
    "emilkowalski/skills": "*",
    "dmmulroy/anti-slop": "*",
    "backnotprop/bro": "bro",
    "obra/superpowers": "*",
    "remotion-dev/skills": { "pick": "*", "modelInvocable": false },
    "cursor/plugins": { "path": "pstack", "pick": ["interrogate", "blast-radius"] }
  }
}
```

External repos that your agents can use as context, and skills that they
load. One command materializes both:

```
$ envit sync
fetched  sqlite  1b3f6d2a94c0  3184 files · 94.2 MB
fetched  posthog-js  9c41d2e807aa  2311 files · 18.7 MB
...
skill    grill-me  e1f0a9b62c44  (mattpocock/skills)
✓ synced 5 repo(s) · 45 skill(s) · 151.4 MB added to store

$ envit sync
✓ synced 5 repo(s) · store unchanged   # 9 ms
```

## Install

```sh
curl -fsSL https://envit.dev/install.sh | sh
```

Also: `brew install plannotator/tap/envit`, `cargo install envit`, `npm install -g envit`.
Windows: use WSL for now. Or build from source:

```sh
cargo build --release   # target/release/envit
```

| scope | |
|---|---|
| `envit init` | This project. Writes `envit.json`. Commit it: teammates, CI, and cloud agents reproduce the same context. |
| `envit init -g` | This machine. Writes `~/.envit/envit.json`. Skills land in `~/.agents/skills/` and `~/.claude/skills/`, for every project. |
| `envit sync` | Materializes both scopes. |

## What agents see

Skills land in `.agents/skills/` and `.claude/skills/`, where agents
already look. Repo source lands in `.envit/repos/<name>/`, read-only, at
a pinned commit. envit stores each repo once per machine, in
`~/.envit/store/`; every project links to that one copy, so nothing is
duplicated on your disk. Each sync generates `.envit/AGENTS.md` (with a
`CLAUDE.md` symlink) that lists every repo, its commit, and its path. If
the project root has an `AGENTS.md` or `CLAUDE.md`, `sync` appends a short
fenced note that points there.

## Commands

| command | |
|---|---|
| `envit sync` | resolve, fetch, check out, link, lock. Refreshes branch-tracking repos at most once per 24 h. |
| `envit sync --frozen` | lockfile only. Errors on drift. For CI and sandboxes. |
| `envit sync --offline` | no network. Links what the store has. |
| `envit add <source>` | `owner/repo`, `gitlab:…`, or any https git URL. `--ref` for branch, tag, or sha. |
| `envit update [name]` | move branch-tracking repos and skill sources to current remote heads. |
| `envit pin <name>` | freeze at the locked commit. `unpin` resumes tracking. |
| `envit status` | ref, commit, link health, staleness. Never touches the network. |
| `envit skills` | declared skills, both scopes. |
| `envit gc` | delete checkouts no project references. `--dry-run` first. |

Per-skill invocation control: `{ "name": "grill-me", "modelInvocable": false }`
in a pick list rewrites both Claude's `disable-model-invocation` and Codex's
`allow_implicit_invocation`, regardless of what the author set.

## Constraints

- No daemon. `ps` shows nothing between invocations.
- One static binary, 4 MB. Git is embedded (gitoxide): no system git, no
  libgit2, no OpenSSL.
- Plain git protocol. GitHub, GitLab, Codeberg, sourcehut, self-hosted.
  No forge APIs.
- Checkouts are immutable and content-addressed. Updates are one atomic
  symlink flip.
- `#![forbid(unsafe_code)]`. Tests run against real repositories and
  filesystems, with no mocks.

## Security

What envit contacts, what it writes, and how releases are verified:
https://envit.dev/security. Report vulnerabilities privately through
[GitHub](https://github.com/plannotator/envit/security/advisories/new).

## Agent skill

`skills/envit/` is an [Agent Skill](https://agentskills.io) that teaches an
agent how to use envit. Install it into a project or globally:

```sh
npx skills add plannotator/envit
```

## License

MIT OR Apache-2.0. The envit CLI and core library are open source. Hosted
platform services built around envit may not be.
