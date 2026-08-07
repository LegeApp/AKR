#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmpdir="$(mktemp -d)"

trap 'rm -rf "$tmpdir"' EXIT

echo "Verifying tracked files"
git -C "$ROOT" ls-files -z | while IFS= read -r -d '' path; do
    mkdir -p "$tmpdir/$(dirname "$path")"
    if [ -e "$ROOT/$path" ]; then
        cp "$ROOT/$path" "$tmpdir/$path"
    else
        echo "warning: tracked file missing, skipping: $path"
    fi
done

# Preserve git context so akr build --check can validate commit-aware outputs in the
# copied workspace. This is the closest approximation to a fresh copy that still
# includes the exact commit object graph and metadata.
cp -a "$ROOT/.git" "$tmpdir/.git"

(
    cd "$tmpdir"
    cargo test
    python3 tools/check-design.py --strict
    cargo run --bin akr -- build --check
)
