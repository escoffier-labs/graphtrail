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
