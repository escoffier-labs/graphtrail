# Releasing GraphTrail

GraphTrail has two release paths for an existing version tag:

- `.github/workflows/release-binaries.yml` builds native `graphtrail` and `graphtrail-mcp` binaries on GitHub-hosted runners, bundles them with `checksums.txt` in a read-only `bundle` job, and attaches platform assets to the GitHub release from that bundle.
- `.github/workflows/publish.yml` publishes the crate to crates.io. The workflow is manual, runs in the `release` environment, packages the exact tagged source before requesting credentials, and uses crates.io Trusted Publishing for a short-lived token.

Ordinary pull-request CI never receives publication credentials.

## Native binary release

Tag push (`v*`) or manual dispatch from `refs/heads/master` triggers `.github/workflows/release-binaries.yml`. Pull requests that touch release inputs run the same five-target build matrix plus the read-only bundle job without write permissions.

The workflow builds both binaries on native GitHub-hosted runners paired one-to-one with each asset:

| Runner | Asset key | Binary names |
|---|---|---|
| `ubuntu-22.04` | `linux-amd64` | `graphtrail-linux-amd64`, `graphtrail-mcp-linux-amd64` |
| `ubuntu-22.04-arm` | `linux-arm64` | `graphtrail-linux-arm64`, `graphtrail-mcp-linux-arm64` |
| `macos-15-intel` | `darwin-amd64` | `graphtrail-darwin-amd64`, `graphtrail-mcp-darwin-amd64` |
| `macos-15` | `darwin-arm64` | `graphtrail-darwin-arm64`, `graphtrail-mcp-darwin-arm64` |
| `windows-latest` | `windows-amd64` | `graphtrail-windows-amd64.exe`, `graphtrail-mcp-windows-amd64.exe` |

Steps:

1. Build with default Cargo features using `cargo build --locked --release` on each native runner (no cross-compilation).
2. Package assets under `dist/` and smoke them from those paths before upload.
3. Download all ten matrix artifacts in the `bundle` job, validate the exact inventory, generate `checksums.txt`, verify all ten digests, and upload one `release-bundle` artifact.
4. In the publish job, download `release-bundle`, run `scripts/release-preflight.sh` against the tagged source, upload only missing assets to the GitHub release (never clobber existing files), and re-download every asset with `scripts/release-smoke.sh`.

### Backfill an existing tag

To attach binaries to an immutable tag such as `v0.4.0` without moving the tag:

```bash
gh workflow run release-binaries.yml --ref master -f tag=v0.4.0
```

Manual `workflow_dispatch` runs publish only from `refs/heads/master`.

### Verify a download independently

```bash
verify_dir="$(mktemp -d)"
trap 'rm -rf "$verify_dir"' EXIT
gh release download vX.Y.Z --dir "$verify_dir"
( cd "$verify_dir" && sha256sum -c checksums.txt )
```

## One-time setup

1. Create a protected GitHub environment named `release` and limit deployments to the default branch.
2. In the `graphtrail` crate settings on crates.io, add a GitHub Trusted Publishing configuration for owner `escoffier-labs`, repository `graphtrail`, workflow `publish.yml`, and environment `release`.
3. Keep the workflow filename and environment aligned with that crates.io configuration. No long-lived registry token is needed.

## Publish a crate tag

Before creating a tag, set the package version in `Cargo.toml`, add the matching `CHANGELOG.md` section, run the full repository gate, and merge the release commit. Create the version tag on that exact commit, then dispatch the workflow from the default branch:

```bash
gh workflow run publish.yml --ref master -f tag=vX.Y.Z
```

The workflow refuses a tag that disagrees with the manifest or changelog, refuses a checkout not pointed to by that tag, runs `cargo package --locked`, publishes, and polls crates.io until the exact version is visible. For the existing v0.3.0 GitHub release, dispatch `tag=v0.3.0`; the workflow packages the tagged commit rather than current `master`.

## Recovery after a partial release

If package publication fails after the tag or GitHub release exists, do not move or recreate the tag. Released source identity is immutable.

- If the exact version is absent from crates.io, correct the environment or Trusted Publishing configuration and rerun the workflow with the same tag.
- If the exact version is present on crates.io, do not publish again. Confirm the registry response and treat the failed verification step as recovered.

If binary upload fails after some assets landed, rerun `release-binaries.yml` with the same tag. The workflow skips assets that already exist and only uploads missing files.

Never reuse a version for different source. If tagged source itself is wrong, leave the tag and release as historical records, fix forward with a new patch version, and document the superseding release.
