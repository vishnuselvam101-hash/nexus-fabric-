#!/usr/bin/env bash
# End-to-end smoke test against a freshly started server with an empty data directory.
# Usage: tests/e2e_smoke.sh   (builds nf-server if needed; needs curl and python3)
set -euo pipefail
cd "$(dirname "$0")/.."
PORT="${NF_TEST_PORT:-8791}"; BASE="http://127.0.0.1:$PORT"
DATA="$(mktemp -d)"; JAR="$DATA/cookies"; VJAR="$DATA/viewer"
cargo build -q -p nf-server
# V-38: never talk to a server this script did not start
if curl -s -m 2 -o /dev/null "$BASE/" 2>/dev/null; then echo "something already answers on $BASE; refusing to run (choose NF_TEST_PORT)"; rm -rf "$DATA"; exit 2; fi
TOKEN="e2e-setup-token-$RANDOM$RANDOM"   # V-36: first-run setup needs the installation's one-time token
NF_SETUP_TOKEN="$TOKEN" NF_DATA_DIR="$DATA" NF_LISTEN="127.0.0.1:$PORT" target/debug/nf-server >"$DATA/server.log" 2>&1 &
PID=$!; trap 'kill $PID 2>/dev/null || true; rm -rf "$DATA"' EXIT
up=0; for _ in $(seq 1 50); do kill -0 "$PID" 2>/dev/null || break; curl -fsS "$BASE/api/health" >/dev/null 2>&1 && { up=1; break; }; sleep 0.2; done
[ "$up" = 1 ] && kill -0 "$PID" 2>/dev/null || { echo "our own server did not start:"; cat "$DATA/server.log"; exit 1; }
pass=0; fail=0
check() { if eval "$2"; then pass=$((pass+1)); echo "ok   - $1"; else fail=$((fail+1)); echo "FAIL - $1"; fi; }
j() { python3 -c "import sys,json; d=json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
code() { curl -s -H "X-NF-Request: 1" -o /dev/null -w '%{http_code}' "$@"; }
post() { curl -s -H "X-NF-Request: 1" -b "$JAR" -H 'Content-Type: application/json' -d "$2" "$BASE$1"; }

check "health reports setup required"            '[ "$(curl -s -H "X-NF-Request: 1" $BASE/api/health | j "d[\"setup_required\"]")" = True ]'
check "API refuses unauthenticated access"        '[ "$(code $BASE/api/projects)" = 401 ]'
check "setup without the setup token refused (403)" '[ "$(code -H "Content-Type: application/json" -d "{\"username\":\"admin\",\"password\":\"correct-horse-9\"}" $BASE/api/setup)" = 403 ]'
check "setup with a wrong setup token refused (403)" '[ "$(code -H "X-NF-Setup-Token: wrong" -H "Content-Type: application/json" -d "{\"username\":\"admin\",\"password\":\"correct-horse-9\"}" $BASE/api/setup)" = 403 ]'
check "weak password refused at setup"            '[ "$(code -H "X-NF-Setup-Token: $TOKEN" -H "Content-Type: application/json" -d "{\"username\":\"admin\",\"password\":\"short\"}" $BASE/api/setup)" = 400 ]'
check "administrator created"                     '[ "$(code -H "X-NF-Setup-Token: $TOKEN" -H "Content-Type: application/json" -d "{\"username\":\"admin\",\"password\":\"correct-horse-9\"}" $BASE/api/setup)" = 200 ]'
check "second setup refused"                      '[ "$(code -H "X-NF-Setup-Token: $TOKEN" -H "Content-Type: application/json" -d "{\"username\":\"evil\",\"password\":\"correct-horse-9\"}" $BASE/api/setup)" = 409 ]'
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
C5=$(run '{"family":"cone_frustum","r1":200,"r2":199.99999,"h":300}')
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
# BE-003: a refused request is REFUSED end to end (it used to be answered 200 and stored as REVIEW_REQUIRED)
check "refused request: HTTP 422 with state REFUSED and a reason" '[ "$(curl -s -o "$DATA/ref.json" -w "%{http_code}" -H "X-NF-Request: 1" -b $JAR -H "Content-Type: application/json" -d "{\"family\":\"cone_frustum\",\"r1\":200,\"r2\":199.99999,\"h\":300}" $BASE/api/projects/1/calculations)" = 422 ] && [ "$(j "d[\"state\"]==\"REFUSED\" and bool(d[\"outcome\"]) and d[\"id\"]>0" < "$DATA/ref.json")" = True ]'
check "refused request: stored state is REFUSED, never REVIEW_REQUIRED" '[ "$(get $C5 | j "d[\"state\"]")" = REFUSED ] && [ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/calculations | j "[c[\"state\"] for c in d if c[\"id\"]==$C5][0]")" = REFUSED ]'
check "refused request: counted as refused, not as review required" '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/dashboard | j "d[\"refused\"]==2 and d[\"review_required\"]==4 and d[\"approved\"]==0")" = True ]'
check "refused request: replay refused (409), attempt kept in the audit log" '[ "$(code -b $JAR -X POST $BASE/api/calculations/$C5/replay)" = 409 ] && [ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/audit | j "sum(1 for a in d if a[\"action\"]==\"calculation_refused\")")" = 2 ]'
# BE-001: declared surface against the radii
BAD='{"family":"header_with_branch","header_radius":150,"header_dev_radius":160,"header_length":1200,"branch_station":600,"branch_radius":100,"branch_dev_radius":110,"beta_deg":60,"delta":0,"s_end":400,"surface_reference":"outside"}'
check "outside radius smaller than development radius: refused (422), no DXF" '[ "$(curl -s -o "$DATA/bad.json" -w "%{http_code}" -H "X-NF-Request: 1" -b $JAR -H "Content-Type: application/json" -d "$BAD" $BASE/api/projects/1/calculations)" = 422 ] && [ "$(j "d[\"field\"]" < "$DATA/bad.json")" = header_dev_radius ] && [ "$(code -b $JAR $BASE/api/calculations/$(j "d[\"id\"]" < "$DATA/bad.json")/dxf/header)" = 404 ]'
check "unknown request field refused by name (400)" '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR -H "Content-Type: application/json" -d "{\"family\":\"cylinder_shell\",\"r_dev\":100,\"length\":250,\"thickness\":12}" $BASE/api/projects/1/calculations | j "\"thickness\" in d[\"error\"]")" = True ]'
check "unsupported input: DXF endpoint 404"       '[ "$(code -b $JAR $BASE/api/calculations/$C5/dxf/reducer)" = 404 ]'
curl -s -H "X-NF-Request: 1" -b "$JAR" -D "$DATA/h" -o "$DATA/out.dxf" "$BASE/api/calculations/$C1/dxf/reducer"
check "DXF downloads and ends with EOF"           '[ "$(tail -1 $DATA/out.dxf)" = EOF ]'
check "DXF file name marked NOT-RELEASED"         'grep -qi "filename=\"NOT-RELEASED_REVIEW_REQUIRED" "$DATA/h"'
check "DXF carries the NOT RELEASED marking on a non-cutting layer" 'grep -q "^NOT RELEASED - DEVELOPMENT BUILD" "$DATA/out.dxf" && [ "$(grep -B2 -A0 "^10$" "$DATA/out.dxf" >/dev/null; python3 - "$DATA/out.dxf" <<PY
import sys
L=open(sys.argv[1]).read().split("\n"); p=list(zip(L[0::2],L[1::2])); i=[k for k,(c,v) in enumerate(p) if c.strip()=="0" and v=="TEXT"]
print(all(p[k+1]==("  8","NF_NOT_RELEASED") for k in i) and len(i)==3 and not any(v=="NF_NOT_RELEASED" and p[k-1][1] in ("LINE","ARC","POLYLINE","VERTEX") for k,(c,v) in enumerate(p)))
PY
)" = True ]'
# D1 (validation): every pattern states the side it is drawn from, the same for all parts, in result, gates and DXF
check "marking side stated in the DXF (non-cutting text) and in the result" 'grep -q "^MARKING SIDE: PATTERN DRAWN AS SEEN FROM THE OUTSIDE OF THE FORMED PART - PROVISIONAL CONVENTION, NOT REVIEWED" "$DATA/out.dxf" && [ "$(get $C3 | j "all(p[\"marking_side\"]==\"outside\" for p in d[\"patterns\"]) and \"PROVISIONAL convention\" in [g[\"detail\"] for g in d[\"gates\"] if g[\"id\"]==\"G02\"][0] and [g[\"result\"] for g in d[\"gates\"] if g[\"id\"]==\"G02\"][0]==\"INCONCLUSIVE\"")" = True ]'
check "DXF header extents are those of the cone sector, not of its full circles" '[ "$(python3 - "$DATA/out.dxf" <<PY
import sys
L=open(sys.argv[1]).read().split("\n"); i=L.index("\$EXTMIN"); j=L.index("\$EXTMAX")
print(abs(float(L[i+2])+255.6485)<1e-3 and abs(float(L[j+4])-632.4555)<1e-3 and float(L[i+4])>-150)
PY
)" = True ]'
check "DXF header declares decimal units, degrees counter-clockwise; millimetres stated" 'grep -A2 "^\$LUNITS" "$DATA/out.dxf" | tail -1 | grep -qx 2 && grep -A2 "^\$ANGDIR" "$DATA/out.dxf" | tail -1 | grep -qx 0 && grep -q "^UNITS: MILLIMETRES" "$DATA/out.dxf"'
RP=$(curl -s -H "X-NF-Request: 1" -b $JAR -X POST $BASE/api/calculations/$C2/replay)
check "replay audit in a separate process matches"  '[ "$(echo "$RP" | j "d[\"matches\"] and d[\"kind\"]==\"separate_process_replay\"")" = True ]'
check "G19 reflects the replay but does not pass"   '[ "$(get $C2 | j "[g[\"result\"] for g in d[\"gates\"] if g[\"id\"]==\"G19\"][0]")" = INCONCLUSIVE ]'
C6=$(run '{"family":"cone_frustum","r1":200,"r2":100,"h":300}')
check "identical request gives identical closure" '[ "$(get $C1 | j "d[\"closure_hash\"]")" = "$(get $C6 | j "d[\"closure_hash\"]")" ]'
# --- assembly, correction, dependency propagation, obligations
A1=$(run '{"family":"header_with_branch","header_radius":150,"header_dev_radius":150,"header_length":1200,"branch_station":600,"branch_radius":100,"branch_dev_radius":100,"beta_deg":60,"delta":0,"s_end":400,"surface_reference":"outside"}')
RA=$(get $A1)
check "assembly: two parts, opening on the header"  '[ "$(echo "$RA" | j "[p[\"part\"] for p in d[\"patterns\"]]==[\"header\",\"branch\"] and len(d[\"patterns\"][0][\"holes\"])==1")" = True ]'
check "assembly: interface checks pass (three fold-back, two independent joint point checks)"  '[ "$(echo "$RA" | j "[c[\"verdict\"] for c in d[\"checks\"] if c[\"scope\"]==\"interface\"]==[\"PASS\"]*5")" = True ]'
check "assembly: dependency edges exposed"          '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/calculations/$A1/dependencies | j "len(d[\"edges\"])>=20")" = True ]'
check "correction without a reason refused"         '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"parameter\":\"header_radius\",\"new_value\":158}" $BASE/api/calculations/$A1/corrections)" = 400 ]'
# BE-004: a correction the engine refuses changes nothing (it used to supersede the valid job and record a lesson)
check "correction to a refused value: 422, nothing changed" '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"parameter\":\"header_radius\",\"new_value\":-5,\"reason\":\"engineer typo test\"}" $BASE/api/calculations/$A1/corrections)" = 422 ] && [ "$(get $A1 | j "d[\"superseded_by\"] is None and d[\"state\"]==\"REVIEW_REQUIRED\"")" = True ] && [ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/lessons | j "len(d)")" = 0 ] && [ "$(code -b $JAR $BASE/api/calculations/$A1/dxf/header)" = 200 ]'
# BE-001: correcting the outside radius to 142 below the development radius 150 is a negative wall thickness: refused too.
check "correction that contradicts the declared surface: 422, nothing changed" '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"parameter\":\"header_radius\",\"new_value\":142,\"reason\":\"drawing dimension is the inside diameter\"}" $BASE/api/calculations/$A1/corrections)" = 422 ] && [ "$(get $A1 | j "d[\"superseded_by\"] is None")" = True ] && [ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/obligations | j "len(d)")" = 0 ]'
CR=$(post /api/calculations/$A1/corrections '{"parameter":"header_radius","new_value":158,"reason":"drawing dimension is the outside diameter of the thicker pipe"}')
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
# BE-009: lists, project view and dashboard report the same state as the detail view
LIST=$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/calculations)
check "job list agrees with every detail view"      '[ "$(for c in $(echo "$LIST" | j "\" \".join(str(c[\"id\"]) for c in d)"); do [ "$(echo "$LIST" | j "[c[\"state\"] for c in d if c[\"id\"]==$c][0]")" = "$(get $c | j "d[\"state\"]")" ] || echo differ; done)" = "" ]'
check "superseded job is REJECTED and superseded in list and project view" '[ "$(echo "$LIST" | j "[(c[\"state\"],c[\"superseded\"]) for c in d if c[\"id\"]==$A1]==[(\"REJECTED\",True)]")" = True ] && [ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/projects/1 | j "[(c[\"state\"],c[\"superseded\"]) for c in d[\"calculations\"] if c[\"id\"]==$A1]==[(\"REJECTED\",True)]")" = True ]'
check "dashboard: superseded and refused are not review required" '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/dashboard | j "d[\"superseded\"]==1 and d[\"refused\"]==3 and d[\"rejected\"]==0 and d[\"review_required\"]==d[\"calculations\"]-4 and d[\"approved\"]==0")" = True ]'
# --- learning from mistakes: the corrected value must not slip through again
check "correction recorded as a lesson"            '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/lessons | j "[(l[\"kind\"],l[\"parameter\"],l[\"wrong_value\"],l[\"right_value\"]) for l in d]==[(\"corrected_value\",\"header_radius\",150.0,158.0)]")" = True ]'
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
A4=$(run '{"family":"header_with_branch","header_radius":158,"header_dev_radius":150,"header_length":900,"branch_station":450,"branch_radius":90,"branch_dev_radius":90,"beta_deg":75,"delta":0,"s_end":400,"surface_reference":"outside"}')
check "the corrected value raises no advisory"      '[ "$(get $A4 | j "d[\"advisories\"]")" = "[]" ]'
check "single-part job cannot be corrected this way" '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"parameter\":\"r1\",\"new_value\":210,\"reason\":\"test reason\"}" $BASE/api/calculations/$C1/corrections)" = 400 ]'
check "malformed request refused"                 '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"family\":\"teapot\"}" $BASE/api/projects/1/calculations)" = 400 ]'

check "viewer account created"                    '[ "$(code -b $JAR -H "Content-Type: application/json" -d "{\"username\":\"viewer1\",\"password\":\"read-only-pass\",\"role\":\"viewer\"}" $BASE/api/users)" = 200 ]'
curl -s -H "X-NF-Request: 1" -o /dev/null -c "$VJAR" -H 'Content-Type: application/json' -d '{"username":"viewer1","password":"read-only-pass"}' "$BASE/api/login"
check "viewer can read"                           '[ "$(code -b $VJAR $BASE/api/projects)" = 200 ]'
check "viewer cannot calculate"                   '[ "$(code -b $VJAR -H "Content-Type: application/json" -d "{\"family\":\"cylinder_shell\",\"r_dev\":1,\"length\":1}" $BASE/api/projects/1/calculations)" = 403 ]'
check "viewer cannot resolve obligations"           '[ "$(code -b $VJAR -H "Content-Type: application/json" -d "{\"decision\":\"waived\",\"reason\":\"viewer attempt\"}" $BASE/api/obligations/1/resolve)" = 403 ]'
check "viewer cannot see audit log"               '[ "$(code -b $VJAR $BASE/api/audit)" = 403 ]'
check "audit log: refused corrections recorded, chain and anchor intact" '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/audit | j "sum(1 for a in d if a[\"action\"]==\"correction_refused\")")" = 2 ] && [ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/audit/verify | j "d[\"intact\"] and d[\"anchor_intact\"] and d[\"database_integrity\"]==\"ok\"")" = True ]'
check "audit log records the rejected upload"     '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/audit | j "any(a[\"action\"]==\"upload_rejected\" for a in d)")" = True ]'
check "AMOEBA summary lists activity records"     '[ "$(curl -s -H "X-NF-Request: 1" -b $JAR $BASE/api/amoeba/summary | j "len(d[\"activities\"])>=5")" = True ]'
check "web client served"                         'curl -s -H "X-NF-Request: 1" $BASE/ | grep -q "NEXUS FABRIC"'
curl -s -H "X-NF-Request: 1" -o /dev/null -b "$JAR" -X POST "$BASE/api/logout"
check "session invalid after logout"              '[ "$(code -b $JAR $BASE/api/projects)" = 401 ]'
echo "----"; echo "passed: $pass  failed: $fail"; [ "$fail" = 0 ]
