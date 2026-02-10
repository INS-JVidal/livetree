# Rust CLI Application — Public Release Checklist

Structured plan for preparing a Rust CLI application for public distribution via GitHub.

---

## 1. Repository Structure & Essentials

- [✅] **README.md** — Include: project description, features, installation methods, usage examples with output, configuration options, and license badge.
- [✅] **LICENSE** — Choose and add a license file (MIT, Apache-2.0, or dual MIT/Apache-2.0 which is the Rust ecosystem convention).
- [✅] **CHANGELOG.md** — Document changes per version. Follow [Keep a Changelog](https://keepachangelog.com/) format.
- [✅] **CONTRIBUTING.md** — Contribution guidelines: how to report bugs, submit PRs, coding style, and testing expectations.
- [✅] **SECURITY.md** — Instructions for reporting vulnerabilities privately (use GitHub Security Advisories).
- [✅] **CODE_OF_CONDUCT.md** — Adopt Contributor Covenant or similar.
- [✅] **.gitignore** — Use the standard Rust template (`/target`, `Cargo.lock` only if library — keep it for binaries).
- [ ] **Clean git history** — Ensure no secrets, credentials, API keys, or personal paths leaked in any commit. Use `git log -p | grep -i "password\|secret\|key\|token"` or tools like `gitleaks`.

---

## 2. Cargo & Project Configuration

- [✅] **Cargo.toml metadata** — Fill in all publishing-relevant fields:
  ```toml
  [package]
  name = "your-app"
  version = "0.1.0"
  edition = "2021"
  authors = ["Your Name <email@example.com>"]
  description = "A brief description of your CLI tool"
  readme = "README.md"
  license = "MIT"
  repository = "https://github.com/user/repo"
  homepage = "https://github.com/user/repo"
  keywords = ["cli", "tool", "relevant-keyword"]
  categories = ["command-line-utilities"]
  ```
- [✅] **Minimum Supported Rust Version (MSRV)** — Define `rust-version = "1.XX"` in `Cargo.toml` and document it.
- [✅] **Dependency audit** — Run `cargo audit` to check for known vulnerabilities. Add `cargo-audit` to CI.
- [ ] **Dependency review** — Minimise dependencies. Verify each crate is maintained and trustworthy.
- [✅] **Lock file** — Commit `Cargo.lock` (mandatory for binary projects, ensures reproducible builds).
- [✅] **Feature flags** — Use Cargo features for optional functionality if applicable.

---

## 3. CLI Conventions & UX

- [✅] **Argument parsing** — Use `clap` (with derive macros) for argument parsing. It provides `--help` and `--version` automatically.
- [✅] **`--help` output** — Ensure it is clear, complete, and includes usage examples. Review the output manually.
- [✅] **`--version` flag** — Must report the correct version (use `clap`'s automatic version from `Cargo.toml`).
- [✅] **Exit codes** — Return `0` on success, `1` for general errors, `2` for usage/argument errors. Use `std::process::ExitCode` or `process::exit()`.
- [✅] **stdout vs stderr** — Normal output to `stdout`, errors/warnings/progress to `stderr`. This enables correct piping behaviour.
- [✅] **No colour in non-TTY** — Respect `NO_COLOR` environment variable ([no-color.org](https://no-color.org/)). Detect if output is a terminal before emitting ANSI codes. Crates like `anstream` or `supports-color` help.
- [✅] **Verbosity levels** — Implement `-v` / `-vv` / `--quiet` flags for controlling output verbosity.
- [✅] **Graceful error messages** — Use `anyhow` for application errors with context. Never show raw panics or stack traces to end users.
- [✅] **Signal handling** — Handle `SIGINT` (Ctrl+C) gracefully. Clean up temporary files or resources.

---

## 4. Code Quality

- [✅] **Formatting** — Run `cargo fmt` and include a `rustfmt.toml` if you customise any rules.
- [✅] **Linting** — Run `cargo clippy -- -D warnings` (treat warnings as errors). Fix all warnings.
- [✅] **No `unwrap()` in production code** — Use proper error handling with `Result` and `?`. Reserve `unwrap()` for tests only.
- [ ] **Documentation comments** — Add `///` doc comments to all public items. Run `cargo doc --open` to verify.
- [✅] **Module organisation** — Separate CLI parsing, core logic, and I/O. Keep `main.rs` thin — delegate to a `lib.rs` or modules.
- [ ] **No hardcoded paths** — Use `dirs` or `directories` crate for platform-appropriate paths (config, cache, data).

---

## 5. Testing

- [✅] **Unit tests** — Test core logic functions with `#[cfg(test)]` modules.
- [✅] **Integration tests** — Place in `tests/` directory. Test the actual binary using `assert_cmd` and `predicates` crates:
  ```rust
  use assert_cmd::Command;
  
  #[test]
  fn test_help_flag() {
      Command::cargo_bin("your-app")
          .unwrap()
          .arg("--help")
          .assert()
          .success()
          .stdout(predicates::str::contains("Usage"));
  }
  ```
- [✅] **Error case tests** — Test invalid input, missing files, bad arguments return correct exit codes and error messages.
- [ ] **Test coverage** — Consider `cargo-tarpaulin` or `cargo-llvm-cov` to measure coverage.

---

## 6. Security

- [✅] **Input validation** — Sanitise all user input. Beware of path traversal, shell injection, and oversized inputs.
- [✅] **No `unsafe` without justification** — If used, document why and audit thoroughly.
- [✅] **Dependency audit** — Run `cargo audit` regularly. Configure Dependabot or `cargo-deny` for automated checks.
- [✅] **`cargo-deny`** — Configure to check for: duplicate dependencies, banned licenses, known vulnerabilities, and unmaintained crates.
  ```
  cargo install cargo-deny
  cargo deny init
  cargo deny check
  ```
- [ ] **File operations** — Use absolute paths or validate relative paths. Avoid following symlinks blindly.
- [ ] **TLS verification** — If making network requests, never disable certificate verification.
- [ ] **Secrets in memory** — If handling passwords/tokens, use `secrecy` or `zeroize` crates to clear sensitive data from memory.

---

## 7. Internationalisation (i18n)

- [ ] **Externalise all user-facing strings** — No hardcoded messages in business logic.
- [ ] **i18n crate** — Use `rust-i18n`, `fluent-rs` (Mozilla's Fluent), or `gettext-rs` for translation support.
- [ ] **Locale detection** — Respect `LANG`, `LC_MESSAGES`, and `LANGUAGE` environment variables.
- [ ] **UTF-8 everywhere** — Rust strings are UTF-8 by default, but verify file I/O and terminal output handle it correctly.
- [ ] **Provide base translations** — At minimum: English (`en`), Catalan (`ca`), Spanish (`es`).
- [ ] **Translation file format** — Use `.ftl` (Fluent) or `.po/.pot` (gettext) files in a `locales/` or `i18n/` directory.
- [ ] **Date/number formatting** — If applicable, use locale-aware formatting.

---

## 8. CI/CD — GitHub Actions

- [✅] **Basic CI workflow** — Create `.github/workflows/ci.yml`:
  ```yaml
  name: CI
  on: [push, pull_request]
  jobs:
    check:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - uses: dtolnay/rust-toolchain@stable
        - run: cargo fmt --check
        - run: cargo clippy -- -D warnings
        - run: cargo test
        - run: cargo audit
  ```
- [✅] **Test on multiple platforms** — Add `runs-on: [ubuntu-latest, macos-latest, windows-latest]` if cross-platform support is intended.
- [✅] **Test against MSRV** — Add a job that tests with the minimum supported Rust version.
- [✅] **Release workflow** — Automate binary builds on tag push using `cargo-dist` or `cross` + GitHub Releases.

---

## 9. Distribution & Installation

- [✅] **GitHub Releases** — Attach pre-built binaries for each target:
  - `x86_64-unknown-linux-gnu` (Linux x86_64)
  - `x86_64-unknown-linux-musl` (static Linux binary — highly recommended)
  - `aarch64-unknown-linux-gnu` (Linux ARM64, optional)
  - `x86_64-apple-darwin` / `aarch64-apple-darwin` (macOS, optional)
  - `x86_64-pc-windows-msvc` (Windows, optional)
- [✅] **Static linking (musl)** — Provide a statically linked Linux binary for maximum portability.
- [ ] **`cargo-dist`** — Consider using [cargo-dist](https://opensource.axo.dev/cargo-dist/) to automate release artifact generation, installers, and shell/PowerShell install scripts.
- [ ] **crates.io** — Publish with `cargo publish` so users can install via `cargo install your-app`.
- [✅] **Install script** — Provide a one-liner in the README:
  ```bash
  # From crates.io
  cargo install your-app
  
  # From source
  git clone https://github.com/user/repo && cd repo && cargo install --path .
  
  # Pre-built binary
  curl -sSL https://github.com/user/repo/releases/latest/download/your-app-x86_64-linux -o your-app
  chmod +x your-app
  ```
- [✅] **Shell completions** — Generate completions for bash, zsh, fish using `clap_complete`. Include them in releases or install them automatically.
- [✅] **Man page** — Generate with `clap_mangen` and include in releases.

---

## 10. Versioning & Release Process

- [✅] **Semantic Versioning** — Follow [semver.org](https://semver.org/): `MAJOR.MINOR.PATCH`.
- [✅] **Git tags** — Tag releases as `v0.1.0`, `v1.0.0`, etc.
- [✅] **Pre-release at `0.x`** — Start at `0.1.0` to signal the API is not yet stable.
- [✅] **CHANGELOG update** — Update before every release.
- [ ] **`cargo-release`** — Consider using it to automate the bump-tag-publish cycle.

---

## 11. Documentation

- [✅] **README sections** — Must include: Overview, Installation, Usage (with examples), Configuration, Contributing, License.
- [✅] **Badges in README** — Add: CI status, crates.io version, license, MSRV.
  ```markdown
  [![CI](https://github.com/user/repo/actions/workflows/ci.yml/badge.svg)](...)
  [![Crates.io](https://img.shields.io/crates/v/your-app)](https://crates.io/crates/your-app)
  [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
  ```
- [✅] **Usage examples** — Show 3–5 real-world usage scenarios with actual command output.
- [✅] **Configuration reference** — Document all environment variables, config file options, and flags.
- [ ] **API docs** — Run `cargo doc` and optionally publish to docs.rs (automatic if on crates.io).

---

## Quick Priority Guide

| Priority | Category | Effort |
|----------|----------|--------|
| 🔴 Must have | Repository essentials, CLI conventions, exit codes, `--help`, license | Low |
| 🔴 Must have | Clean git history, no secrets | Low |
| 🔴 Must have | `cargo fmt` + `cargo clippy` clean | Low |
| 🟠 Should have | CI with GitHub Actions | Medium |
| 🟠 Should have | Integration tests with `assert_cmd` | Medium |
| 🟠 Should have | Pre-built binaries in GitHub Releases | Medium |
| 🟠 Should have | `cargo audit` + `cargo-deny` | Low |
| 🟠 Should have | crates.io publishing | Low |
| 🟡 Nice to have | i18n support | High |
| 🟡 Nice to have | Shell completions + man page | Medium |
| 🟡 Nice to have | Cross-platform builds | Medium |
| 🟡 Nice to have | `cargo-dist` automation | Medium |