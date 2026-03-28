# ── Config ────────────────────────────────────────────────────────────────────

WASM_TARGET = bundler
CARGO_PROFILE = release

# ── Default ───────────────────────────────────────────────────────────────────

.PHONY: all build build-wasm build-native test clean release

all: build

# ── Build ─────────────────────────────────────────────────────────────────────

## Build everything — native Rust binary + WASM npm package
build: build-native build-wasm

## Build native Rust library
build-native:
	cargo build --profile $(CARGO_PROFILE)

## Build WASM + npm package
build-wasm:
	wasm-pack build --target bundler --features wasm
	node scripts/fix-pkg.js

# ── Test ──────────────────────────────────────────────────────────────────────

## Run all Rust tests
test:
	cargo test

## Run tests with output visible
test-verbose:
	cargo test -- --nocapture

# ── Release ───────────────────────────────────────────────────────────────────

## Build, then publish to both crates.io and npm
release: test build
	cargo publish
	wasm-pack publish --access public

## Bump patch version and release to both registries
release-patch:
	cargo release patch --execute

## Bump minor version and release to both registries
release-minor:
	cargo release minor --execute

## Bump major version and release to both registries
release-major:
	cargo release major --execute

## Publish to npm only
publish-npm: build-wasm
	wasm-pack publish --access public

## Publish to crates.io only
publish-crate: test build-native
	cargo publish

# ── Clean ─────────────────────────────────────────────────────────────────────

## Remove all build artifacts
clean:
	cargo clean
	rm -rf pkg/

# ── Help ──────────────────────────────────────────────────────────────────────

## Show available commands
help:
	@grep -E '^##' Makefile | sed 's/## //'