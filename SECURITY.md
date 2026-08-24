# Security policy

## Report a vulnerability

Do not open a public issue for a suspected vulnerability. Use GitHub's
private reporting form:

https://github.com/plannotator/envit/security/advisories/new

Include the affected version, reproduction steps, impact, and any known
workaround. Fixes are provided for the latest released version.

## What envit does and does not do

envit contacts only the git remotes you declare in `envit.json`. It sends
no telemetry, checks for no updates, and opens no ports. A full list of
network requests and filesystem writes is at https://envit.dev/security.

## Release integrity

Release binaries are built by the public workflow in
`.github/workflows/release.yml`. Each asset ships with a SHA-256 checksum
and a GitHub build-provenance attestation. Verify an asset with:

```sh
gh attestation verify envit-<target>.tar.gz --owner plannotator
```
