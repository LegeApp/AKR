#!/usr/bin/env bash
# Run the workspace test binaries in slices, appending to a log.
#
# Written for agent sandboxes that kill any command running longer than a
# fixed wall-clock budget (and kill background processes between commands).
# `cargo test --workspace` exceeds that budget on a cold cache, so this
# compiles once with `--no-run` and then executes the produced binaries one at
# a time, remembering which ones already ran.
#
# Usage:
#     scripts/run-tests-sliced.sh            # run the next few binaries
#     scripts/run-tests-sliced.sh --reset    # start a fresh pass
#     cat /tmp/akr-tests/summary.txt
set -uo pipefail

STATE="${AKR_TEST_STATE:-/tmp/akr-tests}"
BUDGET="${AKR_TEST_BUDGET:-35}"          # seconds of test execution per invocation
mkdir -p "$STATE"

if [ "${1:-}" = "--reset" ]; then
  rm -f "$STATE"/done.txt "$STATE"/summary.txt "$STATE"/output.log
fi
touch "$STATE/done.txt" "$STATE/summary.txt"

mapfile -t BINS < <(
  cargo test --workspace --ignore-rust-version --no-run --message-format=json 2>/dev/null |
    python3 -c '
import json, sys
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") and m.get("profile", {}).get("test"):
        print(m["executable"])
' | sort -u
)

if [ "${#BINS[@]}" -eq 0 ]; then
  echo "no test binaries; did the build fail?" >&2
  exit 1
fi

start=$(date +%s)
ran=0
for bin in "${BINS[@]}"; do
  grep -qxF "$bin" "$STATE/done.txt" && continue
  now=$(date +%s)
  [ $((now - start)) -ge "$BUDGET" ] && break

  name=$(basename "$bin")
  line=$("$bin" --test-threads=2 2>&1 | tee -a "$STATE/output.log" | grep -E '^test result' | tail -1)
  printf '%-44s %s\n' "$name" "${line:-<no result line>}" | tee -a "$STATE/summary.txt"
  echo "$bin" >> "$STATE/done.txt"
  ran=$((ran + 1))
done

remaining=$(( ${#BINS[@]} - $(wc -l < "$STATE/done.txt") ))
echo "--- ran $ran this pass, $remaining of ${#BINS[@]} remaining"

if [ "$remaining" -eq 0 ]; then
  bad=$(grep -vc ' 0 failed' "$STATE/summary.txt" || true)
  if [ "${bad:-0}" -gt 0 ]; then
    echo "--- $bad test binaries did not pass:"
    grep -v ' 0 failed' "$STATE/summary.txt"
    exit 1
  fi
  echo "--- all ${#BINS[@]} test binaries passed"
fi
exit 0
