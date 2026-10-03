# Changelog

All notable changes to this project will be documented in this file.

The format follows Keep a Changelog style, and this project uses SemVer.

## [Unreleased]

## [0.5.1] - 2026-10-03

### Fixed

- Upgrade `rustls` to address RUSTSEC-2026-0285.
- Update the Security and CodeQL workflows to use a supported Node.js runtime.

## [0.5.0] - 2026-05-25

### Added

- Initial Rust CLI project structure.
- HTTP polling, append-only diffing, retry handling, and JSON Lines output.
- GitHub CI, release automation, security scanning, dependency policy, and release hardening.

[Unreleased]: https://github.com/aspirin325/streamtail/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/aspirin325/streamtail/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/aspirin325/streamtail/compare/v0.4.0...v0.5.0
