#!/usr/bin/env bash
# Offline check: proves the program needs no network other than the machine itself.
#  1. static: no outbound-capable library in the server's dependency tree, no external address in the client,
#     the content policy allows connections to the program itself only;
#  2. dynamic: the API suite, the browser suite and the desktop-mode suite run inside a network namespace that has
#     only the loopback interface (needs root and `unshare`; skipped with a clear message otherwise).
# Usage: bash tests/offline_check.sh            (build first: cargo build --release)
#        NF_OFFLINE_DYNAMIC=0 bash tests/offline_check.sh   static part only; the dynamic part is reported as not run
#        (for a host that also runs a live service, where the root part must not run: CLI_WORK_PLAN.md task 0.1)
# Exits 0 only if every part that ran passed. Development build. Not validated. Not for manufacturing.
set -u
cd "$(dirname "$0")/.."
fail=0
say() { printf '%s\n' "$*"; }

bad=$(cargo tree -p nf-server 2>/dev/null | grep -iE 'reqwest|hyper-tls|native-tls|rustls|openssl|ureq|curl|isahc|surf|trust-dns|hickory|lettre|sentry|opentelemetry' | sort -u)
if [ -z "$bad" ]; then say "ok   - no outbound-capable library in the dependency tree"; else say "FAIL - outbound-capable libraries: $bad"; fail=1; fi

ext=$(grep -noE 'https?://[a-zA-Z0-9./_-]+' web/*.js web/*.css web/*.html | grep -vE '127\.0\.0\.1|localhost|www\.w3\.org')
if [ -z "$ext" ]; then say "ok   - no external address in the client"; else say "FAIL - external addresses in the client: $ext"; fail=1; fi

if grep -q "connect-src 'self'" crates/nf-server/src/main.rs && grep -q "default-src 'none'" crates/nf-server/src/main.rs; then
  say "ok   - content policy: default none, connections to the program itself only"
else say "FAIL - content policy does not restrict connections to the program itself"; fail=1; fi

if [ "${NF_OFFLINE_DYNAMIC:-1}" = 0 ]; then
  say "SKIP - dynamic part disabled by NF_OFFLINE_DYNAMIC=0; not run"; exit $fail
fi
if [ "$(id -u)" != 0 ] || ! command -v unshare >/dev/null; then
  say "SKIP - dynamic part needs root and unshare; not run"; exit $fail
fi
BIN=${NF_BIN:-target/release/nf-server}
[ -x "$BIN" ] || { say "FAIL - $BIN not built"; exit 1; }
out=$(NF_BIN="$BIN" unshare -n bash -c '
python3 - <<P
import socket, fcntl, struct
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
f = struct.unpack("16sH", fcntl.ioctl(s, 0x8913, struct.pack("16sH", b"lo", 0) + b"\0" * 14)[:18])[1]
fcntl.ioctl(s, 0x8914, struct.pack("16sH", b"lo", f | 1) + b"\0" * 14)
P
if curl -s -m 4 -o /dev/null https://example.com; then echo "NET reachable"; else echo "NET none"; fi
echo "API $(bash tests/e2e_smoke.sh 2>&1 | tail -1)"
D=$(mktemp -d)
(NF_SETUP_TOKEN=ui-smoke-setup-token NF_DATA_DIR=$D NF_LISTEN=127.0.0.1:8793 "$NF_BIN" >/dev/null 2>&1 & echo $! > $D/pid); sleep 1.5
echo "BROWSER $(NF_BASE=http://127.0.0.1:8793 node tests/ui_smoke_v2.js 2>&1 | grep -E "passed, [0-9]+ failed" | tail -1)"
kill $(cat $D/pid); rm -rf $D
echo "DESKTOP $(node tests/desktop_smoke.js "$NF_BIN" 8867 2>&1 | grep -E "^passed" | tail -1)"
' 2>/dev/null)
say "$out" | sed 's/^/     /'
echo "$out" | grep -q '^NET none' && say "ok   - the sandbox has no route to the internet" || { say "FAIL - the sandbox reached the internet, so the dynamic part proves nothing"; fail=1; }
for s in API BROWSER DESKTOP; do
  line=$(echo "$out" | grep "^$s ")
  if echo "$line" | grep -qE 'passed: [1-9][0-9]* +failed: 0'; then say "ok   - $s suite passes with no network"; else say "FAIL - $s suite without network: ${line:-no result}"; fail=1; fi
done
exit $fail
