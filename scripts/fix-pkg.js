#!/usr/bin/env node
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const pkgDir = path.join(root, "pkg");
const pkgPath = path.join(pkgDir, "package.json");
const templatePath = path.join(root, "package.json");

if (!fs.existsSync(pkgPath)) {
	console.error("pkg/package.json not found — run wasm-pack build first");
	process.exit(1);
}

const renames = [
	["genome_rs.js", "genome.js"],
	["genome_rs_bg.wasm", "genome_bg.wasm"],
	["genome_rs_bg.wasm.d.ts", "genome_bg.wasm.d.ts"],
	["genome_rs.d.ts", "genome.d.ts"],
	["genome_rs_bg.js", "genome_bg.js"],
];

for (const [from, to] of renames) {
	const fromPath = path.join(pkgDir, from);
	const toPath = path.join(pkgDir, to);
	if (fs.existsSync(fromPath)) {
		fs.renameSync(fromPath, toPath);
		console.log(`✓ renamed ${from} → ${to}`);
	}
}

const filesToPatch = ["genome.js", "genome.d.ts", "genome_bg.wasm.d.ts"];

for (const file of filesToPatch) {
	const filePath = path.join(pkgDir, file);
	if (!fs.existsSync(filePath)) continue;
	const content = fs.readFileSync(filePath, "utf8");
	const patched = content
		.replaceAll("genome_rs_bg", "genome_bg")
		.replaceAll("genome_rs", "genome");
	fs.writeFileSync(filePath, patched);
	console.log(`✓ patched references in ${file}`);
}

const generated = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
const template = JSON.parse(fs.readFileSync(templatePath, "utf8"));

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

for (const key of ["main", "module", "types"]) {
	if (generated[key]) {
		generated[key] = generated[key].replaceAll("genome_rs", "genome");
	}
}

if (generated.exports) {
	generated.exports = JSON.parse(
		JSON.stringify(generated.exports).replaceAll("genome_rs", "genome"),
	);
}

if (generated.files) {
	generated.files = generated.files.map((f) =>
		f.replaceAll("genome_rs", "genome"),
	);
}

if (Array.isArray(generated.sideEffects)) {
	generated.sideEffects = generated.sideEffects.map((f) =>
		f.replaceAll("genome_rs", "genome"),
	);
}

// ← this was missing — nothing was ever saved
fs.writeFileSync(pkgPath, JSON.stringify(generated, null, 2) + "\n");
console.log(
	`✓ pkg/package.json saved as "${generated.name}@${generated.version}"`,
);
