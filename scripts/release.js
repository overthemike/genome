import { execSync } from "node:child_process";
import { cancel, intro, outro, select, spinner } from "@clack/prompts";

intro("genome release");

const level = await select({
	message: "Select release type",
	options: [
		{ value: "patch", label: "patch", hint: "1.0.x — bug fixes" },
		{ value: "minor", label: "minor", hint: "1.x.0 — new features" },
		{ value: "major", label: "major", hint: "2.0.0 — breaking changes" },
	],
});

if (!level) {
	cancel("Release cancelled.");
	process.exit(0);
}

const s = spinner();

try {
	s.start(`Bumping ${level} version`);
	execSync(`cargo release ${level} --execute`, { stdio: "inherit" });
	s.stop(`Version bumped`);

	s.start("Building and publishing npm package");
	execSync("make publish-npm", { stdio: "inherit" });
	s.stop("Published to npm");

	outro("Release complete!");
} catch (err) {
	s.stop("Failed");
	cancel(err.message);
	process.exit(1);
}
