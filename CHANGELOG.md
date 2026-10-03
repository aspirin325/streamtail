# Changelog

All notable changes to this project will be documented in this file.

The format follows Keep a Changelog style, and this project uses SemVer.

## [Unreleased]

## [0.5.3] - 2026-10-03

### Fixed

- Read the complete HTTP request body in integration tests to avoid intermittent CI failures.

## [0.5.2] - 2026-10-03

### Changed

- Upgrade `ureq` to 3.x, `base64` to 0.23, `regex` to 1.13, and WebPKI roots to 1.x.
- Update GitHub Actions dependencies and migrate the HTTP client to the `ureq` 3 API.

### Fixed

- Upgrade `rustls` to address RUSTSEC-2026-0285.
- Update the Security and CodeQL workflows to use a supported Node.js runtime.

## [0.5.0] - 2026-05-25

### Added

- Initial Rust CLI project structure.
- HTTP polling, append-only diffing, retry handling, and JSON Lines output.
- GitHub CI, release automation, security scanning, dependency policy, and release hardening.

[Unreleased]: https://github.com/aspirin325/streamtail/compare/v0.5.3...HEAD
[0.5.3]: https://github.com/aspirin325/streamtail/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/aspirin325/streamtail/compare/v0.5.0...v0.5.2
[0.5.0]: https://github.com/aspirin325/streamtail/compare/v0.4.0...v0.5.0
