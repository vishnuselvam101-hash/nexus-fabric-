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
code() { curl -s -H "X-NF-Request: 1" -o /dev/null -w '%{http_code}' "$@"; }
post() { curl -s -H "X-NF-Request: 1" -b "$JAR" -H 'Content-Type: application/json' -d "$2" "$BASE$1"; }

check "health reports setup required"            '[ "$(curl -s -H "X-NF-Request: 1" $BASE/api/health | j "d[\"setup_required\"]")" = True ]'
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
UP=$(curl -s -H "X-NF-Request: 1" -b "$JAR" -H 'X-Filename: a.pdf' --data-binary @"$DATA/a.pdf" "$BASE/api/projects/1/documents")
check "PDF admitted by content"                   '[ "$(echo "$UP" | j "d[\"detected_type\"]")" = pdf ]'
check "executable rejected at the membrane"       '[ "$(code -b $JAR -H "X-Filename: b.pdf" --data-binary @$DATA/b.pdf $BASE/api/projects/1/documents)" = 422 ]'
SHA=$(echo "$UP" | j 'd["sha256"]')
check "stored blob is encrypted (plaintext absent)" '! grep -q "smoke test" "$DATA/blobs/$SHA.bin"'
check "snapshot recorded"                         '[ -n "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/projects/1 | j "d[\"snapshot\"]")" ]'

run() { post /api/projects/1/calculations "$1" | j 'd["id"]'; }
C1=$(run '{"family":"cone_frustum","r1":200,"r2":100,"h":300}')
C2=$(run '{"family":"eccentric_reducer","r1":200,"r2":100,"h":300,"e":100}')
C3=$(run '{"family":"branch_stub","header_radius":150,"branch_radius":100,"branch_dev_radius":100,"beta_deg":60,"delta":0,"s_end":400}')
C4=$(run '{"family":"cylinder_shell","r_dev":500,"length":1000}')
C5=$(run '{"family":"cone_frustum","r1":200,"r2":199.99,"h":300}')
get() { curl -s -H "X-NF-Request: 1" -b "$JAR" "$BASE/api/calculations/$1"; }
for c in $C1 $C2 $C3 $C4; do
  R=$(get $c)
  check "job $c: REVIEW_REQUIRED, never APPROVED"  '[ "$(echo "$R" | j "d[\"state\"]")" = REVIEW_REQUIRED ]'
  check "job $c: all verification checks PASS"     '[ "$(echo "$R" | j "all(c[\"verdict\"]==\"PASS\" for c in d[\"checks\"]) and len(d[\"checks\"])>=4")" = True ]'
  check "job $c: approval gate not implemented"    '[ "$(echo "$R" | j "[g[\"result\"] for g in d[\"gates\"] if g[\"id\"]==\"G16\"][0]")" = NOT_IMPLEMENTED ]'
done
check "CONE-01 sector angle matches reference"    '[ "$(get $C1 | j "abs(d[\"patterns\"][0][\"quantities\"][\"sector_angle\"]-1.986917653159)<1e-11")" = True ]'
check "ECC-01 developed angle matches reference"  '[ "$(get $C2 | j "abs(d[\"patterns\"][0][\"quantities\"][\"developed_angle\"]-1.861891289225)<1e-10")" = True ]'
check "unsupported input: no pattern, no DXF"     '[ "$(get $C5 | j "d[\"patterns\"] == []")" = True ]'
check "unsupported input: DXF endpoint 404"       '[ "$(code -b $JAR $BASE/api/calculations/$C5/dxf/reducer)" = 404 ]'
curl -s -H "X-NF-Request: 1" -b "$JAR" -D "$DATA/h" -o "$DATA/out.dxf" "$BASE/api/calculations/$C1/dxf/reducer"
check "DXF downloads and ends with EOF"           '[ "$(tail -1 $DATA/out.dxf)" = EOF ]'
check "DXF file name marked NOT-RELEASED"         'grep -qi "filename=\"NOT-RELEASED_REVIEW_REQUIRED" "$DATA/h"'
RP=$(curl -s -H "X-NF-Request: 1" -b $JAR -X POST $BASE/api/calculations/$C2/replay)
check "replay audit in a separate process matches"  '[ "$(echo "$RP" | j "d[\"matches\"] and d[\"kind\"]==\"separate_process_replay\"")" = True ]'
check "G19 reflects the replay but does not pass"   '[ "$(get $C2 | j "[g[\"result\"] for g in d[\"gates\"] if g[\"id\"]==\"G19\"][0]")" = INCONCLUSIVE ]'
C6=$(run '{"family":"cone_frustum","r1":200,"r2":100,"h":300}')
check "identical request gives identical closure" '[ "$(get $C1 | j "d[\"closure_hash\"]")" = "$(get $C6 | j "d[\"closure_hash\"]")" ]'
# --- assembly, correction, dependency propagation, obligations
A1=$(run '{"family":"header_with_branch","header_radius":150,"header_dev_radius":150,"header_length":1200,"branch_station":600,"branch_radius":100,"branch_dev_radius":100,"beta_deg":60,"delta":0,"s_end":400,"surface_reference":"outside"}')
RA=$(get $A1)
check "assembly: two parts, opening on the header"  '[ "$(echo "$RA" | j "[p[\"part\"] for p in d[\"patterns\"]]==[\"header\",\"branch\"] and len(d[\"patterns\"][0][\"holes\"])==1")" = True ]'
check "assembly: interface checks pass"             '[ "$(echo "$RA" | j "[c[\"verdict\"] for c in d[\"checks\"] if c[\"scope\"]==\"interface\"]==[\"PASS\",\"PASS\"]")" = True ]'
check "assembly: dependency edges exposed"          '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/calculations/$A1/dependencies | j "len(d[\"edges\"])>=20")" = True ]'
check "correction without a reason refused"         '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"parameter\":\"header_radius\",\"new_value\":142}" $BASE/api/calculations/$A1/corrections)" = 400 ]'
CR=$(post /api/calculations/$A1/corrections '{"parameter":"header_radius","new_value":142,"reason":"drawing dimension is the inside diameter"}')
A2=$(echo "$CR" | j 'd["new_calculation"]')
check "correction creates a new job"                '[ "$A2" -gt "$A1" ]'
check "correction raises obligations, 3 stay open"  '[ "$(echo "$CR" | j "d[\"obligations\"]>=9 and d[\"open\"]==3")" = True ]'
check "old job marked superseded"                   '[ "$(get $A1 | j "d[\"superseded_by\"]")" = "$A2" ]'
check "old job effectively REJECTED"                '[ "$(get $A1 | j "d[\"state\"]")" = REJECTED ]'
check "superseded DXF withheld"                     '[ "$(code -b $JAR $BASE/api/calculations/$A1/dxf/header)" = 409 ]'
RB=$(get $A2)
check "new job: open obligations are common-cause"  '[ "$(echo "$RB" | j "sorted(o[\"label\"] for o in d[\"obligations\"] if o[\"state\"]==\"open\")==[\"branch_dev_radius\",\"branch_radius\",\"header_dev_radius\"]")" = True ]'
check "new job: downstream+lateral discharged by evidence" '[ "$(echo "$RB" | j "all(o[\"state\"]==\"discharged\" for o in d[\"obligations\"] if o[\"direction\"]!=\"common_cause\")")" = True ]'
check "new job: G18 reports open obligations"       '[ "$(echo "$RB" | j "\"OPEN\" in [g[\"detail\"] for g in d[\"gates\"] if g[\"id\"]==\"G18\"][0]")" = True ]'
OID=$(echo "$RB" | j '[o["id"] for o in d["obligations"] if o["state"]=="open"][0]')
check "resolving without reason refused"            '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"decision\":\"confirmed\"}" $BASE/api/obligations/$OID/resolve)" = 400 ]'
check "obligation resolved by engineer decision"    '[ "$(post /api/obligations/$OID/resolve "{\"decision\":\"confirmed\",\"reason\":\"checked against the drawing: outside\"}" | j "d[\"state\"]")" = discharged_by_decision ]'
check "resolved obligation cannot be resolved twice" '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"decision\":\"waived\",\"reason\":\"second attempt\"}" $BASE/api/obligations/$OID/resolve)" = 409 ]'
check "two obligations remain open"                 '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/obligations | j "sum(1 for o in d if o[\"state\"]==\"open\")")" = 2 ]'
check "new job still REVIEW_REQUIRED"               '[ "$(get $A2 | j "d[\"state\"]")" = REVIEW_REQUIRED ]'
# --- learning from mistakes: the corrected value must not slip through again
check "correction recorded as a lesson"            '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/lessons | j "[(l[\"kind\"],l[\"parameter\"],l[\"wrong_value\"],l[\"right_value\"]) for l in d]==[(\"corrected_value\",\"header_radius\",150.0,142.0)]")" = True ]'
check "corrected job itself raises no advisory"    '[ "$(get $A2 | j "d[\"advisories\"]")" = "[]" ]'
A3=$(run '{"family":"header_with_branch","header_radius":150,"header_dev_radius":150,"header_length":900,"branch_station":450,"branch_radius":90,"branch_dev_radius":90,"beta_deg":75,"delta":0,"s_end":400,"surface_reference":"outside"}')
RC=$(get $A3)
check "repeating the corrected value raises an advisory" '[ "$(echo "$RC" | j "len(d[\"advisories\"])==1 and \"header_radius = 150\" in d[\"advisories\"][0][\"message\"] and d[\"advisories\"][0][\"state\"]==\"open\"")" = True ]'
check "known-mistake gate holds the job in review"  '[ "$(echo "$RC" | j "[g[\"result\"] for g in d[\"gates\"] if g[\"id\"]==\"G20\"][0]")" = INCONCLUSIVE ]'
check "the value itself was not changed"            '[ "$(echo "$RC" | j "d[\"request\"][\"header_radius\"]")" = 150.0 ]'
AID=$(echo "$RC" | j 'd["advisories"][0]["id"]')
check "acknowledging without a reason refused"      '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{}" $BASE/api/advisories/$AID/acknowledge)" = 400 ]'
check "engineer acknowledges with a reason"         '[ "$(post /api/advisories/$AID/acknowledge "{\"reason\":\"different header: this one is dimensioned outside\"}" | j "d[\"state\"]")" = acknowledged ]'
check "known-mistake gate passes after acknowledgement" '[ "$(get $A3 | j "[g[\"result\"] for g in d[\"gates\"] if g[\"id\"]==\"G20\"][0]")" = PASS ]'
check "job still not approved"                      '[ "$(get $A3 | j "d[\"state\"]")" = REVIEW_REQUIRED ]'
A4=$(run '{"family":"header_with_branch","header_radius":142,"header_dev_radius":150,"header_length":900,"branch_station":450,"branch_radius":90,"branch_dev_radius":90,"beta_deg":75,"delta":0,"s_end":400,"surface_reference":"inside"}')
check "the corrected value raises no advisory"      '[ "$(get $A4 | j "d[\"advisories\"]")" = "[]" ]'
check "single-part job cannot be corrected this way" '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"parameter\":\"r1\",\"new_value\":210,\"reason\":\"test reason\"}" $BASE/api/calculations/$C1/corrections)" = 400 ]'
check "malformed request refused"                 '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"family\":\"teapot\"}" $BASE/api/projects/1/calculations)" = 400 ]'

check "viewer account created"                    '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"username\":\"viewer1\",\"password\":\"read-only-pass\",\"role\":\"viewer\"}" $BASE/api/users)" = 200 ]'
curl -s -H "X-NF-Request: 1" -o /dev/null -c "$VJAR" -H 'Content-Type: application/json' -d '{"username":"viewer1","password":"read-only-pass"}' "$BASE/api/login"
check "viewer can read"                           '[ "$(code -b $VJAR $BASE/api/projects)" = 200 ]'
check "viewer cannot calculate"                   '[ "$(code -b $VJAR -H "Content-Type: application/json" -d "{\"family\":\"cylinder_shell\",\"r_dev\":1,\"length\":1}" $BASE/api/projects/1/calculations)" = 403 ]'
check "viewer cannot resolve obligations"           '[ "$(code -b $VJAR -H "Content-Type: application/json" -d "{\"decision\":\"waived\",\"reason\":\"viewer attempt\"}" $BASE/api/obligations/1/resolve)" = 403 ]'
check "viewer cannot see audit log"               '[ "$(code -b $VJAR $BASE/api/audit)" = 403 ]'
check "audit log records the rejected upload"     '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/audit | j "any(a[\"action\"]==\"upload_rejected\" for a in d)")" = True ]'
check "AMOEBA summary lists activity records"     '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/amoeba/summary | j "len(d[\"activities\"])>=5")" = True ]'
check "web client served"                         'curl -s -H "X-NF-Request: 1" $BASE/ | grep -q "NEXUS FABRIC"'
curl -s -H "X-NF-Request: 1" -o /dev/null -b "$JAR" -X POST "$BASE/api/logout"
check "session invalid after logout"              '[ "$(code -b $JAR $BASE/api/projects)" = 401 ]'
echo "----"; echo "passed: $pass  failed: $fail"; [ "$fail" = 0 ]
