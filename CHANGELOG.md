# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/).

## [0.3.0] - 2026-02-10

### Added
- Public release and governance assets: `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, and `CODE_OF_CONDUCT.md`.
- Packaging and release automation:
  - Release workflow for tagged builds with multi-platform artifacts.
  - `dependabot.yml` for automated dependency updates.
  - `deny.toml` baseline policy for `cargo deny`.
- CLI distribution assets generator (`src/bin/generate-assets.rs`) for:
  - shell completions (bash, zsh, fish, PowerShell)
  - man page generation
- Architecture extension points:
  - `TreeBuilder` / `WalkdirTreeBuilder`
  - `FsWatcher` / `NotifyFsWatcher`
  - dedicated `ScrollState` in the event loop

### Changed
- Expanded `Cargo.toml` package metadata for publishing (`readme`, `license`, `repository`, `homepage`, `keywords`, `categories`, `rust-version`, explicit features).
- Improved CLI behavior and UX:
  - clearer `--help` with examples
  - verbosity controls via `-v` / `--quiet`
  - color automatically disabled on non-TTY output while respecting `NO_COLOR`
  - graceful `SIGINT` handling
- Improved startup error handling with contextual errors (`anyhow`) and clearer user-facing messages.
- Upgraded CI quality gates to run `fmt`, `clippy -D warnings`, tests, `cargo audit`, and `cargo deny`, plus cross-platform and MSRV validation.
- Fully refreshed `README.md` to include installation paths, usage examples, configuration reference, quality checks, and release guidance.

### Security
- Enforced safer defaults and release hygiene through:
  - explicit MSRV declaration
  - dependency auditing in CI (`cargo audit`)
  - dependency/license policy checks (`cargo deny`)

## [0.2.2] - 2026-02-10

### Added
- Architecture and quality improvements for release readiness.

