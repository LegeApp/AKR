#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: setup-akr-mcp.sh [--repo-dir DIR] [--dry-run] [--no-claude] [--no-codex] [--no-opencode] [--debug]

One-time setup for the AKR MCP server across:
- AkR CLI build/install
- Codex MCP config (~/.codex/config.toml)
- OpenCode MCP config (~/.config/opencode/opencode.jsonc)
- Claude MCP registration

Options:
  --repo-dir DIR   AKR repo root (default: parent directory of this script)
  --dry-run        Print changes without writing
  --debug          Use target/debug/akr-mcp instead of release
  --no-claude      Skip Claude registration
  --no-codex       Skip Codex config update
  --no-opencode    Skip OpenCode config update
  -h, --help       Show this help
USAGE
}

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY_RUN=0
DO_CLAUDE=1
DO_CODEX=1
DO_OPENCODE=1
USE_DEBUG=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo-dir)
      REPO_DIR="$2"; shift 2 ;;
    --dry-run)
      DRY_RUN=1; shift ;;
    --debug)
      USE_DEBUG=1; shift ;;
    --no-claude)
      DO_CLAUDE=0; shift ;;
    --no-codex)
      DO_CODEX=0; shift ;;
    --no-opencode)
      DO_OPENCODE=0; shift ;;
    -h|--help)
      usage; exit 0 ;;
    *)
      echo "error: unknown arg: $1" >&2
      usage
      exit 1 ;;
  esac
done

CARGO_TOML="$REPO_DIR/Cargo.toml"
if [[ ! -f "$CARGO_TOML" ]]; then
  echo "error: no Cargo.toml at repo root: $CARGO_TOML" >&2
  exit 1
fi

if [[ ! -d "$REPO_DIR/crates/akr-mcp" ]]; then
  echo "error: akr-mcp crate missing at $REPO_DIR/crates/akr-mcp" >&2
  exit 1
fi

AKR_BIN_DIR="${HOME}/.local/bin"
AKR_BIN="$AKR_BIN_DIR/akr"
AKR_MCP_BIN="$AKR_BIN_DIR/akr-mcp"

log() { echo "[setup-akr-mcp] $*"; }
run() { if [[ "$DRY_RUN" -eq 1 ]]; then log "DRY-RUN: $*"; else "$@"; fi; }

# Build and install AKR MCP
if [[ "$USE_DEBUG" -eq 1 ]]; then
  BUILD_MODE="debug"
  BUILD_CMD=(cargo build --package akr-cli --package akr-mcp)
  SOURCE_AKR="$REPO_DIR/target/debug/akr"
  SOURCE_BIN="$REPO_DIR/target/debug/akr-mcp"
else
  BUILD_MODE="release"
  BUILD_CMD=(cargo build --release --package akr-cli --package akr-mcp)
  SOURCE_AKR="$REPO_DIR/target/release/akr"
  SOURCE_BIN="$REPO_DIR/target/release/akr-mcp"
fi

log "Using repo: $REPO_DIR"
log "Build mode: $BUILD_MODE"
if [[ "$DRY_RUN" -eq 1 ]]; then
  log "DRY-RUN: (cd $REPO_DIR && ${BUILD_CMD[*]})"
else
  (cd "$REPO_DIR" && "${BUILD_CMD[@]}")
fi

if [[ "$DRY_RUN" -eq 0 && ( ! -x "$SOURCE_AKR" || ! -x "$SOURCE_BIN" ) ]]; then
  echo "error: built binaries not found: $SOURCE_AKR, $SOURCE_BIN" >&2
  echo "Hint: rerun with --debug or build first manually." >&2
  exit 1
fi

# Copy beside the target and rename over it, rather than writing through it.
#
# `cp` onto a binary that a running MCP server has mapped fails with ETXTBSY ("Text file
# busy"), which is the common case: the server whose stale build you are replacing is the
# reason you are running this. `rename(2)` swaps the directory entry instead, so the
# running process keeps the inode it already opened and the next start picks up the new
# one — and it is atomic, so there is never a half-written binary on the path.
if [[ "$DRY_RUN" -eq 1 ]]; then
  log "DRY-RUN: install $SOURCE_AKR -> $AKR_BIN and $SOURCE_BIN -> $AKR_MCP_BIN"
else
  mkdir -p "$AKR_BIN_DIR"
  cp "$SOURCE_AKR" "$AKR_BIN.new" && mv -f "$AKR_BIN.new" "$AKR_BIN"
  cp "$SOURCE_BIN" "$AKR_MCP_BIN.new" && mv -f "$AKR_MCP_BIN.new" "$AKR_MCP_BIN"
fi
log "Installed $AKR_BIN"
log "Installed $AKR_MCP_BIN"
if [[ "$DRY_RUN" -eq 0 ]]; then
  INSTALLED_VERSION="$($AKR_MCP_BIN --version)"
  log "Verified installed server: $INSTALLED_VERSION ($AKR_MCP_BIN)"
fi
log "NOTE: a server that is already running keeps the old binary until it restarts."
log "      Reconnect the MCP server (or restart the session) before using knowledge.* tools."

# Install/refresh a section in ~/.codex/config.toml
if [[ "$DO_CODEX" -eq 1 ]]; then
  CODEX_CFG="$HOME/.codex/config.toml"
  if [[ ! -f "$CODEX_CFG" ]]; then
    echo "warning: Codex config not found: $CODEX_CFG (skipping Codex update)"
  else
    if grep -q '^\[mcp_servers\.akr\]' "$CODEX_CFG"; then
      log "Codex already has [mcp_servers.akr]; updating command to $AKR_MCP_BIN"
      if [[ "$DRY_RUN" -eq 0 ]]; then
        python3 - <<PY
import re
from pathlib import Path
path = Path(r"$CODEX_CFG")
text = path.read_text()
text = re.sub(r"\[mcp_servers\.akr\]\ncommand\s*=\s*\"[^\"]*\"", f"[mcp_servers.akr]\ncommand = \"$AKR_MCP_BIN\"", text)
path.write_text(text)
PY
      fi
    else
      log "Appending [mcp_servers.akr] to Codex config"
      if [[ "$DRY_RUN" -eq 1 ]]; then log "DRY-RUN: append AKR MCP config to $CODEX_CFG"; else printf '\n[mcp_servers.akr]\ncommand = "%s"\n' "$AKR_MCP_BIN" >> "$CODEX_CFG"; fi
    fi
  fi
fi

# Update ~/.config/opencode/opencode.jsonc
if [[ "$DO_OPENCODE" -eq 1 ]]; then
  OPCFG="$HOME/.config/opencode/opencode.jsonc"
  if [[ ! -f "$OPCFG" ]]; then
    echo "warning: OpenCode config not found: $OPCFG (skipping OpenCode update)"
  else
    if ! command -v jq >/dev/null 2>&1; then
      echo "error: jq is required to safely edit OpenCode config. Install jq and rerun, or skip with --no-opencode." >&2
      exit 1
    fi
    log "Updating OpenCode MCP section"
    TMPFILE="$(mktemp)"
    if [[ "$DRY_RUN" -eq 1 ]]; then log "DRY-RUN: update OpenCode MCP section"; else jq --arg cmd "$AKR_MCP_BIN" '.mcp.akr = { type: "local", command: [ $cmd ], enabled: true }' "$OPCFG" > "$TMPFILE" && mv "$TMPFILE" "$OPCFG"; fi
  fi
fi

# Register Claude MCP server
if [[ "$DO_CLAUDE" -eq 1 ]]; then
  if ! command -v claude >/dev/null 2>&1; then
    echo "warning: claude binary not found; skipping Claude registration"
  else
    if claude mcp get akr >/dev/null 2>&1; then
      log "Claude already has an AKR MCP server configured"
    else
      log "Registering AKR MCP in Claude (user scope)"
      run claude mcp add --scope user akr "$AKR_MCP_BIN"
    fi
  fi
fi

log "Done. Restart Codex, Claude, and OpenCode to load updated MCP config."
