# Contributing

Thanks for helping improve `streamtail`.

## Development Setup

Install Rust 1.86 or newer:

```sh
rustup toolchain install stable
rustup component add rustfmt clippy llvm-tools-preview
```

Run the local verification suite before opening a pull request:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release --bin streamtail
```

Optional security and coverage checks:

```sh
cargo install --locked cargo-deny
cargo install --locked cargo-llvm-cov
cargo install --locked cargo-fuzz
cargo deny --all-features check
cargo llvm-cov --locked --workspace --all-targets --all-features
cargo fuzz run cli_parse_duration -- -max_total_time=30
```

## Pull Requests

- Keep changes focused and explain the user-visible behavior.
- Add or update tests for behavioral changes.
- Update `README.md` or `docs/` when commands, architecture, or release behavior changes.
- Do not commit `target/` or local tool caches.

## Commit Style

Use clear, imperative commit subjects, for example:

```text
Add HTTP retry policy tests
Document release verification
```
