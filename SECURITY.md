# Security Policy

## Supported Versions

Security fixes are provided for the latest released version of `streamtail`.
Pre-1.0 releases may include breaking changes when needed to fix security issues.

## Reporting a Vulnerability

Please report suspected vulnerabilities privately through GitHub Security
Advisories for this repository.

If GitHub Security Advisories are unavailable, contact the maintainer directly
before opening a public issue. Include:

- affected version or commit
- reproduction steps
- expected and observed impact
- any known mitigations

## Security Checks

Pull requests and protected branches run:

- `cargo fmt`
- `cargo clippy`
- `cargo test`
- MSRV tests
- release build verification
- dependency advisory, license, ban, and source checks via `cargo-deny`
- CodeQL scanning

Release builds publish SHA-256 checksums, SBOM files, and GitHub artifact
attestations.
