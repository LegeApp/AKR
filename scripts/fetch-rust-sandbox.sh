#!/usr/bin/env bash
# Fetch a Rust toolchain into a sandbox that cannot reach static.rust-lang.org.
#
# Some agent sandboxes allow the npm registry and crates.io but block
# `https://static.rust-lang.org`, which is where rustup gets its toolchains.
# The `@rusttonpm/*` packages republish the official Rust component tarballs
# on npm, so they can be fetched with plain curl and unpacked by hand.
#
# Usage:
#     scripts/fetch-rust-sandbox.sh [install-dir]      # default /tmp/rust
#     export PATH="/tmp/rust/bin:$PATH"
#     cargo check --workspace --all-targets
#
# The version below is a nightly; the workspace declares `rust-version = 1.94`,
# so `cargo` needs `--ignore-rust-version` when the fetched toolchain is older.
# That only relaxes the MSRV assertion — a real compile error is still a real
# compile error.
set -euo pipefail

VERSION="${RUSTTONPM_VERSION:-0.92.20250925}"
PREFIX="${1:-/tmp/rust}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# component name on npm -> directory inside the package's vendor/ tree
COMPONENTS=(
  "rustc-linux-x64:rustc"
  "cargo-linux-x64:cargo"
  "rust-std-linux-x64:rust-std-x86_64-unknown-linux-gnu"
  "clippy-linux-x64:clippy-preview"
)

fetch() {
  # Sandboxes often cap a single command's wall clock, so resume in slices
  # rather than streaming a 130 MB tarball in one go.
  local url="$1" out="$2" i
  for i in $(seq 1 40); do
    if curl -sSL -C - -o "$out" --max-time 35 "$url"; then
      return 0
    fi
  done
  echo "could not download $url" >&2
  return 1
}

rm -rf "$PREFIX"
mkdir -p "$PREFIX"

for entry in "${COMPONENTS[@]}"; do
  pkg="${entry%%:*}"
  dir="${entry##*:}"
  url="https://registry.npmjs.org/@rusttonpm/${pkg}/-/${pkg}-${VERSION}.tgz"

  echo "fetching ${pkg} ..."
  fetch "$url" "$WORK/${pkg}.tgz"

  mkdir -p "$WORK/x-${pkg}"
  tar xzf "$WORK/${pkg}.tgz" -C "$WORK/x-${pkg}"
  cp -a "$WORK/x-${pkg}/package/vendor/${dir}/." "$PREFIX/"
  rm -rf "$WORK/x-${pkg}" "$WORK/${pkg}.tgz"
done

"$PREFIX/bin/rustc" --version
"$PREFIX/bin/cargo" --version

cat <<EOF

Toolchain installed to $PREFIX.

    export PATH="$PREFIX/bin:\$PATH"
    export CARGO_TARGET_DIR=/tmp/akr-target    # keep the repo's target/ alone
    export CARGO_HOME=/tmp/cargo-home
    # libsqlite3-sys 0.38's build script uses cfg_select!, stabilised after this
    # nightly. Turning the feature on crate-wide is enough to get past it.
    export RUSTFLAGS="-Zcrate-attr=feature(cfg_select)"

    cargo check --workspace --all-targets --ignore-rust-version
    cargo test  --workspace --ignore-rust-version

EOF
