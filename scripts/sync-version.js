#!/usr/bin/env node
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");

// Read version from Cargo.toml
const cargo = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8");
const match = cargo.match(/^version\s*=\s*"([^"]+)"/m);
if (!match) {
	console.error("Could not find version in Cargo.toml");
	process.exit(1);
}
const version = match[1];

// Write to package.json
const pkgPath = path.join(root, "package.json");
const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
pkg.version = version;
fs.writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);

console.log(`✓ synced version ${version} to package.json`);
