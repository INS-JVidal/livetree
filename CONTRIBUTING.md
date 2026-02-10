# Contributing

Thanks for your interest in contributing to `livetree`.

## How to contribute

1. Fork the repository and create a branch from `main`.
2. Keep changes focused and small whenever possible.
3. Add or update tests for behavioral changes.
4. Open a PR with:
   - clear description of the problem
   - approach and trade-offs
   - test evidence

## Development setup

```bash
cargo build
cargo test
```

## Required checks before PR

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

If you changed dependency constraints or added crates, also run:

```bash
cargo deny check
cargo audit
```

## Coding guidelines

- Follow idiomatic Rust style and ownership patterns.
- Avoid `unwrap()` / `expect()` in production paths.
- Keep `main.rs` thin; prefer modules for business logic.
- Preserve CLI compatibility unless the change is explicitly breaking.

## Reporting bugs

Please include:

- OS and architecture
- command used
- expected behavior
- actual behavior
- relevant stderr/stdout output

## Security issues

Do **not** open a public issue for vulnerabilities.  
See [SECURITY.md](SECURITY.md) for private disclosure instructions.

