#!/usr/bin/env bash
# Adversarial security checks against a freshly started server with an empty data directory.
# Synthetic data only. Usage: tests/security_smoke.sh   (needs curl and python3)
set -uo pipefail
cd "$(dirname "$0")/.."
PORT="${NF_TEST_PORT:-8792}"; BASE="http://127.0.0.1:$PORT"
DATA="$(mktemp -d)"; KEYDIR="$(mktemp -d)"
cargo build -q -p nf-server || exit 1
start() { NF_DATA_DIR="$DATA" NF_KEY_FILE="$KEYDIR/storage.key" NF_LISTEN="127.0.0.1:$PORT" target/debug/nf-server >>"$DATA/server.log" 2>&1 & PID=$!; for _ in $(seq 1 50); do curl -fsS "$BASE/api/health" >/dev/null 2>&1 && break; sleep 0.2; done; }
start; trap 'kill $PID 2>/dev/null; rm -rf "$DATA" "$KEYDIR"' EXIT
pass=0; fail=0
check() { if eval "$2"; then pass=$((pass+1)); echo "ok   - $1"; else fail=$((fail+1)); echo "FAIL - $1"; fi; }
j() { python3 -c "import sys,json; d=json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
X='X-NF-Request: 1'; CT='Content-Type: application/json'
code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }
login() { curl -s -o /dev/null -c "$DATA/$1.jar" -H "$X" -H "$CT" -d "{\"username\":\"$1\",\"password\":\"$2\"}" "$BASE/api/login"; }
as() { local who=$1; shift; curl -s -b "$DATA/$who.jar" -H "$X" "$@"; }
asc() { local who=$1; shift; code -b "$DATA/$who.jar" -H "$X" "$@"; }

curl -s -o /dev/null -H "$X" -H "$CT" -d '{"username":"admin","password":"correct-horse-9"}' "$BASE/api/setup"; login admin correct-horse-9
for u in alice bob; do as admin -o /dev/null -H "$CT" -d "{\"username\":\"$u\",\"password\":\"password-of-$u\",\"role\":\"engineer\"}" "$BASE/api/users"; login $u password-of-$u; done

echo "# anti-forgery and headers"
check "state change without anti-forgery header refused"  '[ "$(code -b $DATA/alice.jar -H "$CT" -d "{\"name\":\"x\"}" $BASE/api/projects)" = 403 ]'
check "cross-origin state change refused"                 '[ "$(asc alice -H "Origin: http://evil.example" -H "$CT" -d "{\"name\":\"x\"}" $BASE/api/projects)" = 403 ]'
check "same-origin state change accepted"                 '[ "$(asc alice -H "Origin: $BASE" -H "$CT" -d "{\"name\":\"Alice project\"}" $BASE/api/projects)" = 200 ]'
H=$(curl -s -D - -o /dev/null "$BASE/")
check "CSP forbids inline and foreign scripts"            'echo "$H" | grep -qi "content-security-policy: default-src .none.; script-src .self.;"'
check "framing denied, sniffing disabled"                 'echo "$H" | grep -qi "x-frame-options: DENY" && echo "$H" | grep -qi "x-content-type-options: nosniff"'
check "API responses not cacheable"                       'curl -s -D - -o /dev/null $BASE/api/health | grep -qi "cache-control: no-store"'
check "session cookie HttpOnly and SameSite=Strict"       'curl -s -D - -o /dev/null -H "$X" -H "$CT" -d "{\"username\":\"alice\",\"password\":\"password-of-alice\"}" $BASE/api/login | grep -i "set-cookie" | grep -qi "HttpOnly; SameSite=Strict"'

echo "# project isolation (broken access control, IDOR)"
CID=$(as alice -H "$CT" -d '{"family":"header_with_branch","header_radius":150,"header_dev_radius":150,"header_length":1200,"branch_station":600,"branch_radius":100,"branch_dev_radius":100,"beta_deg":60,"delta":0,"s_end":400,"surface_reference":"outside"}' "$BASE/api/projects/1/calculations" | j 'd["id"]')
NEW=$(as alice -H "$CT" -d '{"parameter":"header_radius","new_value":142,"reason":"inside diameter per drawing"}' "$BASE/api/calculations/$CID/corrections" | j 'd["new_calculation"]')
printf '%%PDF-1.7\nconfidential synthetic content\n' > "$DATA/a.pdf"
SHA=$(as alice -H 'X-Filename: a.pdf' --data-binary @"$DATA/a.pdf" "$BASE/api/projects/1/documents" | j 'd["sha256"]')
check "other engineer: project list is empty"             '[ "$(as bob $BASE/api/projects | j "len(d)")" = 0 ]'
check "other engineer: project hidden (404)"              '[ "$(asc bob $BASE/api/projects/1)" = 404 ]'
check "other engineer: job hidden"                        '[ "$(asc bob $BASE/api/calculations/$CID)" = 404 ]'
check "other engineer: DXF hidden"                        '[ "$(asc bob $BASE/api/calculations/$NEW/dxf/header)" = 404 ]'
check "other engineer: dependencies hidden"               '[ "$(asc bob $BASE/api/calculations/$CID/dependencies)" = 404 ]'
check "other engineer: cannot upload into the project"    '[ "$(asc bob -H "X-Filename: b.pdf" --data-binary @$DATA/a.pdf $BASE/api/projects/1/documents)" = 404 ]'
check "other engineer: cannot calculate in the project"   '[ "$(asc bob -H "$CT" -d "{\"family\":\"cylinder_shell\",\"r_dev\":1,\"length\":1}" $BASE/api/projects/1/calculations)" = 404 ]'
check "other engineer: cannot replay"                     '[ "$(asc bob -X POST $BASE/api/calculations/$CID/replay)" = 404 ]'
check "other engineer: cannot correct"                    '[ "$(asc bob -H "$CT" -d "{\"parameter\":\"delta\",\"new_value\":5,\"reason\":\"attempted tamper\"}" $BASE/api/calculations/$NEW/corrections)" = 404 ]'
check "other engineer: obligations list is empty"         '[ "$(as bob $BASE/api/obligations | j "len(d)")" = 0 ]'
check "other engineer: cannot resolve an obligation"      '[ "$(asc bob -H "$CT" -d "{\"decision\":\"waived\",\"reason\":\"attempted bypass\"}" $BASE/api/obligations/1/resolve)" = 404 ]'
check "other engineer: dashboard shows nothing of it"     '[ "$(as bob $BASE/api/dashboard | j "d[\"calculations\"]+d[\"documents\"]+d[\"records\"]+d[\"open_obligations\"]")" = 0 ]'
check "other engineer: record summary shows nothing"      '[ "$(as bob $BASE/api/amoeba/summary | j "len(d[\"records_by_kind\"])")" = 0 ]'
check "engineer cannot grant themselves membership"       '[ "$(asc bob -H "$CT" -d "{\"username\":\"bob\"}" $BASE/api/projects/1/members)" = 403 ]'
check "engineer cannot list users or read audit/security" '[ "$(asc bob $BASE/api/users)$(asc bob $BASE/api/audit)$(asc bob $BASE/api/security/status)" = 403403403 ]'
as admin -o /dev/null -H "$CT" -d '{"username":"bob"}' "$BASE/api/projects/1/members"
check "after an administrator adds the member: visible"   '[ "$(asc bob $BASE/api/projects/1)" = 200 ]'

echo "# input handling"
check "path traversal in DXF part name"                   '[ "$(asc alice "$BASE/api/calculations/$NEW/dxf/..%2F..%2Fstorage.key")" = 404 ]'
check "path separators stripped from stored file name"    '[ "$(as alice -H "X-Filename: ../../etc/passwd.pdf" --data-binary @$DATA/a.pdf $BASE/api/projects/1/documents >/dev/null; as alice $BASE/api/projects/1 | j "all(\"/\" not in x[\"filename\"] for x in d[\"documents\"])")" = True ]'
check "no file was written outside the blob directory"    '[ "$(find "$DATA" -type f | grep -v -e "/blobs/" -e "nexus-fabric.sqlite" -e "server.log" -e "\.jar$" -e "a.pdf" -e big -e huge | wc -l)" = 0 ]'
check "SQL metacharacters stored as data"                 '[ "$(as alice -H "$CT" -d "{\"name\":\"x\\\"); DROP TABLE users;--\",\"description\":\"<img src=x onerror=alert(1)>\"}" $BASE/api/projects | j "d[\"id\"]")" = 2 ] && [ "$(as admin $BASE/api/users | j "len(d)")" = 3 ]'
check "malformed JSON refused without crash"              '[ "$(asc alice -H "$CT" -d "{not json" $BASE/api/projects)" -ge 400 ] && curl -fsS $BASE/api/health >/dev/null'
check "non-finite and absurd numbers refused"             '[ "$(as alice -H "$CT" -d "{\"family\":\"cone_frustum\",\"r1\":1e308,\"r2\":-5,\"h\":0}" $BASE/api/projects/1/calculations | j "d[\"state\"]")" != APPROVED ]'
head -c 300000 /dev/zero | tr '\0' 'a' > "$DATA/big"; printf '{"name":"%s"}' "$(cat $DATA/big)" > "$DATA/big.json"
check "oversized JSON body refused (413)"                 '[ "$(asc alice -H "$CT" --data-binary @$DATA/big.json $BASE/api/projects)" = 413 ]'
( printf '%%PDF-1.7\n'; head -c 53000000 /dev/zero ) > "$DATA/huge"
check "oversized upload refused (413)"                    '[ "$(asc alice -H "X-Filename: huge.pdf" --data-binary @$DATA/huge $BASE/api/projects/1/documents)" = 413 ]'
check "executable content refused whatever its name"      '[ "$(printf "MZ\x90\x00" > $DATA/big; asc alice -H "X-Filename: drawing.pdf" --data-binary @$DATA/big $BASE/api/projects/1/documents)" = 422 ]'
check "server still healthy after hostile input"          'curl -fsS $BASE/api/health >/dev/null'

echo "# data at rest and integrity"
check "uploaded document not readable in the blob store"  '! grep -rq "confidential synthetic content" "$DATA/blobs"'
check "storage key is outside the data directory"         '[ ! -e "$DATA/storage.key" ] && [ -s "$KEYDIR/storage.key" ] && [ "$(as admin $BASE/api/security/status | j "d[\"storage_key_outside_data_directory\"]")" = True ]'
check "storage key file is owner-only"                    '[ "$(stat -c %a "$KEYDIR/storage.key")" = 600 ]'
check "passwords and tokens absent from database text"    '! grep -aq -e "password-of-alice" -e "correct-horse-9" "$DATA/nexus-fabric.sqlite"'
check "audit chain intact"                                '[ "$(as admin $BASE/api/audit/verify | j "d[\"intact\"]")" = True ]'
kill $PID; wait $PID 2>/dev/null
python3 - "$DATA/nexus-fabric.sqlite" "$DATA/blobs/$SHA.bin" <<'PY'
import sqlite3, sys
db = sqlite3.connect(sys.argv[1])
db.execute("UPDATE audit SET detail = 'tampered' WHERE id = 3")                      # rewrite history
rid = db.execute("SELECT id FROM records WHERE kind = 'parameter' LIMIT 1").fetchone()[0]
db.execute("UPDATE records SET json = replace(json, '\"value\":142.0', '\"value\":150.0') WHERE kind = 'parameter'")  # alter an engineering value
db.execute("UPDATE records SET json = replace(json, 'CUT_OUTER', 'CUT_OUTEX') WHERE kind = 'pattern'")
db.commit(); db.close()
b = bytearray(open(sys.argv[2], 'rb').read()); b[-1] ^= 1; open(sys.argv[2], 'wb').write(b)      # corrupt a stored document
PY
start
check "tampered audit entry detected"                     '[ "$(as admin $BASE/api/audit/verify | j "d[\"intact\"] is False and d[\"first_broken_entry\"]==3")" = True ]'
RP=$(as alice -X POST "$BASE/api/calculations/$NEW/replay")
check "tampered engineering records detected by replay"   '[ "$(echo "$RP" | j "d[\"matches\"] is False and d[\"stored_records_failing_identity_check\"]>0")" = True ]'
check "job with tampered records becomes REJECTED"        '[ "$(as alice $BASE/api/calculations/$NEW | j "d[\"state\"]")" = REJECTED ]'
check "files of the rejected job are withheld"            '[ "$(asc alice $BASE/api/calculations/$NEW/dxf/header)" = 409 ]'

echo "# authentication"
check "password change needs the current password"        '[ "$(asc alice -H "$CT" -d "{\"current\":\"wrong\",\"new\":\"a-new-password-1\"}" $BASE/api/password)" = 403 ]'
check "password change succeeds"                          '[ "$(asc alice -H "$CT" -d "{\"current\":\"password-of-alice\",\"new\":\"a-new-password-1\"}" $BASE/api/password)" = 200 ]'
check "all sessions revoked by password change"           '[ "$(asc alice $BASE/api/me)" = 401 ]'
check "old password no longer works"                      '[ "$(code -H "$X" -H "$CT" -d "{\"username\":\"alice\",\"password\":\"password-of-alice\"}" $BASE/api/login)" = 401 ]'
as admin -o /dev/null -H "$CT" -d '{"active":false}' "$BASE/api/users/3/active"
check "deactivated account: session revoked"              '[ "$(asc bob $BASE/api/me)" = 401 ]'
check "deactivated account: cannot sign in"               '[ "$(code -H "$X" -H "$CT" -d "{\"username\":\"bob\",\"password\":\"password-of-bob\"}" $BASE/api/login)" = 401 ]'
check "administrator cannot deactivate own account"       '[ "$(asc admin -H "$CT" -d "{\"active\":false}" $BASE/api/users/1/active)" = 400 ]'
for i in 1 2 3 4 5 6; do LAST=$(code -H "$X" -H "$CT" -d '{"username":"admin","password":"guess-number-'$i'"}' "$BASE/api/login"); done
check "sign-in throttled after repeated failures (429)"   '[ "$LAST" = 429 ]'
check "throttle holds even for the correct password"      '[ "$(code -H "$X" -H "$CT" -d "{\"username\":\"admin\",\"password\":\"correct-horse-9\"}" $BASE/api/login)" = 429 ]'
check "forged session cookie rejected"                    '[ "$(code -b "nf_session=$(head -c 32 /dev/zero | xxd -p -c 64)" $BASE/api/me)" = 401 ]'
echo "----"; echo "passed: $pass  failed: $fail"; [ "$fail" = 0 ]
