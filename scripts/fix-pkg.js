#!/usr/bin/env node
// Merges root package.json metadata into the wasm-pack generated pkg/package.json.
// wasm-pack uses the crate name (genome-rs) for the npm name by default —
// this script overwrites it with the fields from our package.json template.

const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const pkgPath = path.join(root, "pkg", "package.json");
const templatePath = path.join(root, "package.json");

if (!fs.existsSync(pkgPath)) {
	console.error("pkg/package.json not found — run wasm-pack build first");
	process.exit(1);
}

const generated = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
const template = JSON.parse(fs.readFileSync(templatePath, "utf8"));

// Fields to copy from template into the generated package.json.
// We don't overwrite files, main, module, types etc since wasm-pack
// sets those correctly for the WASM output.
const MERGE_FIELDS = [
	"name",
	"version",
	"description",
	"author",
	"license",
	"repository",
	"bugs",
	"homepage",
	"keywords",
	"publishConfig",
];

for (const field of MERGE_FIELDS) {
	if (template[field] !== undefined) {
		generated[field] = template[field];
	}
}

fs.writeFileSync(pkgPath, `${JSON.stringify(generated, null, 2)}\n`);
console.log(`✓ pkg/package.json name set to "${generated.name}"`);
