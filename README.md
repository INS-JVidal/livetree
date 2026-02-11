# livetree

[![CI](https://github.com/INS-JVidal/livetree/actions/workflows/ci.yml/badge.svg)](https://github.com/INS-JVidal/livetree/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/livetree)](https://crates.io/crates/livetree)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.74-blue)](https://github.com/INS-JVidal/livetree)

Real-time directory tree watcher for the terminal, with incremental updates and minimal flicker.

## Overview

`livetree` monitors a directory recursively and redraws a tree view as files change.  
It is built as a Rust CLI binary with:

- `clap` for robust argument parsing
- `notify` + `notify-debouncer-full` for filesystem events
- `ratatui` for terminal UI rendering

## Installation

### From crates.io

```bash
cargo install livetree
```

### From source

```bash
git clone https://github.com/INS-JVidal/livetree
cd livetree
cargo install --path .
```

### Using Make

```bash
git clone https://github.com/INS-JVidal/livetree
cd livetree
make build
make install   # installs to ~/.local/bin/livetree
```

### Quick install (Linux/macOS)

```bash
curl -sSfL https://raw.githubusercontent.com/INS-JVidal/livetree/main/install.sh | sh
```

### Pre-built binaries (GitHub Releases)

Download the archive for your platform from [Releases](https://github.com/INS-JVidal/livetree/releases/latest), then:

```bash
tar -xzf livetree-*.tar.gz
install -d ~/.local/bin
install -m 0755 livetree ~/.local/bin/
```

> **Note:** Ensure `~/.local/bin` is in your `PATH`. Add `export PATH="$HOME/.local/bin:$PATH"` to your shell profile if needed.

## Usage

### Basic

```bash
livetree .
```

### Common options

```bash
livetree -L 3 -I target -I "*.log" ./my-project
livetree --dirs-only .
NO_COLOR=1 livetree .
```

### Example output

```text
├── src
│   ├── cli.rs
│   ├── event_loop.rs
│   └── main.rs
├── tests
│   └── phase0_cli.rs
└── Cargo.toml
```

## Configuration

### CLI flags

- `-L, --level <N>`: maximum depth
- `-I, --ignore <PATTERN>`: glob patterns to exclude (repeatable)
- `-a, --all`: show hidden files
- `-D, --dirs-only`: show only directories
- `-f, --follow-symlinks`: follow symbolic links
- `--debounce <MS>`: debounce interval (minimum `50`)
- `--no-color`: disable colors
- `-v, --verbose`: increase verbosity (`-v`, `-vv`)
- `--quiet`: silence non-critical stderr messages

### Environment variables

- `NO_COLOR`: disables colored output
- `LANG`, `LC_ALL`: terminal locale behavior (UTF-8 recommended)

### MSRV

- Minimum supported Rust version: **1.74**

## Quality and security checks

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo deny check
cargo audit
```

## Shell completions and man page

Generate completion files and a man page:

```bash
cargo run --bin generate-assets
```

Generated files:

- `dist/completions/` for bash, zsh, fish, powershell
- `dist/man/livetree.1`

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow, testing, and PR guidelines.

Project behavior standards are documented in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Security

See [SECURITY.md](SECURITY.md) for vulnerability reporting instructions.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes and version history.

## License

MIT — see [LICENSE](LICENSE).

