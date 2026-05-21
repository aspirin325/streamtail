# streamtail

[![CI](https://github.com/aspirin325/streamtail/actions/workflows/ci.yml/badge.svg)](https://github.com/aspirin325/streamtail/actions/workflows/ci.yml)
[![Security](https://github.com/aspirin325/streamtail/actions/workflows/security.yml/badge.svg)](https://github.com/aspirin325/streamtail/actions/workflows/security.yml)
[![CodeQL](https://github.com/aspirin325/streamtail/actions/workflows/codeql.yml/badge.svg)](https://github.com/aspirin325/streamtail/actions/workflows/codeql.yml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/aspirin325/streamtail/badge)](https://securityscorecards.dev/viewer/?uri=github.com/aspirin325/streamtail)
[![Release](https://img.shields.io/github/v/release/aspirin325/streamtail?sort=semver)](https://github.com/aspirin325/streamtail/releases)
[![License](https://img.shields.io/github/license/aspirin325/streamtail)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.86-orange)](Cargo.toml)

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

## Build on macOS

Install Apple command-line tools and Rust 1.86 or newer:

```sh
xcode-select --install
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

## Build on Windows

Install Visual Studio Build Tools 2022 with the `Desktop development with C++`
workload, then install Rust 1.86 or newer. In PowerShell:

```powershell
winget install --id Rustlang.Rustup -e
rustup default stable
rustup update
```

Build and run the binary:

```powershell
cargo build --release
.\target\release\streamtail.exe --help
```

Run tests and checks:

```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release --bin streamtail
```

On Unix-like systems, view the manual page from the source tree:

```sh
man ./docs/man/streamtail.1
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
docs/releases.md     Release, package, and install guide
docs/man/            Manual pages
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
- ClusterFuzzLite fuzzing
- dependency advisory, license, ban, and source checks with `cargo-deny`
- CodeQL Rust analysis

## Releases And Packages

Release artifacts include Linux binary tarballs, RPM packages, DEB packages,
checksums, SPDX SBOMs, and GitHub artifact attestations. See
[`docs/releases.md`](docs/releases.md) for installation commands, verification
steps, and the maintainer release checklist.
