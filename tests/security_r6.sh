#!/usr/bin/env bash
# Security audit round 6 — reproducible checks for the findings in docs/security/SECURITY_AUDIT_R6.md.
# Every check is labelled with its vulnerability ID. A check FAILS while the finding is open: on the build audited
# on 19 Nov 2026 most of them fail by design. A passing run means the regression checks hold, not that the system is secure.
#
# Usage:  NF_BIN=/path/to/nf-server [NF_TEST_PORT=8820] bash tests/security_r6.sh
#         (default binary: target/release/nf-server, built if missing). Needs curl and python3; prlimit and
#         node+playwright are optional (the checks that need them are reported as SKIP).
# Runs ONLY against server processes it starts itself, on 127.0.0.1, with temporary data directories.
# It refuses to start if anything already answers on the port (see V-38). Never point it at a real installation.
# Optional: NF_R6_SETUP_ARGS='-H X-NF-Setup-Token:...' — extra curl arguments for first-run setup once V-36 is fixed.
set -uo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
PORT="${NF_TEST_PORT:-8820}"; HOSTPORT="127.0.0.1:$PORT"; BASE="http://$HOSTPORT"
BIN="${NF_BIN:-$ROOT/target/release/nf-server}"
[ -x "$BIN" ] || { cargo build -q --release -p nf-server || exit 1; BIN="$ROOT/target/release/nf-server"; }
SETUP_ARGS=(); [ -n "${NF_R6_SETUP_ARGS:-}" ] && read -r -a SETUP_ARGS <<<"$NF_R6_SETUP_ARGS"
X='X-NF-Request: 1'; CT='Content-Type: application/json'
TMP="$(mktemp -d)"; PID=""
cleanup() { [ -n "$PID" ] && kill "$PID" 2>/dev/null; rm -rf "$TMP"; }
trap cleanup EXIT
pass=0; fail=0; skip=0
check() { if eval "$2"; then pass=$((pass+1)); echo "ok   - $1"; else fail=$((fail+1)); echo "FAIL - $1"; fi; }
skipc() { skip=$((skip+1)); echo "SKIP - $1 ($2)"; }
j() { python3 -c "import sys,json; d=json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }

if curl -s -m 2 -o /dev/null "$BASE/" 2>/dev/null; then echo "something already answers on $BASE; refusing to run (choose NF_TEST_PORT)"; exit 2; fi

# start <data-dir> <key-file> [VAR=value ...]: start our own server; succeed only if OUR process answers
start() {
  local data=$1 key=$2; shift 2
  mkdir -p "$data" "$(dirname "$key")"
  ( umask 022; exec env "$@" NF_DATA_DIR="$data" NF_KEY_FILE="$key" NF_LISTEN="$HOSTPORT" "$BIN" >>"$TMP/server.log" 2>&1 ) & PID=$!
  for _ in $(seq 1 50); do
    kill -0 "$PID" 2>/dev/null || { PID=""; return 1; }
    curl -fsS -o /dev/null "$BASE/api/health" 2>/dev/null && return 0
    sleep 0.1
  done
  return 1
}
stop() { [ -n "$PID" ] && { kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null; }; PID=""; for _ in $(seq 1 30); do curl -s -m 1 -o /dev/null "$BASE/" 2>/dev/null || return 0; sleep 0.1; done; }
login() { curl -s -o /dev/null -c "$TMP/$1.jar" -H "$X" -H "$CT" -d "{\"username\":\"$1\",\"password\":\"$2\"}" "$BASE/api/login"; }
as() { local who=$1; shift; curl -s -b "$TMP/$who.jar" -H "$X" "$@"; }
asc() { local who=$1; shift; code -b "$TMP/$who.jar" -H "$X" "$@"; }

# ======================================================================== instance A
A="$TMP/a"; start "$A/data" "$A/key/storage.key" || { echo "server A did not start"; exit 1; }

echo "# V-36 first-administrator claim / Host validation"
R=$(curl -s -w ' %{http_code}' -H "$X" -H "$CT" -H "Host: rebind.attacker.example:$PORT" -H "Origin: http://rebind.attacker.example:$PORT" \
    -d '{"username":"rebinder","password":"rebinder-password-1"}' "$BASE/api/setup")
check "V-36 setup through a foreign Host (DNS rebinding) is refused"         '[ "${R##* }" != 200 ]'
if [ "${R##* }" = 200 ]; then ADMIN=rebinder; ADMINPW=rebinder-password-1
else ADMIN=admin; ADMINPW=admin-password-r6; curl -s -o /dev/null "${SETUP_ARGS[@]}" -H "$X" -H "$CT" -d "{\"username\":\"$ADMIN\",\"password\":\"$ADMINPW\"}" "$BASE/api/setup"; fi
login "$ADMIN" "$ADMINPW"; cp "$TMP/$ADMIN.jar" "$TMP/adm.jar"
for u in alice:engineer vic:viewer; do as adm -o /dev/null -H "$CT" -d "{\"username\":\"${u%%:*}\",\"password\":\"password-of-${u%%:*}\",\"role\":\"${u##*:}\"}" "$BASE/api/users"; login "${u%%:*}" "password-of-${u%%:*}"; done
check "setup: administrator session works (precondition)"                      '[ "$(asc adm $BASE/api/me)" = 200 ]'

echo "# V-43 data at rest: file modes under a permissive umask"
check "V-43 database file is owner-only (600)"                                 '[ "$(stat -c %a "$A/data/nexus-fabric.sqlite")" = 600 ]'
check "V-43 data directory is owner-only (700)"                                '[ "$(stat -c %a "$A/data")" = 700 ]'

echo "# V-49 / V-45 / V-47 unauthenticated surface and headers"
check "V-49 unauthenticated health does not disclose engine version or uptime" '! curl -s $BASE/api/health | grep -q -e uptime_seconds -e "\"engine\""'
H=$(curl -s -D - -o /dev/null "$BASE/")
check "V-45 CSP has no 'unsafe-inline'"                                        '! echo "$H" | grep -i "content-security-policy" | grep -q "unsafe-inline"'
check "V-47 Origin with non-web scheme (file://host) refused"                  '[ "$(asc alice -H "Origin: file://$HOSTPORT" -H "$CT" -d "{\"name\":\"o1\"}" $BASE/api/projects)" = 403 ]'
check "V-47 Origin with the wrong scheme (https on an http listener) refused"  '[ "$(asc alice -H "Origin: https://$HOSTPORT" -H "$CT" -d "{\"name\":\"o2\"}" $BASE/api/projects)" = 403 ]'

echo "# V-46 password policy"
check "V-46 common password 'password12' refused"                              '[ "$(asc adm -H "$CT" -d "{\"username\":\"weak1\",\"password\":\"password12\",\"role\":\"viewer\"}" $BASE/api/users)" = 400 ]'
check "V-46 password equal to user name refused"                               '[ "$(asc adm -H "$CT" -d "{\"username\":\"samesame12\",\"password\":\"samesame12\",\"role\":\"viewer\"}" $BASE/api/users)" = 400 ]'

echo "# V-37 password check under the global lock; unthrottled password change"
for i in $(seq 1 8); do LAST=$(asc vic -H "$CT" -d "{\"current\":\"guess-$i\",\"new\":\"whatever-12345\"}" "$BASE/api/password"); done
check "V-37 repeated wrong current passwords are throttled (429)"              '[ "$LAST" = 429 ]'
login vic password-of-vic
BG=(); for i in $(seq 1 40); do asc vic -o /dev/null -H "$CT" -d "{\"current\":\"g-$i\",\"new\":\"whatever-12345\"}" "$BASE/api/password" >/dev/null & BG+=($!); done
sleep 0.1; LAT=$(curl -s -o /dev/null -w '%{time_total}' "$BASE/api/health"); wait "${BG[@]}"   # never a bare wait: the server is a child too
check "V-37 health answers in < 0.5 s while 40 password checks run ($LAT s)"   'python3 -c "import sys; sys.exit(0 if float(sys.argv[1]) < 0.5 else 1)" "$LAT"'
login vic password-of-vic

echo "# V-41 security events in the audit log"
PRJ=$(as alice -H "$CT" -d '{"name":"r6 project"}' "$BASE/api/projects" | j 'd["id"]')
asc vic "$BASE/api/projects/$PRJ" >/dev/null
as vic -o /dev/null -X POST "$BASE/api/logout"; login vic password-of-vic
AUD=$(as adm "$BASE/api/audit")
check "V-41 sign-out is audited"                                               'echo "$AUD" | j "any(x[\"action\"]==\"logout\" for x in d)" | grep -q True'
check "V-41 refused access to another project is audited"                      'echo "$AUD" | j "any(\"denied\" in x[\"action\"] or \"forbidden\" in x[\"action\"] for x in d)" | grep -q True'

echo "# V-40 transient replay failure must not reject a job"
CID=$(as alice -H "$CT" -d '{"family":"cylinder_shell","r_dev":321,"length":654}' "$BASE/api/projects/$PRJ/calculations" | j 'd["id"]')
if command -v prlimit >/dev/null; then
  SOFT=$(prlimit --pid "$PID" --nofile --noheadings --output SOFT | tr -d ' '); HARD=$(prlimit --pid "$PID" --nofile --noheadings --output HARD | tr -d ' ')
  MAXFD=$(ls /proc/$PID/fd | sort -n | tail -1)
  if prlimit --pid "$PID" --nofile=$((MAXFD+3)):"$HARD" 2>/dev/null; then
    as alice -o /dev/null -X POST "$BASE/api/calculations/$CID/replay"
    prlimit --pid "$PID" --nofile="$SOFT":"$HARD"
    check "V-40 job is not REJECTED when the replay process could not start"   '[ "$(as alice $BASE/api/calculations | j "[c[\"state\"] for c in d if c[\"id\"]==$CID][0]")" != REJECTED ]'
    check "V-40 its DXF stays downloadable"                                    '[ "$(asc alice $BASE/api/calculations/$CID/dxf/shell)" = 200 ]'
  else skipc "V-40 replay failure handling" "prlimit not permitted"; fi
else skipc "V-40 replay failure handling" "prlimit not installed"; fi

echo "# V-35 upload body read before authentication"
( printf '%%PDF-1.7\n'; head -c 52000000 /dev/zero ) > "$TMP/big.pdf"
UP=$(curl -s -o /dev/null -w '%{http_code} %{size_upload}' -H "$X" -H 'Expect: 100-continue' --data-binary @"$TMP/big.pdf" "$BASE/api/projects/$PRJ/documents")
check "V-35 unauthenticated upload refused before the body is read (sent: ${UP#* } bytes)" '[ "${UP%% *}" = 401 ] && [ "${UP#* }" -lt 1048576 ]'

echo "# V-14 regression: stored script injection through the new client (optional browser probe)"
PL='<img src=x onerror=alert(1)><svg onload=alert(2)>'
as alice -o /dev/null -H "$CT" -d "{\"name\":\"$PL\",\"description\":\"$PL\"}" "$BASE/api/projects"
as alice -o /dev/null -H "$CT" -d "{\"family\":\"header_with_branch\",\"header_radius\":150,\"header_dev_radius\":150,\"header_length\":1200,\"branch_station\":600,\"branch_radius\":100,\"branch_dev_radius\":100,\"beta_deg\":60,\"delta\":0,\"s_end\":400,\"surface_reference\":\"$PL\"}" "$BASE/api/projects/$PRJ/calculations"
HW=$(as alice -H "$CT" -d '{"family":"header_with_branch","header_radius":150,"header_dev_radius":150,"header_length":1200,"branch_station":600,"branch_radius":100,"branch_dev_radius":100,"beta_deg":60,"delta":0,"s_end":400,"surface_reference":"outside"}' "$BASE/api/projects/$PRJ/calculations" | j 'd["id"]')
NEWJ=$(as alice -H "$CT" -d "{\"parameter\":\"header_radius\",\"new_value\":158,\"reason\":\"$PL\"}" "$BASE/api/calculations/$HW/corrections" | j 'd["new_calculation"]')
OBL=$(as alice "$BASE/api/obligations" | j '[o["id"] for o in d if o["state"]=="open"][0]')
as alice -o /dev/null -H "$CT" -d "{\"decision\":\"waived\",\"reason\":\"$PL\"}" "$BASE/api/obligations/$OBL/resolve"
as alice -o /dev/null -H "X-Filename: %3Cimg%20src%3Dx%20onerror%3Dalert(3)%3E.pdf" --data-binary $'%PDF-1.7\nx' "$BASE/api/projects/$PRJ/documents"
if command -v node >/dev/null && NODE_PATH="${NODE_PATH:-$(npm root -g 2>/dev/null)}" node -e "require('playwright')" 2>/dev/null; then
  cat > "$TMP/probe.js" <<'JS'
const { chromium } = require('playwright');
(async () => {
  const [B, user, pw, routes] = process.argv.slice(2);
  const opts = process.env.CHROME ? { executablePath: process.env.CHROME, args: ['--no-sandbox'] } : {};
  const browser = await chromium.launch(opts); const page = await browser.newPage(); const hits = [];
  page.on('dialog', async (d) => { hits.push('dialog ' + d.message()); await d.dismiss(); });
  await page.addInitScript(() => { window.__x = []; for (const f of ['alert', 'confirm', 'prompt']) window[f] = (m) => window.__x.push(String(m)); });
  await page.goto(B + '/'); await page.fill('#lu', user); await page.fill('#lp', pw); await page.click('#f button'); await page.waitForSelector('#view');
  for (const r of routes.split(' ')) {
    await page.evaluate((h) => { location.hash = h; }, r); await page.waitForTimeout(500);
    for (const t of await page.$$('[role=tab]')) { try { await t.click({ timeout: 300 }); } catch { /* not visible */ } }
    for (const d of await page.$$('details.rec')) { try { await d.click({ timeout: 200 }); } catch { /* not visible */ } }
    const bad = await page.evaluate(() => [...document.querySelectorAll('#app img, #app [onerror], #app [onload], #app script, #app iframe')].length);
    if (bad) hits.push(r + ' injected element');
  }
  hits.push(...(await page.evaluate(() => window.__x)));
  console.log(JSON.stringify(hits)); await browser.close(); process.exit(hits.length ? 1 : 0);
})();
JS
  CHROME="${CHROME:-$(command -v google-chrome || true)}" NODE_PATH="${NODE_PATH:-$(npm root -g)}" node "$TMP/probe.js" "$BASE" "$ADMIN" "$ADMINPW" \
    "#/dashboard #/projects #/project/$PRJ #/workspace #/calc/$HW #/calc/$NEWJ #/calc/$((HW-1)) #/manufacturing #/verification #/amoeba/records #/amoeba/obligations #/amoeba/lessons #/amoeba/dependencies/$NEWJ #/admin/users #/admin/audit #/admin/security #/account" >"$TMP/probe.out" 2>&1
  check "V-14 no script runs and no element is injected in 17 views ($(head -c 120 "$TMP/probe.out"))" '[ "$(cat $TMP/probe.out)" = "[]" ]'
else skipc "V-14 browser probe" "node with playwright not available"; fi

echo "# V-33 global sign-in lockout"
for i in $(seq 1 40); do curl -s -o /dev/null -H "$X" -H "$CT" -d "{\"username\":\"nobody-$i-$RANDOM\",\"password\":\"x\"}" "$BASE/api/login"; done
check "V-33 administrator can sign in after 40 failures by other names"       '[ "$(code -H "$X" -H "$CT" -d "{\"username\":\"$ADMIN\",\"password\":\"$ADMINPW\"}" $BASE/api/login)" = 200 ]'

echo "# V-34 unauthenticated audit flooding"
python3 -c "import json; print(json.dumps({'username': 'A'*200000, 'password': 'x'}))" > "$TMP/biguser.json"
size() { du -cb "$A/data"/nexus-fabric.sqlite* | tail -1 | cut -f1; }
S0=$(size); for i in $(seq 1 20); do curl -s -o /dev/null -H "$X" -H "$CT" --data-binary @"$TMP/biguser.json" "$BASE/api/login"; done; S1=$(size)
check "V-34 20 anonymous sign-in attempts grow the database by < 1 MB (grew $(( (S1-S0)/1024 )) KB)" '[ $((S1-S0)) -lt 1048576 ]'
check "V-34 user names written to the audit log are bounded (<= 64 chars)"     '[ "$(as adm $BASE/api/audit | j "max(len(x[\"username\"]) for x in d)")" -le 64 ]'
check "V-34 audit API response stays small (< 1 MB)"                           '[ "$(as adm -o /dev/null -w "%{size_download}" $BASE/api/audit)" -lt 1048576 ]'

echo "# V-25 status (pre-existing): truncation of the audit tail"
stop
python3 - "$A/data/nexus-fabric.sqlite" <<'PY'
import sqlite3, sys
db = sqlite3.connect(sys.argv[1]); n = db.execute("SELECT MAX(id) FROM audit").fetchone()[0]
db.execute("DELETE FROM audit WHERE id > ?", (n - 5,)); db.commit()
PY
start "$A/data" "$A/key/storage.key" || { echo "server A did not restart"; exit 1; }
# sessions are stored in the database, so the administrator session survives the restart (sign-in is still throttled by V-33)
check "V-25 removal of the newest audit entries is detected"                   '[ "$(as adm $BASE/api/audit/verify | j "d[\"intact\"]")" = False ]'
stop

# ======================================================================== instance B (NF_COOKIE_SECURE=1, as in deploy/nf-server.service)
B="$TMP/b"; mkdir -p "$B/key"
start "$B/data" "$B/key/storage.key" NF_COOKIE_SECURE=1 || { echo "server B did not start"; exit 1; }
echo "# V-36 anonymous first-run setup"
R=$(code -H "$X" -H "$CT" -d '{"username":"claimer","password":"claimer-password-1"}' "$BASE/api/setup")
check "V-36 anonymous setup without a one-time setup secret is refused"        '[ "$R" != 200 ]'
if [ "$R" = 200 ]; then BA=claimer; BP=claimer-password-1; else BA=admin; BP=admin-password-r6; curl -s -o /dev/null "${SETUP_ARGS[@]}" -H "$X" -H "$CT" -d "{\"username\":\"$BA\",\"password\":\"$BP\"}" "$BASE/api/setup"; fi
echo "# V-39 session cookie"
SC=$(curl -s -D - -o /dev/null -c "$TMP/b.jar" -H "$X" -H "$CT" -d "{\"username\":\"$BA\",\"password\":\"$BP\"}" "$BASE/api/login" | grep -i '^set-cookie')
check "V-39 session cookie uses the __Host- prefix"                             'echo "$SC" | grep -q "__Host-"'
LC=$(curl -s -D - -o /dev/null -b "$TMP/b.jar" -H "$X" -X POST "$BASE/api/logout" | grep -i '^set-cookie')
check "V-39 sign-out cookie carries Secure"                                     'echo "$LC" | grep -qi "secure"'
curl -s -o /dev/null -c "$TMP/b.jar" -H "$X" -H "$CT" -d "{\"username\":\"$BA\",\"password\":\"$BP\"}" "$BASE/api/login"
BP_ID=$(curl -s -b "$TMP/b.jar" -H "$X" -H "$CT" -d '{"name":"key test"}' "$BASE/api/projects" | j 'd["id"]')
curl -s -o /dev/null -b "$TMP/b.jar" -H "$X" -H 'X-Filename: k.pdf' --data-binary $'%PDF-1.7\nkey test' "$BASE/api/projects/$BP_ID/documents"
stop
echo "# V-42 storage key handling"
if start "$B/data" "$B/other-key-path/typo.key"; then R=started; stop; else R=refused; fi
check "V-42 server refuses to start with a new key when encrypted data exists" '[ "$R" = refused ]'
chmod 644 "$B/key/storage.key"
if start "$B/data" "$B/key/storage.key"; then R=started; stop; else R=refused; fi
check "V-42 server refuses a world-readable key file"                           '[ "$R" = refused ]'

# ======================================================================== static checks of tests/ and deploy/
echo "# V-38 test harness may talk to a server it did not start"
check "V-38 security_smoke.sh refuses an occupied port / checks its own process" 'grep -q "kill -0" tests/security_smoke.sh'
check "V-38 e2e_smoke.sh refuses an occupied port / checks its own process"      'grep -q "kill -0" tests/e2e_smoke.sh'
check "V-38 documented live port differs from the test default ports"            '! grep -q -e "127.0.0.1:8792" -e "127.0.0.1:8791" deploy/README.md'
echo "# V-44 deployment hardening"
U=deploy/nf-server.service
for d in "UMask=0077" "IPAddressDeny=any" "ProtectProc=invisible" "RestrictSUIDSGID=true" "SystemCallArchitectures=native" "ProtectKernelLogs=true"; do
  check "V-44 unit sets $d" "grep -q '^$d' $U"
done
check "V-44 install.sh validates DOMAIN before writing it into the Caddyfile"   'grep -Eq "DOMAIN.*=~|case .{1,3}DOMAIN" deploy/install.sh'
check "V-44 install.sh does not pipe a download into a shell"                   '! grep -Eq "curl[^|]*\|[[:space:]]*(ba)?sh" deploy/install.sh'
check "V-44 install.sh does not build as root"                                  'grep -Eq "runuser|sudo -u|setpriv" deploy/install.sh'

echo "----"; echo "passed: $pass  failed: $fail  skipped: $skip"; [ "$fail" = 0 ]
