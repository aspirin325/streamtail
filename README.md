# streamtail

`streamtail` is a terminal utility for following changing web content, similar to
`tail -f` for URLs. It polls HTTP/HTTPS endpoints, detects appended content, and
prints only the new text after the first snapshot.

The first implementation slice focuses on basic HTTP polling, incremental diffing,
timeouts, and retry handling. SSE, NDJSON, chunked streaming, and WebSocket support
are tracked as later roadmap phases in `docs/`.

## Usage

```sh
streamtail https://example.com/logs
streamtail https://example.com/api/events --interval 2s
streamtail https://example.com/logs --json
```

Useful options:

```text
-i, --interval <DURATION>     Poll interval, for example 500ms, 2s, 1m
    --timeout <DURATION>      HTTP timeout per request
    --max-retries <N|unlimited>
                              Retry limit for consecutive retryable failures
    --once                    Fetch once and exit
    --json                    Emit each update as a JSON line
```

## Build on Linux

Install Rust 1.86 or newer and native build tools:

```sh
sudo dnf install -y gcc ca-certificates curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Build and run the binary:

```sh
cargo build --release
./target/release/streamtail --help
```

Run tests and checks:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release --bin streamtail
```

## Project Layout

```text
src/main.rs          CLI binary entry point
src/lib.rs           Public library surface and module exports
src/app.rs           Application run loop and top-level errors
src/cli/             Argument parsing and CLI configuration
src/fetcher/         HTTP fetching, timeout, and status handling
src/diff/            Snapshot diffing for appended content
src/renderer/        Text and JSON Lines output
src/retry/           Retry limits and exponential backoff
tests/               Integration tests
docs/                Product spec, architecture, roadmap, and testing notes
.github/workflows/   CI and release automation
```

## Pull Request Checks

Pull requests run the same gates expected before merge:

- formatting with `cargo fmt`
- linting with `cargo clippy -D warnings`
- tests on stable Rust
- tests on the declared MSRV, Rust 1.86
- release build verification
- coverage report generation
- dependency advisory, license, ban, and source checks with `cargo-deny`
- CodeQL Rust analysis

## Releases

The package uses SemVer. Release tags should use the recommended GitHub release
format `vMAJOR.MINOR.PATCH`, for example `v0.1.0`.

Pushing a matching tag starts the release workflow:

```sh
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions builds release binaries inside Red Hat UBI 8 and Oracle Linux 8
containers, then publishes:

- binary tarballs
- SHA-256 checksum files
- SPDX JSON SBOM files
- GitHub artifact attestations for release provenance

Verify a downloaded tarball:

```sh
sha256sum -c streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.sha256
gh attestation verify streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.tar.gz \
  --repo aspirin325/streamtail
```
