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
    # Formatting first, and gated rather than merely documented. `cargo fmt --check` was
    # red on the committed tree for three days in August 2026 without anyone noticing,
    # which means the next contributor running the documented `cargo fmt` gets a large
    # diff belonging to somebody else. A check nobody runs is a convention, not a rule.
    cargo fmt --check
    cargo test
    python3 tools/check-design.py --strict
    cargo run --bin akr -- build --check
    # The source library's own invariant: registered bytes still match their hashes.
    cargo run --bin akr -- source verify
)
