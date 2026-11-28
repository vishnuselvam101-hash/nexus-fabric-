#!/usr/bin/env bash
# NF CAD UI smoke: starts its own nf-server on a free port in 8796-8829 with an empty data directory, creates the
# administrator, uploads the DXF fixture, and runs tests/cad_ui_smoke.js in Chromium. Never touches other servers.
set -euo pipefail
cd "$(dirname "$0")/.."
PORT=""
for p in $(seq 8796 8829); do
  if ! (echo > /dev/tcp/127.0.0.1/$p) 2>/dev/null; then PORT=$p; break; fi
done
[ -n "$PORT" ] || { echo "no free port in 8796-8829"; exit 2; }
BASE="http://127.0.0.1:$PORT"
DATA="$(mktemp -d)"; JAR="$DATA/cookies"
cargo build -q -p nf-server --offline
TOKEN="cad-ui-setup-$RANDOM$RANDOM"
NF_SETUP_TOKEN="$TOKEN" NF_DATA_DIR="$DATA" NF_LISTEN="127.0.0.1:$PORT" target/debug/nf-server >"$DATA/server.log" 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true; rm -rf "$DATA"' EXIT
up=0; for _ in $(seq 1 50); do kill -0 "$PID" 2>/dev/null || break; curl -fsS "$BASE/api/health" >/dev/null 2>&1 && { up=1; break; }; sleep 0.2; done
[ "$up" = 1 ] || { echo "our server did not start:"; cat "$DATA/server.log"; exit 1; }
H='X-NF-Request: 1'
curl -s -H "$H" -H "X-NF-Setup-Token: $TOKEN" -H 'Content-Type: application/json' -d '{"username":"admin","password":"correct-horse-9"}' "$BASE/api/setup" >/dev/null
curl -s -H "$H" -c "$JAR" -H 'Content-Type: application/json' -d '{"username":"admin","password":"correct-horse-9"}' "$BASE/api/login" >/dev/null
curl -s -H "$H" -b "$JAR" -H 'Content-Type: application/json' -d '{"name":"CAD smoke","description":"ui"}' "$BASE/api/projects" >/dev/null
curl -s -H "$H" -b "$JAR" -H 'X-Filename: basic.dxf' --data-binary @crates/nf-cad/tests/fixtures/basic.dxf "$BASE/api/projects/1/documents" >/dev/null
SESSION="$(awk '$6=="nf_session"{print "nf_session="$7}' "$JAR" | tail -1)"
[ -n "$SESSION" ] || { echo "no session cookie"; exit 1; }
NF_BASE="$BASE" NF_SESSION="$SESSION" NF_DOC=1 NODE_PATH=/usr/lib/node_modules/playwright/.. node tests/cad_ui_smoke.js
