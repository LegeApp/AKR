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
AKR_MCP_BIN="$AKR_BIN_DIR/akr-mcp"

log() { echo "[setup-akr-mcp] $*"; }
run() { if [[ "$DRY_RUN" -eq 1 ]]; then log "DRY-RUN: $*"; else eval "$@"; fi }

# Build and install AKR MCP
if [[ "$USE_DEBUG" -eq 1 ]]; then
  BUILD_MODE="debug"
  BUILD_CMD="cargo build --package akr-mcp"
  SOURCE_BIN="$REPO_DIR/target/debug/akr-mcp"
else
  BUILD_MODE="release"
  BUILD_CMD="cargo build --release --package akr-mcp"
  SOURCE_BIN="$REPO_DIR/target/release/akr-mcp"
fi

log "Using repo: $REPO_DIR"
log "Build mode: $BUILD_MODE"
run "cd \"$REPO_DIR\" && $BUILD_CMD"

if [[ ! -x "$SOURCE_BIN" ]]; then
  echo "error: built binary not found: $SOURCE_BIN" >&2
  echo "Hint: rerun with --debug or build first manually." >&2
  exit 1
fi

run "mkdir -p \"$AKR_BIN_DIR\" && cp \"$SOURCE_BIN\" \"$AKR_MCP_BIN\""
log "Installed $AKR_MCP_BIN"

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
      run "printf '\n[mcp_servers.akr]\ncommand = \"%s\"\n' \"$AKR_MCP_BIN\" >> \"$CODEX_CFG\""
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
    run "jq --arg cmd \"$AKR_MCP_BIN\" '.mcp.akr = { type: \"local\", command: [ \$cmd ], enabled: true }' \"$OPCFG\" > \"$TMPFILE\" && mv \"$TMPFILE\" \"$OPCFG\""
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
      run "claude mcp add --scope user akr \"$AKR_MCP_BIN\""
    fi
  fi
fi

log "Done. Restart Codex, Claude, and OpenCode to load updated MCP config."
