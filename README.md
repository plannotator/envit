# envit

Declarative repository context and skills for agents on your local or
cloud machines.

A committed `envit.json` declares the external repos your agents can use
as context, and the skills they can use. `envit sync` materializes both:
commit-pinned, read-only, stored once per machine, reproducible anywhere
from the lockfile.

```json
{
  "repos": [
    "sqlite/sqlite",
    "PostHog/posthog-js"
  ],
  "skills": {
    "mattpocock/skills": ["grill-me", "grilling"],
    "obra/superpowers": "*"
  }
}
```

```sh
curl -fsSL https://envit.dev/install.sh | sh
envit sync
```

See https://envit.dev for the manifest format, commands, and on-disk
layout. `skills/envit/` in this repo is the Agent Skill that teaches an
agent how to use envit.

## Build from source

```sh
cargo build --release   # target/release/envit
cargo test
```

## Design constraints

- No daemon. Nothing runs between invocations.
- One static binary. Git is embedded (gitoxide): no system git, no
  libgit2, no OpenSSL.
- Plain git protocol against any forge. No forge APIs.
- Checkouts are immutable, content-addressed, and swapped atomically.

## License

MIT OR Apache-2.0. The envit CLI and core library are open source.
Hosted platform services built around envit may not be.
