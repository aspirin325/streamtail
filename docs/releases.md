# Releases and Packages

This guide covers installing published packages and cutting a new project
release.

## Versioning

`streamtail` uses SemVer. GitHub release tags must use `vMAJOR.MINOR.PATCH`,
for example `v0.1.0`.

The tag version and `Cargo.toml` package version should match. For tag `v0.1.0`,
`Cargo.toml` should contain:

```toml
version = "0.1.0"
```

## Release Artifacts

The release workflow runs for tags matching `v*.*.*`. It builds Linux artifacts
inside Red Hat UBI 8, Oracle Linux 8, Debian 12, and Ubuntu 24.04 containers.

Each release publishes:

- binary tarballs
- RPM packages for Red Hat and Oracle Linux
- DEB packages for Debian and Ubuntu
- SHA-256 checksum files
- SPDX JSON SBOM files
- GitHub artifact attestations for release provenance

The artifact names include the release tag and platform suffix:

```text
streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.tar.gz
streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.rpm
streamtail-v0.1.0-x86_64-oraclelinux8-linux-gnu.tar.gz
streamtail-v0.1.0-x86_64-oraclelinux8-linux-gnu.rpm
streamtail-v0.1.0-x86_64-debian12-linux-gnu.tar.gz
streamtail-v0.1.0-x86_64-debian12-linux-gnu.deb
streamtail-v0.1.0-x86_64-ubuntu2404-linux-gnu.tar.gz
streamtail-v0.1.0-x86_64-ubuntu2404-linux-gnu.deb
```

## Install RPM Packages

Download the RPM and checksum for your system from the GitHub release page.

Red Hat or RHEL-compatible systems:

```sh
curl -LO https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.rpm
curl -LO https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.rpm.sha256
sha256sum -c streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.rpm.sha256
sudo dnf install ./streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.rpm
streamtail --help
man streamtail
```

Oracle Linux:

```sh
curl -LO https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-oraclelinux8-linux-gnu.rpm
curl -LO https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-oraclelinux8-linux-gnu.rpm.sha256
sha256sum -c streamtail-v0.1.0-x86_64-oraclelinux8-linux-gnu.rpm.sha256
sudo dnf install ./streamtail-v0.1.0-x86_64-oraclelinux8-linux-gnu.rpm
streamtail --help
man streamtail
```

## Install DEB Packages

Download the DEB and checksum for your system from the GitHub release page.

Debian 12:

```sh
curl -LO https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-debian12-linux-gnu.deb
curl -LO https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-debian12-linux-gnu.deb.sha256
sha256sum -c streamtail-v0.1.0-x86_64-debian12-linux-gnu.deb.sha256
sudo apt install ./streamtail-v0.1.0-x86_64-debian12-linux-gnu.deb
streamtail --help
man streamtail
```

Ubuntu 24.04:

```sh
curl -LO https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-ubuntu2404-linux-gnu.deb
curl -LO https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-ubuntu2404-linux-gnu.deb.sha256
sha256sum -c streamtail-v0.1.0-x86_64-ubuntu2404-linux-gnu.deb.sha256
sudo apt install ./streamtail-v0.1.0-x86_64-ubuntu2404-linux-gnu.deb
streamtail --help
man streamtail
```

## Install Binary Tarballs

Use a tarball when you want a manual install without the RPM or DEB package
manager metadata.

```sh
curl -LO https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.tar.gz
curl -LO https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.tar.gz.sha256
sha256sum -c streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.tar.gz.sha256
tar -xzf streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.tar.gz
sudo install -m 0755 streamtail /usr/local/bin/streamtail
streamtail --help
```

## Verify Provenance

Release binaries and packages are attested by GitHub Actions. After downloading
an artifact, verify its attestation with the GitHub CLI:

```sh
gh attestation verify streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.rpm \
  --repo aspirin325/streamtail
```

For SBOM attestations, pass the SPDX predicate type:

```sh
gh attestation verify streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.rpm \
  --repo aspirin325/streamtail \
  --predicate-type https://spdx.dev/Document/v2.3
```

## Build From Source

Install Rust 1.86 or newer, then build from a checked-out release tag:

```sh
git clone https://github.com/aspirin325/streamtail.git
cd streamtail
git checkout v0.1.0
cargo build --locked --release --bin streamtail
./target/release/streamtail --help
```

Run the local verification suite before using a locally built binary:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
```

## Maintainer Release Checklist

1. Update `Cargo.toml` to the new SemVer version without the leading `v`.
2. Update `Cargo.lock` if Cargo changes the package metadata.
3. Update `CHANGELOG.md` with the release notes.
4. Run the local checks:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release --bin streamtail
```

5. Create and push the release tag:

```sh
git tag -a v0.1.0 -m "streamtail v0.1.0"
git push origin v0.1.0
```

6. Wait for the `Release` workflow to finish.
7. Confirm the GitHub release contains tarballs, RPMs, DEBs, checksums, SPDX
   SBOMs, and generated release notes.
8. Download one RPM or DEB and verify its checksum and GitHub attestation.
