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

This matters because the compiled `streamtail --version` output and RPM/DEB
package metadata come from `Cargo.toml`, not from the Git tag. The release
workflow fails before building if the tag version and `Cargo.toml` version do
not match.

## Release Artifacts

The release workflow runs for tags matching `v*.*.*`. It builds Linux artifacts
inside Red Hat UBI 8, Oracle Linux 8, Debian 12, and Ubuntu 24.04 containers,
and builds a Windows artifact on the GitHub-hosted Windows runner.

Each release publishes:

- binary tarballs
- Windows ZIP archives
- RPM packages for Red Hat and Oracle Linux
- DEB packages for Debian and Ubuntu
- SHA-256 checksum files
- SPDX JSON SBOM files
- GitHub artifact attestations for release provenance
- Release-attached Sigstore attestation bundles ending in `.sigstore.json`

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
streamtail-v0.1.0-x86_64-windows-msvc.zip
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

## Install Windows ZIP Archives

Download the Windows ZIP and checksum from the GitHub release page. In
PowerShell:

```powershell
Invoke-WebRequest `
  -Uri https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-windows-msvc.zip `
  -OutFile streamtail-v0.1.0-x86_64-windows-msvc.zip
Invoke-WebRequest `
  -Uri https://github.com/aspirin325/streamtail/releases/download/v0.1.0/streamtail-v0.1.0-x86_64-windows-msvc.zip.sha256 `
  -OutFile streamtail-v0.1.0-x86_64-windows-msvc.zip.sha256

$expected = (Get-Content .\streamtail-v0.1.0-x86_64-windows-msvc.zip.sha256).Split()[0]
$actual = (Get-FileHash -Algorithm SHA256 .\streamtail-v0.1.0-x86_64-windows-msvc.zip).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw "Checksum mismatch" }

Expand-Archive .\streamtail-v0.1.0-x86_64-windows-msvc.zip -DestinationPath .\streamtail
.\streamtail\streamtail-v0.1.0-x86_64-windows-msvc\streamtail.exe --help
```

## Verify Provenance

Release binaries and packages are attested by GitHub Actions. After downloading
an artifact, verify its attestation with the GitHub CLI:

```sh
gh attestation verify streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.rpm \
  --repo aspirin325/streamtail
```

On Windows, verify the ZIP artifact from PowerShell:

```powershell
gh attestation verify .\streamtail-v0.1.0-x86_64-windows-msvc.zip `
  --repo aspirin325/streamtail
```

For SBOM attestations, pass the SPDX predicate type:

```sh
gh attestation verify streamtail-v0.1.0-x86_64-redhat-ubi8-linux-gnu.rpm \
  --repo aspirin325/streamtail \
  --predicate-type https://spdx.dev/Document/v2.3
```

The release also includes `.provenance.sigstore.json` and `.sbom.sigstore.json`
sidecar files. These are the same generated attestation bundles attached as
release assets so tools such as OpenSSF Scorecard can detect that the release is
signed or has provenance.

Releases created before these sidecar files existed will still be counted by
OpenSSF Scorecard until they fall outside its recent-release window or are
backfilled with matching signature or provenance assets.

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

1. Run the `Prepare Release` workflow from GitHub Actions with the new SemVer
   version without the leading `v`, for example `0.2.0`.
2. Review the generated pull request. It updates `Cargo.toml`, refreshes
   `Cargo.lock`, and updates the man page version.
3. Update `CHANGELOG.md` in that pull request with the release notes.
4. Let pull request CI pass, then merge the release preparation pull request.
5. Tag the merge commit with the matching `vMAJOR.MINOR.PATCH` tag.

If preparing the release manually instead, update `Cargo.toml` to the new SemVer
version without the leading `v`. This is the version shown by
`streamtail --version` and embedded in package metadata. Refresh `Cargo.lock`
and update `CHANGELOG.md` before tagging.

The `Prepare Release` workflow uses the default `GITHUB_TOKEN` unless a
`RELEASE_PR_TOKEN` secret exists. Use that optional secret for a fine-scoped
GitHub token if your repository settings require PRs created by automation to
trigger pull request CI automatically.

Run the local checks before tagging:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release --bin streamtail
```

Create and push the release tag:

```sh
git tag -a v0.1.0 -m "streamtail v0.1.0"
git push origin v0.1.0
```

Wait for the `Release` workflow to finish, then confirm the GitHub release
contains tarballs, the Windows ZIP, RPMs, DEBs, checksums, SPDX SBOMs, Sigstore
attestation bundles, and generated release notes. Download one Linux package
and the Windows ZIP, then verify their checksums and GitHub attestations.
