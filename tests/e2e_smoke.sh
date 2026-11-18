#!/usr/bin/env bash
# End-to-end smoke test against a freshly started server with an empty data directory.
# Usage: tests/e2e_smoke.sh   (builds nf-server if needed; needs curl and python3)
set -euo pipefail
cd "$(dirname "$0")/.."
PORT="${NF_TEST_PORT:-8791}"; BASE="http://127.0.0.1:$PORT"
DATA="$(mktemp -d)"; JAR="$DATA/cookies"; VJAR="$DATA/viewer"
cargo build -q -p nf-server
NF_DATA_DIR="$DATA" NF_LISTEN="127.0.0.1:$PORT" target/debug/nf-server >"$DATA/server.log" 2>&1 &
PID=$!; trap 'kill $PID 2>/dev/null || true; rm -rf "$DATA"' EXIT
for _ in $(seq 1 50); do curl -fsS "$BASE/api/health" >/dev/null 2>&1 && break; sleep 0.2; done
pass=0; fail=0
check() { if eval "$2"; then pass=$((pass+1)); echo "ok   - $1"; else fail=$((fail+1)); echo "FAIL - $1"; fi; }
j() { python3 -c "import sys,json; d=json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }
post() { curl -s -b "$JAR" -H 'Content-Type: application/json' -d "$2" "$BASE$1"; }

check "health reports setup required"            '[ "$(curl -s $BASE/api/health | j "d[\"setup_required\"]")" = True ]'
check "API refuses unauthenticated access"        '[ "$(code $BASE/api/projects)" = 401 ]'
check "weak password refused at setup"            '[ "$(code -H "Content-Type: application/json" -d "{\"username\":\"admin\",\"password\":\"short\"}" $BASE/api/setup)" = 400 ]'
check "administrator created"                     '[ "$(code -H "Content-Type: application/json" -d "{\"username\":\"admin\",\"password\":\"correct-horse-9\"}" $BASE/api/setup)" = 200 ]'
check "second setup refused"                      '[ "$(code -H "Content-Type: application/json" -d "{\"username\":\"evil\",\"password\":\"correct-horse-9\"}" $BASE/api/setup)" = 409 ]'
check "wrong password refused"                    '[ "$(code -H "Content-Type: application/json" -d "{\"username\":\"admin\",\"password\":\"wrong-password\"}" $BASE/api/login)" = 401 ]'
check "login"                                     '[ "$(code -c $JAR -H "Content-Type: application/json" -d "{\"username\":\"admin\",\"password\":\"correct-horse-9\"}" $BASE/api/login)" = 200 ]'
PID_P=$(post /api/projects '{"name":"Smoke project","description":"e2e"}' | j 'd["id"]')
check "project created"                           '[ "$PID_P" = 1 ]'

printf '%%PDF-1.7\nsmoke test, not a drawing\n' > "$DATA/a.pdf"
printf 'MZ not allowed' > "$DATA/b.pdf"
UP=$(curl -s -b "$JAR" -H 'X-Filename: a.pdf' --data-binary @"$DATA/a.pdf" "$BASE/api/projects/1/documents")
check "PDF admitted by content"                   '[ "$(echo "$UP" | j "d[\"detected_type\"]")" = pdf ]'
check "executable rejected at the membrane"       '[ "$(code -b $JAR -H "X-Filename: b.pdf" --data-binary @$DATA/b.pdf $BASE/api/projects/1/documents)" = 422 ]'
SHA=$(echo "$UP" | j 'd["sha256"]')
check "stored blob is encrypted (plaintext absent)" '! grep -q "smoke test" "$DATA/blobs/$SHA.bin"'
check "snapshot recorded"                         '[ -n "$(curl -s -b $JAR $BASE/api/projects/1 | j "d[\"snapshot\"]")" ]'

run() { post /api/projects/1/calculations "$1" | j 'd["id"]'; }
C1=$(run '{"family":"cone_frustum","r1":200,"r2":100,"h":300}')
C2=$(run '{"family":"eccentric_reducer","r1":200,"r2":100,"h":300,"e":100}')
C3=$(run '{"family":"branch_stub","header_radius":150,"branch_radius":100,"branch_dev_radius":100,"beta_deg":60,"delta":0,"s_end":400}')
C4=$(run '{"family":"cylinder_shell","r_dev":500,"length":1000}')
C5=$(run '{"family":"cone_frustum","r1":200,"r2":199.99,"h":300}')
get() { curl -s -b "$JAR" "$BASE/api/calculations/$1"; }
for c in $C1 $C2 $C3 $C4; do
  R=$(get $c)
  check "job $c: REVIEW_REQUIRED, never APPROVED"  '[ "$(echo "$R" | j "d[\"state\"]")" = REVIEW_REQUIRED ]'
  check "job $c: all verification checks PASS"     '[ "$(echo "$R" | j "all(c[\"verdict\"]==\"PASS\" for c in d[\"checks\"]) and len(d[\"checks\"])>=4")" = True ]'
  check "job $c: approval gate not implemented"    '[ "$(echo "$R" | j "[g[\"result\"] for g in d[\"gates\"] if g[\"id\"]==\"G16\"][0]")" = NOT_IMPLEMENTED ]'
done
check "CONE-01 sector angle matches reference"    '[ "$(get $C1 | j "abs(d[\"pattern\"][\"quantities\"][\"sector_angle\"]-1.986917653159)<1e-11")" = True ]'
check "ECC-01 developed angle matches reference"  '[ "$(get $C2 | j "abs(d[\"pattern\"][\"quantities\"][\"developed_angle\"]-1.861891289225)<1e-10")" = True ]'
check "unsupported input: no pattern, no DXF"     '[ "$(get $C5 | j "d[\"pattern\"] is None and d[\"dxf_available\"] is False")" = True ]'
check "unsupported input: DXF endpoint 404"       '[ "$(code -b $JAR $BASE/api/calculations/$C5/dxf)" = 404 ]'
curl -s -b "$JAR" -D "$DATA/h" -o "$DATA/out.dxf" "$BASE/api/calculations/$C1/dxf"
check "DXF downloads and ends with EOF"           '[ "$(tail -1 $DATA/out.dxf)" = EOF ]'
check "DXF file name marked NOT-RELEASED"         'grep -qi "filename=\"NOT-RELEASED_REVIEW_REQUIRED" "$DATA/h"'
check "replay audit matches"                      '[ "$(curl -s -b $JAR -X POST $BASE/api/calculations/$C2/replay | j "d[\"matches\"]")" = True ]'
C6=$(run '{"family":"cone_frustum","r1":200,"r2":100,"h":300}')
check "identical request gives identical closure" '[ "$(get $C1 | j "d[\"closure_hash\"]")" = "$(get $C6 | j "d[\"closure_hash\"]")" ]'
check "malformed request refused"                 '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"family\":\"teapot\"}" $BASE/api/projects/1/calculations)" = 400 ]'

check "viewer account created"                    '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"username\":\"viewer1\",\"password\":\"read-only-pass\",\"role\":\"viewer\"}" $BASE/api/users)" = 200 ]'
curl -s -o /dev/null -c "$VJAR" -H 'Content-Type: application/json' -d '{"username":"viewer1","password":"read-only-pass"}' "$BASE/api/login"
check "viewer can read"                           '[ "$(code -b $VJAR $BASE/api/projects)" = 200 ]'
check "viewer cannot calculate"                   '[ "$(code -b $VJAR -H "Content-Type: application/json" -d "{\"family\":\"cylinder_shell\",\"r_dev\":1,\"length\":1}" $BASE/api/projects/1/calculations)" = 403 ]'
check "viewer cannot see audit log"               '[ "$(code -b $VJAR $BASE/api/audit)" = 403 ]'
check "audit log records the rejected upload"     '[ "$(curl -s -b $JAR $BASE/api/audit | j "any(a[\"action\"]==\"upload_rejected\" for a in d)")" = True ]'
check "AMOEBA summary lists activity records"     '[ "$(curl -s -b $JAR $BASE/api/amoeba/summary | j "len(d[\"activities\"])>=5")" = True ]'
check "web client served"                         'curl -s $BASE/ | grep -q "NEXUS FABRIC"'
curl -s -o /dev/null -b "$JAR" -X POST "$BASE/api/logout"
check "session invalid after logout"              '[ "$(code -b $JAR $BASE/api/projects)" = 401 ]'
echo "----"; echo "passed: $pass  failed: $fail"; [ "$fail" = 0 ]
