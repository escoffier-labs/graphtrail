# Native Release Binaries Plan (Brigade issue #354)

**Goal:** Ship native `graphtrail` and `graphtrail-mcp` binaries for five GitHub-hosted targets through `.github/workflows/release-binaries.yml`, backfill the existing mutable `v0.4.0` GitHub release, and lock the contract in `tests/repository_contract.rs`.

**Architecture:** A separate binary-release workflow builds on native hosted runners (no cross-compilation, no self-hosted runners), bundles ten platform-named binaries plus `checksums.txt` in a read-only `bundle` job that always runs after the matrix, and publishes from that bundle on tag push or `workflow_dispatch` from `master` only. Pull requests exercise the full build matrix plus bundle with `contents: read` only. Crates.io publication stays in `publish.yml`.

**Key tech:** GitHub Actions matrix (`ubuntu-22.04`, `ubuntu-22.04-arm`, `macos-15-intel`, `macos-15`, `windows-latest`), `cargo build --locked --release` with default features, `gh release upload` without clobber, `sha256sum` checksums, repository contract tests in Rust.

**Agent instruction:** Execute task-by-task in order. Check each box. Commit after each task. Do not skip the RED step.

## Pressure-test decision record

| Decision | Choice | Basis |
|---|---|---|
| Workflow file | `.github/workflows/release-binaries.yml` (separate from `publish.yml`) | `evidence+judgment` |
| Build strategy | Native hosted matrix, no `--target` cross-compilation | `evidence+judgment` |
| Runner availability | GitHub-hosted `ubuntu-22.04`, `ubuntu-22.04-arm`, `macos-15-intel`, `macos-15`, `windows-latest` | `evidence` |
| Native dependency build | tree-sitter and transitive crates need a C compiler on each runner | `evidence` |
| Runner ↔ asset map | `linux-amd64`=`ubuntu-22.04`, `linux-arm64`=`ubuntu-22.04-arm`, `darwin-amd64`=`macos-15-intel`, `darwin-arm64`=`macos-15`, `windows-amd64`=`windows-latest` | `evidence+judgment` |
| Windows toolchain | Assert `rustc -vV` host ends in `pc-windows-msvc` before build | `stated-constraint` |
| Self-hosted runners | Not used | `stated-constraint` |
| Cargo features | Default features only (`watch` on) | `evidence+judgment` |
| Asset names | `graphtrail-<slug>` and `graphtrail-mcp-<slug>`; `.exe` on Windows only | `stated-constraint` |
| Release payload | Exactly ten binaries + `checksums.txt` (11 assets) | `stated-constraint` |
| Bundle job | Always runs after matrix; read-only; produces `release-bundle` artifact | `evidence+judgment` |
| PR behavior | Full matrix build/smoke/bundle; no `contents: write` | `evidence+judgment` |
| Publish triggers | `push: tags: v*` and `workflow_dispatch` with `tag` input (dispatch only from `refs/heads/master`) | `evidence+judgment` |
| `v0.4.0` backfill | Manual dispatch attaches assets to the existing published release while GitHub reports `immutable=false` | `evidence+judgment` + `evidence` (release exists with zero assets; API `immutable=false`) |
| Upload policy | Skip assets that already exist; never `--clobber` | `evidence+judgment` |
| Release publication | Fresh releases are created as drafts, smoke-tested, then published; existing mutable releases (for example `v0.4.0` with `immutable=false`) keep the backfill path | `evidence` (GitHub API reports `v0.4.0` `immutable=false`) |
| Immutable published releases | Cannot be backfilled; fix forward with a new patch version | `evidence+judgment` |
| Concurrency | Same tag group via `github.event.inputs.tag \|\| github.ref_name` with `cancel-in-progress: false` | `evidence+judgment` |
| Asset inventory lookup | Single `gh release view` before the upload loop; lookup failure aborts | `evidence+judgment` |
| Windows MSVC check | Coerce `rustc -vV` with `Out-String` before scalar `-notmatch` | `evidence` |
| Post-upload verify | Re-download all release assets; `sha256sum -c checksums.txt` | `stated-constraint` |
| Test order | Extend `tests/repository_contract.rs` first (RED), then workflow, then docs contract (RED), then docs | `stated-constraint` |

## File map

| File | Responsibility |
|---|---|
| `.github/workflows/release-binaries.yml` | Matrix build, bundle job, PR-safe permissions, conditional publish/upload/verify |
| `scripts/release-smoke.sh` | Download all release assets for a tag and verify checksums plus a Linux execution smoke |
| `tests/repository_contract.rs` | Workflow helper parsers plus binary and documentation contract tests |
| `docs/releasing.md` | Document binary release path, runner map, backfill dispatch, checksum verification |
| `CHANGELOG.md` | Record native binary release workflow under `[Unreleased]` |

Unchanged: `.github/workflows/publish.yml`, `scripts/release-preflight.sh`, `scripts/verify-crates-version.sh`.

## Asset inventory (11 release files)

| Asset | Runner |
|---|---|
| `graphtrail-linux-amd64` | `ubuntu-22.04` |
| `graphtrail-mcp-linux-amd64` | `ubuntu-22.04` |
| `graphtrail-linux-arm64` | `ubuntu-22.04-arm` |
| `graphtrail-mcp-linux-arm64` | `ubuntu-22.04-arm` |
| `graphtrail-darwin-amd64` | `macos-15-intel` |
| `graphtrail-mcp-darwin-amd64` | `macos-15-intel` |
| `graphtrail-darwin-arm64` | `macos-15` |
| `graphtrail-mcp-darwin-arm64` | `macos-15` |
| `graphtrail-windows-amd64.exe` | `windows-latest` |
| `graphtrail-mcp-windows-amd64.exe` | `windows-latest` |
| `checksums.txt` | bundle job (`ubuntu-latest`) |

---

### Task 1: Repository contract test (RED)

**Files:**
- Modify: `tests/repository_contract.rs`

- [x] Add workflow helper functions immediately after the imports:

```rust
fn release_workflow_job<'a>(workflow: &'a str, job: &str) -> &'a str {
    let marker = format!("  {job}:");
    let start = workflow
        .find(&marker)
        .unwrap_or_else(|| panic!("release workflow must declare the {job} job"));
    let rest = &workflow[start + marker.len()..];
    let end = match job {
        "build" => rest
            .find("\n  bundle:")
            .unwrap_or_else(|| panic!("release workflow must declare a bundle job after build")),
        "bundle" => rest
            .find("\n  publish:")
            .unwrap_or_else(|| panic!("release workflow must declare a publish job after bundle")),
        "publish" => rest.len(),
        other => panic!("unsupported release workflow job: {other}"),
    };
    &rest[..end]
}

fn release_matrix_os_asset_pairs(workflow: &str) -> Vec<(String, String)> {
    let build = release_workflow_job(workflow, "build");
    let include = build
        .split("include:")
        .nth(1)
        .expect("release build job must declare a matrix include");
    let include = include
        .split("steps:")
        .next()
        .expect("matrix include must precede job steps");

    let mut pairs = Vec::new();
    let mut current_os = None;
    for line in include.lines() {
        let trimmed = line.trim();
        let runner = trimmed
            .strip_prefix("- os: ")
            .or_else(|| trimmed.strip_prefix("os: "));
        if let Some(os) = runner {
            current_os = Some(os.to_string());
        } else if let Some(asset) = trimmed.strip_prefix("asset: ") {
            let os = current_os
                .take()
                .expect("matrix asset must follow its runner label");
            pairs.push((os, asset.to_string()));
        }
    }
    pairs
}

fn release_job_permissions(workflow: &str, job: &str) -> String {
    let section = release_workflow_job(workflow, job);
    let permissions = section
        .split("permissions:")
        .nth(1)
        .unwrap_or_else(|| panic!("release {job} job must declare permissions"));
    permissions
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("contents:"))
        .unwrap_or_else(|| panic!("release {job} job must declare contents permissions"))
        .to_string()
}
```

- [x] Add `HashMap` to the `use` line:

```rust
use std::{collections::HashMap, collections::HashSet, fs, path::Path};
```

- [x] Append the failing test before `agent_startup_requires_skill_selection_before_brigade_commands`:

```rust
#[test]
fn binary_release_attaches_native_assets_with_checksums() {
    assert!(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".github/workflows/release-binaries.yml")
            .is_file(),
        "binary release contract must include .github/workflows/release-binaries.yml"
    );
    assert!(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/release-smoke.sh")
            .is_file(),
        "binary release contract must include scripts/release-smoke.sh"
    );

    let workflow = repository_file(".github/workflows/release-binaries.yml");
    for required in [
        "pull_request:",
        "push:",
        "tags:",
        "- \"v*\"",
        "workflow_dispatch:",
        "os: ubuntu-22.04",
        "os: ubuntu-22.04-arm",
        "os: macos-15-intel",
        "os: macos-15",
        "os: windows-latest",
        "cargo build --locked --release",
        "--bin graphtrail",
        "--bin graphtrail-mcp",
        "asset: linux-amd64",
        "asset: linux-arm64",
        "asset: darwin-amd64",
        "asset: darwin-arm64",
        "asset: windows-amd64",
        "dist/graphtrail-${{ matrix.asset }}",
        "dist/graphtrail-mcp-${{ matrix.asset }}",
        r#"ext: ".exe""#,
        "pc-windows-msvc",
        "needs: build",
        "needs: bundle",
        "name: release-bundle",
        "graphtrail-linux-amd64",
        "graphtrail-mcp-linux-amd64",
        "graphtrail-linux-arm64",
        "graphtrail-mcp-linux-arm64",
        "graphtrail-darwin-amd64",
        "graphtrail-mcp-darwin-amd64",
        "graphtrail-darwin-arm64",
        "graphtrail-mcp-darwin-arm64",
        "graphtrail-windows-amd64.exe",
        "graphtrail-mcp-windows-amd64.exe",
        "checksums.txt",
        "sha256sum graphtrail-* > checksums.txt",
        "sha256sum -c checksums.txt",
        "scripts/release-preflight.sh",
        "scripts/release-smoke.sh",
        "--verify-tag",
        "refs/heads/master",
        "if: github.event_name != 'pull_request'",
    ] {
        assert!(
            workflow.contains(required),
            "release-binaries workflow must preserve the native asset contract: {required}"
        );
    }
    assert!(
        !workflow.contains("--target"),
        "release-binaries workflow must not cross-compile with --target"
    );
    assert!(
        !workflow.contains("self-hosted"),
        "release-binaries workflow must use only GitHub-hosted runners"
    );
    assert!(
        !workflow.contains("--clobber"),
        "release-binaries workflow must not clobber existing release assets"
    );

    let expected_runner_by_asset = HashMap::from([
        ("linux-amd64".to_string(), "ubuntu-22.04".to_string()),
        ("linux-arm64".to_string(), "ubuntu-22.04-arm".to_string()),
        ("darwin-amd64".to_string(), "macos-15-intel".to_string()),
        ("darwin-arm64".to_string(), "macos-15".to_string()),
        ("windows-amd64".to_string(), "windows-latest".to_string()),
    ]);
    let runner_by_asset: HashMap<_, _> = release_matrix_os_asset_pairs(&workflow)
        .into_iter()
        .map(|(os, asset)| (asset, os))
        .collect();
    assert_eq!(
        runner_by_asset, expected_runner_by_asset,
        "release matrix must pair each asset with its native GitHub-hosted runner"
    );

    let bundle = release_workflow_job(&workflow, "bundle");
    assert!(
        bundle.contains("sha256sum graphtrail-* > checksums.txt"),
        "bundle job must generate checksums.txt from the ten graphtrail assets"
    );
    assert!(
        bundle.contains("sha256sum -c checksums.txt"),
        "bundle job must verify all ten digests before upload"
    );
    assert!(
        !release_workflow_job(&workflow, "publish").contains("sha256sum graphtrail-* > checksums.txt"),
        "publish job must download the bundled checksums instead of regenerating them"
    );

    assert_eq!(
        release_job_permissions(&workflow, "build"),
        "contents: read",
        "build job must not receive repository write permissions"
    );
    assert_eq!(
        release_job_permissions(&workflow, "bundle"),
        "contents: read",
        "bundle job must remain read-only"
    );
    assert_eq!(
        release_job_permissions(&workflow, "publish"),
        "contents: write",
        "publish job must retain repository write permissions"
    );
    assert!(
        !workflow
            .split("jobs:")
            .next()
            .expect("release-binaries workflow must declare jobs")
            .contains("contents: write"),
        "workflow-level permissions must not grant contents: write to every job"
    );

    let smoke = repository_file("scripts/release-smoke.sh");
    for required in [
        "graphtrail-linux-amd64",
        "graphtrail-mcp-linux-amd64",
        "checksums.txt",
        "sha256sum -c",
        "--version",
        "initialize",
    ] {
        assert!(
            smoke.contains(required),
            "release smoke script must verify downloaded assets: {required}"
        );
    }
}
```

- [x] Run RED: `brigade work verify run --target . --command "cargo test --test repository_contract binary_release_attaches_native_assets_with_checksums" --capture brigade-work`
- [x] Expect FAIL with messages such as `binary release contract must include .github/workflows/release-binaries.yml` and `binary release contract must include scripts/release-smoke.sh`.
- [x] Commit: `git add tests/repository_contract.rs && git commit -m "test(release): require native binary release contract"`

---

### Task 2: Release smoke script

**Files:**
- Create: `scripts/release-smoke.sh`

- [x] Create `scripts/release-smoke.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

tag="${1:?usage: release-smoke.sh <tag>}"

verify_dir="$(mktemp -d)"
trap 'rm -rf "$verify_dir"' EXIT

gh release download "$tag" --dir "$verify_dir"

asset_count="$(find "$verify_dir" -maxdepth 1 -type f | wc -l | tr -d ' ')"
if [[ "$asset_count" != "11" ]]; then
  echo "expected 11 release assets for $tag, found $asset_count" >&2
  ls -la "$verify_dir" >&2
  exit 1
fi

(
  cd "$verify_dir"
  sha256sum -c checksums.txt
)

chmod +x "$verify_dir/graphtrail-linux-amd64" "$verify_dir/graphtrail-mcp-linux-amd64"
"$verify_dir/graphtrail-linux-amd64" --version
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  | "$verify_dir/graphtrail-mcp-linux-amd64" \
  | grep -F '"name":"graphtrail"'

echo "release smoke passed for $tag"
```

- [x] Make executable: `chmod +x scripts/release-smoke.sh`
- [x] Run RED again: `brigade work verify run --target . --command "cargo test --test repository_contract binary_release_attaches_native_assets_with_checksums" --capture brigade-work`
- [x] Expect FAIL on missing `.github/workflows/release-binaries.yml` only.
- [x] Commit: `git add scripts/release-smoke.sh && git commit -m "feat(release): add post-upload release smoke script"`

---

### Task 3: Release binaries workflow

**Files:**
- Create: `.github/workflows/release-binaries.yml`

- [x] Create `.github/workflows/release-binaries.yml`:

```yaml
name: Release binaries

on:
  pull_request:
    paths:
      - ".github/workflows/release-binaries.yml"
      - "scripts/release-smoke.sh"
      - "tests/repository_contract.rs"
      - "Cargo.toml"
      - "Cargo.lock"
      - "src/**"
  push:
    tags:
      - "v*"
  workflow_dispatch:
    inputs:
      tag:
        description: Existing version tag to attach binaries to, for example v0.4.0
        required: true
        type: string

permissions:
  contents: read

jobs:
  build:
    name: build (${{ matrix.asset }})
    runs-on: ${{ matrix.os }}
    permissions:
      contents: read
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-22.04
            asset: linux-amd64
            ext: ""
          - os: ubuntu-22.04-arm
            asset: linux-arm64
            ext: ""
          - os: macos-15-intel
            asset: darwin-amd64
            ext: ""
          - os: macos-15
            asset: darwin-arm64
            ext: ""
          - os: windows-latest
            asset: windows-amd64
            ext: ".exe"
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event_name == 'workflow_dispatch' && inputs.tag || github.ref }}
          fetch-depth: 0

      - uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.asset }}

      - name: Assert MSVC host toolchain
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          $verbose = rustc -vV
          if ($verbose -notmatch 'host: .+-pc-windows-msvc') {
            throw "expected MSVC Windows host toolchain, got:`n$verbose"
          }

      - name: Build release binaries
        run: cargo build --locked --release --bin graphtrail --bin graphtrail-mcp

      - name: Package native assets
        shell: bash
        run: |
          set -euo pipefail
          mkdir -p dist
          cp "target/release/graphtrail${{ matrix.ext }}" \
            "dist/graphtrail-${{ matrix.asset }}${{ matrix.ext }}"
          cp "target/release/graphtrail-mcp${{ matrix.ext }}" \
            "dist/graphtrail-mcp-${{ matrix.asset }}${{ matrix.ext }}"

      - name: Smoke test packaged binaries
        shell: bash
        run: |
          set -euo pipefail
          "./dist/graphtrail-${{ matrix.asset }}${{ matrix.ext }}" --version
          printf '%s\n' \
            '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
            | "./dist/graphtrail-mcp-${{ matrix.asset }}${{ matrix.ext }}" \
            | grep -F '"name":"graphtrail"'

      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.asset }}
          path: dist/*

  bundle:
    name: bundle release artifacts
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Assemble dist and validate inventory
        shell: bash
        run: |
          set -euo pipefail
          mkdir -p dist
          shopt -s nullglob
          for file in artifacts/*/*; do
            cp "$file" "dist/$(basename "$file")"
          done
          expected=(
            graphtrail-linux-amd64
            graphtrail-mcp-linux-amd64
            graphtrail-linux-arm64
            graphtrail-mcp-linux-arm64
            graphtrail-darwin-amd64
            graphtrail-mcp-darwin-amd64
            graphtrail-darwin-arm64
            graphtrail-mcp-darwin-arm64
            graphtrail-windows-amd64.exe
            graphtrail-mcp-windows-amd64.exe
          )
          actual=($(printf '%s\n' dist/* | LC_ALL=C sort | xargs -n1 basename))
          if [[ "${#actual[@]}" -ne 10 ]]; then
            echo "expected exactly ten binaries, found ${#actual[@]}" >&2
            printf '%s\n' "${actual[@]}" >&2
            exit 1
          fi
          for name in "${expected[@]}"; do
            [[ -f "dist/$name" ]] || { echo "missing bundled asset: $name" >&2; exit 1; }
          done

      - name: Generate and verify checksums
        shell: bash
        run: |
          set -euo pipefail
          cd dist
          sha256sum graphtrail-* > checksums.txt
          sha256sum -c checksums.txt

      - uses: actions/upload-artifact@v4
        with:
          name: release-bundle
          path: dist/*

  publish:
    name: publish and verify
    if: github.event_name != 'pull_request' && (github.event_name != 'workflow_dispatch' || github.ref == 'refs/heads/master')
    needs: bundle
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event_name == 'workflow_dispatch' && inputs.tag || github.ref }}
          fetch-depth: 0

      - name: Resolve release tag
        id: tag
        env:
          EVENT_NAME: ${{ github.event_name }}
          DISPATCH_TAG: ${{ inputs.tag }}
        run: |
          set -euo pipefail
          if [[ "$EVENT_NAME" == "workflow_dispatch" ]]; then
            echo "value=$DISPATCH_TAG" >> "$GITHUB_OUTPUT"
          else
            echo "value=${GITHUB_REF_NAME}" >> "$GITHUB_OUTPUT"
          fi

      - name: Verify release identity
        env:
          TAG: ${{ steps.tag.outputs.value }}
        run: bash scripts/release-preflight.sh "$TAG" .

      - uses: actions/download-artifact@v4
        with:
          name: release-bundle
          path: dist

      - name: Upload missing release assets
        env:
          GH_TOKEN: ${{ github.token }}
          TAG: ${{ steps.tag.outputs.value }}
        shell: bash
        run: |
          set -euo pipefail
          if ! gh release view "$TAG" >/dev/null 2>&1; then
            gh release create "$TAG" --verify-tag --title "$TAG" --notes "Native binary release"
          fi
          for file in dist/*; do
            name="$(basename "$file")"
            if gh release view "$TAG" --json assets --jq '.assets[].name' | grep -Fxq "$name"; then
              echo "skip existing asset: $name"
              continue
            fi
            gh release upload "$TAG" "$file"
          done

      - name: Verify published checksums
        env:
          GH_TOKEN: ${{ github.token }}
          TAG: ${{ steps.tag.outputs.value }}
        run: bash scripts/release-smoke.sh "$TAG"
```

- [x] Run contract test GREEN: `brigade work verify run --target . --command "cargo test --test repository_contract binary_release_attaches_native_assets_with_checksums" --capture brigade-work`
- [x] Expect PASS (`test result: ok. 1 passed`).
- [x] Commit: `git add .github/workflows/release-binaries.yml && git commit -m "feat(release): add native binary release workflow"`

---

### Task 4: Release documentation

**Files:**
- Modify: `tests/repository_contract.rs`
- Modify: `docs/releasing.md`

- [x] Append the documentation contract test before `agent_startup_requires_skill_selection_before_brigade_commands` (after the binary workflow test):

```rust
#[test]
fn binary_release_documentation_covers_native_assets() {
    let recovery = repository_file("docs/releasing.md");
    for required in [
        ".github/workflows/release-binaries.yml",
        "graphtrail-linux-amd64",
        "graphtrail-mcp-linux-amd64",
        "checksums.txt",
        "ubuntu-22.04",
        "ubuntu-22.04-arm",
        "linux-arm64",
        "macos-15-intel",
        "darwin-amd64",
        "macos-15",
        "darwin-arm64",
        "windows-amd64",
        "workflow_dispatch",
        "release-bundle",
        "refs/heads/master",
    ] {
        assert!(
            recovery.contains(required),
            "release guide must document native binary assets: {required}"
        );
    }
}
```

- [x] Run documentation contract RED: `brigade work verify run --target . --command "cargo test --test repository_contract binary_release_documentation_covers_native_assets" --capture brigade-work`
- [x] Expect FAIL with `release guide must document native binary assets`.

- [x] Replace the file contents of `docs/releasing.md` with:

```markdown
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
```

- [x] Run documentation contract GREEN: `brigade work verify run --target . --command "cargo test --test repository_contract binary_release_documentation_covers_native_assets" --capture brigade-work`
- [x] Commit: `git add tests/repository_contract.rs docs/releasing.md && git commit -m "docs(release): document native binary release workflow"`

---

### Task 5: Changelog entry

**Files:**
- Modify: `CHANGELOG.md`

- [x] Under `## [Unreleased]`, add:

```markdown
### Added
- Native binary releases through `.github/workflows/release-binaries.yml` for `linux-amd64`, `linux-arm64`, `darwin-amd64`, `darwin-arm64`, and `windows-amd64`, with `graphtrail-<platform>` and `graphtrail-mcp-<platform>` assets plus `checksums.txt`.
```

- [x] Commit: `git add CHANGELOG.md && git commit -m "docs(changelog): note native binary release workflow"`

---

### Task 6: Repository gate

- [x] Run full gate through Brigade:

```bash
brigade work verify run --target . --command "cargo fmt --check" --capture brigade-work
brigade work verify run --target . --command "cargo clippy --all-targets --all-features -- -D warnings" --capture brigade-work
brigade work verify run --target . --command "cargo test --all-features" --capture brigade-work
brigade work verify run --target . --command "cargo build --release" --capture brigade-work
```

- [x] Expect all four commands exit 0.

---

### Task 7: Pull request and five-target green proof

- [ ] Push branch: `git push -u origin HEAD`
- [ ] Open PR:

```bash
gh pr create --title "feat(release): native binary releases for five hosted targets" --body "$(cat <<'EOF'
## Summary
- Add `.github/workflows/release-binaries.yml` with a native five-target matrix, read-only bundle job, and checksum-verified GitHub release uploads.
- Lock the contract in `tests/repository_contract.rs` and document backfill dispatch in `docs/releasing.md`.

Refs escoffier-labs/brigade#354

## Test plan
- [ ] `cargo test --test repository_contract binary_release_attaches_native_assets_with_checksums`
- [ ] `cargo test --test repository_contract binary_release_documentation_covers_native_assets`
- [ ] PR `Release binaries` workflow shows five green `build (...)` jobs and one green `bundle release artifacts` job
- [ ] After merge, manual dispatch `tag=v0.4.0` uploads 11 assets and passes post-upload smoke

EOF
)"
```

- [ ] Wait for PR checks. Confirm all five matrix jobs and the bundle job are green:

```bash
gh pr checks --watch
```

- [ ] Record the PR run URL and each build job link:

```bash
RUN_ID="$(gh run list --workflow release-binaries.yml --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run view "$RUN_ID" --json url,jobs --jq -r '.url, (.jobs[] | select(.name | startswith("build")) | "\(.name): \(.url)")'
```

- [ ] Expect five lines ending in successful `build (...)` jobs for `build (linux-amd64)`, `build (linux-arm64)`, `build (darwin-amd64)`, `build (darwin-arm64)`, and `build (windows-amd64)`, plus a successful `bundle release artifacts` job.

- [ ] Do not merge until every required check is green.

---

### Task 8: Issue #354 evidence comment

- [ ] Derive the workflow run URL and each build job URL, then post the runner-vs-cross decision record to Brigade issue #354:

```bash
RUN_ID="$(gh run list --workflow release-binaries.yml --branch "$(git branch --show-current)" --limit 1 --json databaseId --jq '.[0].databaseId')"
RUN_URL="$(gh run view "$RUN_ID" --json url --jq -r '.url')"
LINUX_AMD64_URL="$(gh run view "$RUN_ID" --json jobs --jq -r '.jobs[] | select(.name == "build (linux-amd64)") | .url')"
LINUX_ARM64_URL="$(gh run view "$RUN_ID" --json jobs --jq -r '.jobs[] | select(.name == "build (linux-arm64)") | .url')"
DARWIN_AMD64_URL="$(gh run view "$RUN_ID" --json jobs --jq -r '.jobs[] | select(.name == "build (darwin-amd64)") | .url')"
DARWIN_ARM64_URL="$(gh run view "$RUN_ID" --json jobs --jq -r '.jobs[] | select(.name == "build (darwin-arm64)") | .url')"
WINDOWS_AMD64_URL="$(gh run view "$RUN_ID" --json jobs --jq -r '.jobs[] | select(.name == "build (windows-amd64)") | .url')"

gh issue comment 354 --repo escoffier-labs/brigade --body "$(cat <<EOF
## Native hosted runners (issue #354)

Chose native GitHub-hosted matrix builds over cross-compilation/self-hosted runners.

| Asset | Runner | PR job |
|---|---|---|
| linux-amd64 | ubuntu-22.04 | ${LINUX_AMD64_URL} |
| linux-arm64 | ubuntu-22.04-arm | ${LINUX_ARM64_URL} |
| darwin-amd64 | macos-15-intel | ${DARWIN_AMD64_URL} |
| darwin-arm64 | macos-15 | ${DARWIN_ARM64_URL} |
| windows-amd64 | windows-latest (MSVC host asserted) | ${WINDOWS_AMD64_URL} |

PR workflow run: ${RUN_URL}

All five build jobs passed on the PR without \`--target\` cross-compilation.
EOF
)"
```

---

### Task 9: Merge, backfill v0.4.0, verify release, close issue

- [ ] Merge PR only after Task 7 green proof: `gh pr merge --merge --delete-branch`
- [ ] Dispatch backfill from `master`:

```bash
gh workflow run release-binaries.yml --ref master -f tag=v0.4.0
```

- [ ] Watch publish job:

```bash
gh run watch "$(gh run list --workflow release-binaries.yml --branch master --limit 1 --json databaseId --jq '.[0].databaseId')"
```

- [ ] Verify exactly eleven GitHub assets:

```bash
gh release view v0.4.0 --json assets --jq '.assets | length'
gh release view v0.4.0 --json assets --jq '.assets[].name' | sort
```

- [ ] Expect `11` and this sorted name list:

```text
checksums.txt
graphtrail-darwin-amd64
graphtrail-darwin-arm64
graphtrail-linux-amd64
graphtrail-linux-arm64
graphtrail-mcp-darwin-amd64
graphtrail-mcp-darwin-arm64
graphtrail-mcp-linux-amd64
graphtrail-mcp-linux-arm64
graphtrail-mcp-windows-amd64.exe
graphtrail-windows-amd64.exe
```

- [ ] Verify all ten checksum lines from the published `checksums.txt`:

```bash
verify_dir="$(mktemp -d)"
trap 'rm -rf "$verify_dir"' EXIT
gh release download v0.4.0 --dir "$verify_dir"
( cd "$verify_dir" && sha256sum -c checksums.txt )
wc -l < "$verify_dir/checksums.txt"
```

- [ ] Expect `OK` for all ten binaries and `10` checksum lines.
- [ ] Close Brigade issue only after the asset count, name inventory, and checksum verification all pass:

```bash
gh issue close 354 --repo escoffier-labs/brigade --comment "v0.4.0 now ships 10 native binaries plus checksums.txt; verified 11 GitHub assets and all ten checksum lines."
```

---

## Verification summary

| Stage | Command | Expected |
|---|---|---|
| RED (Task 1) | `cargo test --test repository_contract binary_release_attaches_native_assets_with_checksums` | FAIL: missing workflow/smoke script |
| GREEN (Task 3) | same | PASS: 1 passed |
| RED (Task 4 docs) | `cargo test --test repository_contract binary_release_documentation_covers_native_assets` | FAIL: missing release guide sections |
| GREEN (Task 4 docs) | same | PASS: 1 passed |
| Gate (Task 6) | fmt, clippy, test, build via Brigade | exit 0 |
| PR proof (Task 7) | `gh pr checks` | five `build (...)` jobs and `bundle release artifacts` success |
| Release proof (Task 9) | `gh release view v0.4.0` + local `sha256sum -c` | 11 assets, 10 checksum lines, all `OK` |

## Commit sequence (implementation phase)

1. `test(release): require native binary release contract`
2. `feat(release): add post-upload release smoke script`
3. `feat(release): add native binary release workflow`
4. `docs(release): document native binary release workflow`
5. `docs(changelog): note native binary release workflow`
