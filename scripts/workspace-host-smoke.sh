#!/usr/bin/env bash
set -euo pipefail

readonly REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly RUNTIME_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/a3s-webview-smoke.XXXXXX")"
readonly SERVER_LOG="$RUNTIME_ROOT/server.log"
readonly WEBVIEW_LOG="$RUNTIME_ROOT/webview.log"
readonly SHELL_ORIGIN="http://127.0.0.1:4318"
readonly REMOTE_ORIGIN="http://127.0.0.1:4319"
server_pid=""
webview_pid=""

cleanup() {
  local status=$?
  trap - EXIT INT TERM
  if [[ -n "$webview_pid" ]] && kill -0 "$webview_pid" 2>/dev/null; then
    kill -TERM "$webview_pid" 2>/dev/null || true
    wait "$webview_pid" 2>/dev/null || true
  fi
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill -TERM "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  if [[ $status -ne 0 ]]; then
    echo "Workspace Host smoke server log: $SERVER_LOG" >&2
    echo "Workspace Host native log: $WEBVIEW_LOG" >&2
  else
    rm -rf "$RUNTIME_ROOT"
  fi
  exit "$status"
}

trap cleanup EXIT INT TERM

for port in 4318 4319; do
  if lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "TCP port $port is already in use; stop the existing local service first." >&2
    exit 1
  fi
done

cargo build --quiet --manifest-path "$REPOSITORY_ROOT/Cargo.toml"
python3 "$REPOSITORY_ROOT/examples/workspace-host-smoke/server.py" \
  >"$SERVER_LOG" 2>&1 &
server_pid=$!

for _ in {1..50}; do
  if curl --silent --fail "$SHELL_ORIGIN/health" >/dev/null \
    && curl --silent --fail "$REMOTE_ORIGIN/health" >/dev/null; then
    break
  fi
  sleep 0.1
done

if ! curl --silent --fail "$SHELL_ORIGIN/health" >/dev/null \
  || ! curl --silent --fail "$REMOTE_ORIGIN/health" >/dev/null; then
  echo "Timed out waiting for the Workspace Host smoke origins." >&2
  exit 1
fi

"$REPOSITORY_ROOT/target/debug/a3s-webview" \
  --workspace-host \
  --url "$SHELL_ORIGIN/index.html" \
  --title "A3S Workspace Host Smoke" \
  >"$WEBVIEW_LOG" 2>&1 &
webview_pid=$!

SMOKE_STATE_URL="$SHELL_ORIGIN/smoke-state" python3 - <<'PY'
import json
import os
import sys
import time
import urllib.request

required = {
    "bridge-present",
    "calling-ready",
    "state-retained",
    "remote-loaded",
    "remote-ready",
    "remote-same-origin-navigation",
    "remote-cross-origin-navigation-blocked",
}
url = os.environ["SMOKE_STATE_URL"]
deadline = time.monotonic() + 20
last = {}
while time.monotonic() < deadline:
    try:
        with urllib.request.urlopen(url, timeout=1) as response:
            last = json.load(response)
    except Exception:
        time.sleep(0.1)
        continue
    steps = set(last.get("steps", []))
    if required <= steps and not last.get("forbiddenRequests"):
        print(json.dumps(last, indent=2, sort_keys=True))
        sys.exit(0)
    if last.get("forbiddenRequests") or "state-lost" in steps or "bridge-missing" in steps:
        break
    time.sleep(0.1)

print(json.dumps(last, indent=2, sort_keys=True), file=sys.stderr)
sys.exit(1)
PY
