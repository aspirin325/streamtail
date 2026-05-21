# Architecture

## Components

### App
Responsible for:
- Wiring CLI configuration into runtime behavior
- Owning the polling loop
- Coordinating fetch, diff, render, and retry decisions

### Fetcher
Responsible for:
- HTTP polling
- Streaming connections
- Retry logic
- Compression

### Diff Engine
Responsible for:
- Detecting appended text
- Incremental diffs
- Stateful snapshots

### Renderer
Responsible for:
- ANSI colors
- JSON formatting
- stdout buffering

### CLI Layer
Responsible for:
- Parsing flags
- Config loading
- Output modes

## Source layout

- `src/main.rs`: binary entry point
- `src/lib.rs`: public library exports
- `src/app.rs`: application orchestration
- `src/cli/`: argument parsing and configuration
- `src/fetcher/`: HTTP client behavior
- `src/diff/`: incremental diff engine
- `src/renderer/`: terminal and pipe output
- `src/retry/`: retry policy and backoff
- `docs/man/streamtail.1`: manual page installed by Linux packages
- `fuzz/`: cargo-fuzz targets used by ClusterFuzzLite

## Quality gates

- Pull request CI runs formatting, linting, tests, MSRV verification, coverage, and release build checks.
- Fuzz CI runs ClusterFuzzLite against cargo-fuzz targets.
- Security CI runs dependency advisory, license, ban, and source checks with `cargo-deny`.
- CodeQL scans Rust code on pull requests, protected branch pushes, scheduled runs, and manual runs.
- OpenSSF Scorecard runs on protected branch pushes, scheduled runs, and manual runs.
- Release builds publish tarballs, a Windows ZIP, RPMs, DEBs, checksums, SBOMs, GitHub artifact attestations, and release-attached Sigstore bundles.
