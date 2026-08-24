#!/usr/bin/env node
// Resolves the platform-specific binary package and execs it.
// No download step, no postinstall: npm installs only the matching
// optional dependency for this OS/CPU.
const { spawnSync } = require("node:child_process");
const { platform, arch } = process;

const key = `${platform}-${arch}`;
const pkg = { "darwin-arm64": "@envit/darwin-arm64", "darwin-x64": "@envit/darwin-x64", "linux-arm64": "@envit/linux-arm64", "linux-x64": "@envit/linux-x64" }[key];
if (!pkg) {
  console.error(`envit: no prebuilt binary for ${key}. See https://envit.dev for other install options.`);
  process.exit(1);
}
let bin;
try {
  bin = require.resolve(`${pkg}/bin/envit`);
} catch {
  console.error(`envit: ${pkg} is not installed. Reinstall envit, or check that optional dependencies are enabled.`);
  process.exit(1);
}
const r = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
process.exit(r.status ?? 1);
