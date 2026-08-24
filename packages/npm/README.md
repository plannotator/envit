# npm packages

`envit` is the meta package: a Node shim that execs the binary from the
matching platform package (`@envit/darwin-arm64`, `@envit/darwin-x64`,
`@envit/linux-arm64`, `@envit/linux-x64`), which npm selects through
`optionalDependencies` plus `os`/`cpu` fields. No postinstall download.

The release workflow copies each built binary into its platform package's
`bin/` and publishes all five with provenance. First publish of each
package is manual (npm trusted publishing needs an existing package);
after that, the workflow publishes on every tag.

Manual first publish, from a checkout of the tag with the release
tarballs extracted into place:

```sh
for p in darwin-arm64 darwin-x64 linux-arm64 linux-x64; do
  (cd packages/npm/$p && npm publish --access public)
done
(cd packages/npm/envit && npm publish --access public)
```
