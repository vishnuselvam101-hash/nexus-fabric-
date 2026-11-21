#!/usr/bin/env bash
# Installer test (V-44): runs deploy/install.sh against a throw-away root (NF_INSTALL_ROOT) with stand-ins for
# systemctl, useradd, apt-get, caddy, ufw, journalctl, chown, curl and cargo first on PATH. Covers malformed input
# (also the administrator's password: file rules, the refused environment variable), insufficient privileges, the
# build as an unprivileged account (never as root), a prebuilt binary and its checksum, a port in use, an install
# interrupted during the build and between the switch and the restart, re-runs after both, rollback when the new
# version does not answer its health check (the database is rolled back with the binary), the success path with the
# real server, the instructions printed for an administrator that does not exist yet, a missing Caddy (the key
# download is attempted and is the only download), and a Caddyfile that does not validate (refused, unchanged).
#
# Run ONLY inside the strict sandbox (whole file system read-only except a private /tmp, private network):
#   NF_SANDBOX_STRICT=1 nf-sandbox bash tests/install_check.sh
# It refuses to run anywhere it could write a real path. Needs root inside the sandbox, python3, curl, runuser,
# setpriv, setsid, ss, and target/release/nf-server (build first; NF_BIN=<path> for another binary).
# NF_DEPLOY_DIR=<dir> runs the same checks against the installer in another directory (for example an older version).
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
REPO=$PWD
DEPLOY="${NF_DEPLOY_DIR:-$REPO/deploy}"
REAL_BIN="${NF_BIN:-$REPO/target/release/nf-server}"

# ---------------------------------------------------------------- refuse to run outside the strict sandbox
[ "$(id -u)" = 0 ] || { echo "run as root inside the strict sandbox: NF_SANDBOX_STRICT=1 nf-sandbox bash tests/install_check.sh"; exit 2; }
probe="$REPO/.install-check-probe.$$"
if (: >"$probe") 2>/dev/null; then rm -f "$probe"; echo "refusing to run: $REPO is writable, so this is not the strict sandbox"; exit 2; fi
for d in /usr/local/bin /etc /var/lib /root; do
  if [ -w "$d" ]; then echo "refusing to run: $d is writable, so this is not the strict sandbox"; exit 2; fi
done
[ "$(ls /sys/class/net)" = lo ] || { echo "refusing to run: the network is not private (only lo expected)"; exit 2; }
[ -x "$REAL_BIN" ] || { echo "no server binary at $REAL_BIN (build first)"; exit 2; }
[ -f "$DEPLOY/install.sh" ] || { echo "no installer at $DEPLOY/install.sh"; exit 2; }

pass=0; fail=0
check() { if eval "$2"; then pass=$((pass+1)); echo "ok   - $1"; else fail=$((fail+1)); echo "FAIL - $1"; fi; }
sha() { sha256sum "$1" | cut -d' ' -f1; }

# ---------------------------------------------------------------- the throw-away world
T="$(mktemp -d /tmp/nf-install-check.XXXXXX)"; chmod 0755 "$T"
R="$T/root"; B="$T/bin"; S="$T/state"; L="$T/log"; M="$T/marks"; P="$T/payload"; CO="$T/checkout"; BD="$T/build"; TC="$T/toolchain"
mkdir -p "$R"/usr/local/bin "$R"/usr/local/lib "$R"/etc/systemd/system "$R"/etc/caddy "$R"/var/lib "$R"/var/log \
  "$R"/usr/share/keyrings "$R"/etc/apt/sources.list.d "$B" "$S" "$L" "$M" "$P" "$CO" "$BD" "$TC/good" "$TC/broken" "$T/out" "$T/rootdir"
chmod 0777 "$L" "$M"; chmod 0755 "$S" "$P"
chown 65534:65534 "$BD"
BIN="$R/usr/local/bin/nf-server"; UNIT="$R/etc/systemd/system/nf-server.service"; KEY="$R/etc/nexus-fabric/master.key"
STATE="$R/usr/local/lib/nexus-fabric"; PREV="$STATE/previous"
LISTENER=""
# password files for the first administrator: root-owned mode 600 (usable), 644, owned by nobody, with a quote, with a newline
PWF="$T/pw-good"; PW644="$T/pw-644"; PWNOBODY="$T/pw-nobody"; PWQUOTE="$T/pw-quote"; PWNL="$T/pw-newline"
( umask 077; printf 'correct-horse-9\n' >"$PWF"; printf 'correct-horse-9\n' >"$PWNOBODY"; printf 'correct"horse-9\n' >"$PWQUOTE"
  printf 'correct-horse-9\n"role":"x\n' >"$PWNL" )
( umask 022; printf 'correct-horse-9\n' >"$PW644" ); chmod 0644 "$PW644"
chown 65534:65534 "$PWNOBODY"
cleanup() {
  [ -f "$S/service.pid" ] && kill "$(cat "$S/service.pid")" 2>/dev/null
  [ -n "$LISTENER" ] && kill "$LISTENER" 2>/dev/null
  [ -f "$M/building.pid" ] && kill "$(cat "$M/building.pid")" 2>/dev/null
  rm -rf "$T"
}
trap cleanup EXIT

# a checkout: the installer under test and the files it packs for the build (their content is not compiled here)
mkdir -p "$CO/deploy" "$CO/crates/nf-server" "$CO/web"
cp "$DEPLOY"/* "$CO/deploy/"
echo '[workspace] # placeholder: the stand-in cargo builds nothing' >"$CO/Cargo.toml"; : >"$CO/Cargo.lock"
echo placeholder >"$CO/crates/nf-server/README"; echo placeholder >"$CO/web/README"

# stand-in servers: one that answers /api/health with its version, one that never starts
fake() {
  cat >"$1" <<EOF
#!/bin/bash
exec python3 -c '
import http.server, os
host, port = os.environ["NF_LISTEN"].rsplit(":", 1)
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = b"{\"status\":\"ok\",\"stand_in\":\"$2\"}"
        self.send_response(200 if self.path == "/api/health" else 404)
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *a): pass
http.server.HTTPServer((host, int(port)), H).serve_forever()
'
EOF
  chmod 0755 "$1"
}
cp "$REAL_BIN" "$P/real"; chmod 0755 "$P/real"
fake "$P/v1" v1; fake "$P/v2" v2; fake "$P/v3" v3
printf '#!/bin/bash\n# stand-in v4: migrates the database to a newer schema (user_version 999), then does not start\n'\
'sqlite3 "$NF_DATA_DIR/nexus-fabric.sqlite" "PRAGMA user_version=999"\necho "stand-in v4: does not start" >&2\nexit 1\n' >"$P/v4"; chmod 0755 "$P/v4"
SHA_REAL=$(sha "$P/real"); SHA_V1=$(sha "$P/v1"); SHA_V2=$(sha "$P/v2"); SHA_V3=$(sha "$P/v3"); SHA_V4=$(sha "$P/v4")

# ---------------------------------------------------------------- stand-in commands
cat >"$B/systemctl" <<EOF
#!/bin/bash
# stand-in for systemctl: "nf-server" is a background process started from the installed unit, its paths moved below
# the test root. The pause point lets the test interrupt the installer at daemon-reload.
echo "systemctl \$*" >>"$L/systemctl.log"
pidf="$S/service.pid"
running() { [ -f "\$pidf" ] && kill -0 "\$(cat "\$pidf")" 2>/dev/null; }
stop() {
  if running; then kill "\$(cat "\$pidf")"; for _ in \$(seq 1 50); do running || break; sleep 0.1; done; fi
  rm -f "\$pidf"
}
start() {
  local unit="$R/etc/systemd/system/nf-server.service" envs=() kv exe
  [ -f "\$unit" ] || return 1
  while IFS= read -r kv; do
    kv="\${kv#Environment=}"
    case "\$kv" in NF_DATA_DIR=/*|NF_KEY_FILE=/*) kv="\${kv%%=*}=$R\${kv#*=}" ;; esac
    envs+=("\$kv")
  done < <(grep '^Environment=' "\$unit")
  exe="\$(sed -n 's/^ExecStart=//p' "\$unit")"
  setsid env -i PATH=/usr/bin:/bin "\${envs[@]}" "$R\$exe" >>"$L/service.log" 2>&1 9<&- </dev/null &
  echo \$! >"\$pidf"
}
case "\${1:-}" in
  is-active) [ "\${2:-}" = --quiet ] && shift; [ "\${2:-}" = nf-server ] && running ;;
  daemon-reload) if [ "\$(cat "$S/pause-at" 2>/dev/null)" = daemon-reload ]; then echo \$\$ >"$M/daemon-reload"; exec sleep 120; fi ;;
  restart) if [ "\${2:-}" = nf-server ]; then stop; start; fi ;;
  stop) if [ "\${2:-}" = nf-server ]; then stop; fi ;;
  *) ;;
esac
EOF
for c in useradd apt-get chown; do printf '#!/bin/bash\necho "%s $*" >>"%s/%s.log"\n' "$c" "$L" "$c" >"$B/$c"; done
printf '#!/bin/bash\n[ "$1" = validate ] && [ -f "%s/caddy-invalid" ] && { echo "stand-in: invalid"; exit 1; }\nexit 0\n' "$S" >"$B/caddy"
printf '#!/bin/bash\n[ "$1" = status ] && echo "Status: inactive"\nexit 0\n' >"$B/ufw"
printf '#!/bin/bash\necho "-- stand-in journal --"; tail -n 3 "%s/service.log" 2>/dev/null; exit 0\n' "$L" >"$B/journalctl"
# curl: loopback only; anything else is recorded as a download attempt and fails
cat >"$B/curl" <<EOF
#!/bin/bash
for a in "\$@"; do case "\$a" in http://127.0.0.1:*) exec /usr/bin/curl "\$@" ;; esac; done
echo "curl \$*" >>"$L/downloads.log"; exit 7
EOF
# cargo: "builds" the payload the test chose, and records the user id it ran as
cat >"$TC/good/cargo" <<EOF
#!/bin/bash
echo "\$(id -u) \$*" >>"$L/cargo.log"
case "\${1:-}" in --version) echo "cargo 1.85.0 (stand-in)"; exit 0 ;; build) ;; *) exit 0 ;; esac
td="\$PWD/target"; prev=""
for a in "\$@"; do [ "\$prev" = --target-dir ] && td="\$a"; prev="\$a"; done
if [ -f "$S/pause-build" ]; then echo \$\$ >"$M/building.pid"; touch "$M/building"; exec sleep 120; fi
mkdir -p "\$td/release" && cp "\$(cat "$S/build-payload")" "\$td/release/nf-server"
EOF
printf '#!/bin/bash\necho "cargo: this toolchain does not run (stand-in)" >&2\nexit 1\n' >"$TC/broken/cargo"
chmod 0755 "$B"/* "$TC"/good/cargo "$TC"/broken/cargo

SYSPATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
TPATH="$B:$TC/good:$SYSPATH"
for c in systemctl useradd apt-get caddy ufw journalctl chown curl; do
  [ "$(PATH=$TPATH command -v $c)" = "$B/$c" ] || { echo "stand-in for $c is not first on PATH; refusing to run"; exit 2; }
done
[ "$(PATH=$TPATH command -v cargo)" = "$TC/good/cargo" ] || { echo "stand-in cargo is not first on PATH; refusing to run"; exit 2; }

# run_install <name> [VAR=value ...]: the installer as root, in its own session, with a clean environment
run_install() {
  local name=$1; shift
  ( cd / && exec setsid env -i PATH="$TPATH" HOME=/root LANG=C.UTF-8 NF_INSTALL_ROOT="$R" "$@" bash "$CO/deploy/install.sh" ) >"$T/out/$name.log" 2>&1 </dev/null
}
run_install_bg() { # the same in the background; sets BG to its process (and process group) id
  local name=$1; shift
  ( cd / && exec setsid env -i PATH="$TPATH" HOME=/root LANG=C.UTF-8 NF_INSTALL_ROOT="$R" "$@" bash "$CO/deploy/install.sh" ) >"$T/out/$name.log" 2>&1 </dev/null &
  BG=$!
}
wait_for() { local _; for _ in $(seq 1 "$2"); do [ -e "$1" ] && return 0; sleep 0.2; done; return 1; }
snap() { (cd "$R" && find . -printf '%p %y %m\n' | sort && find . -type f -exec sha256sum {} + | sort); }
count() { local n; n="$(grep -c -- "$2" "$1" 2>/dev/null)"; echo "${n:-0}"; }   # lines of file $1 that contain $2
restarts() { count "$L/systemctl.log" "restart nf-server"; }
health() { curl -fs --max-time 2 "http://127.0.0.1:$1/api/health"; }
answers_as() { health "$1" | grep -q "\"stand_in\":\"$2\""; }
logline() { tail -n 3 "$T/out/$1.log" | tr '\n' ' ' | cut -c1-200; }

# refused <name> <message pattern> [VAR=value ...]: the run fails with that message and changes nothing
refused() {
  local name=$1 re=$2; shift 2
  local log before rs ap cg rc why=""
  log="$(printf '%s' "$name" | tr -c 'A-Za-z0-9._=-' '_')"
  before="$(snap)"; rs=$(restarts); ap=$(count "$L/apt-get.log" .); cg=$(count "$L/cargo.log" " build")
  run_install "$log" "$@"; rc=$?
  [ "$rc" != 0 ] || why+=" exit 0;"
  grep -Eq -- "$re" "$T/out/$log.log" || why+=" message missing;"
  [ "$before" = "$(snap)" ] || why+=" files changed: $(diff <(echo "$before") <(snap) | grep '^[<>]' | head -3 | tr '\n' ' ');"
  [ "$(restarts)" = "$rs" ] || why+=" service restarted;"
  [ "$(count "$L/apt-get.log" .)" = "$ap" ] || why+=" packages installed;"
  [ "$(count "$L/cargo.log" " build")" = "$cg" ] || why+=" a build ran;"
  check "$name: refused, nothing changed ($(logline "$log"))${why:+ [$why]}" '[ -z "$why" ]'
}

VALID=(DOMAIN=app.example.test NF_PORT=18787)
ACCOUNT=(NF_BUILD_USER=nobody "NF_BUILD_DIR=$BD")

echo "# malformed input: refused before anything changes"
refused "no DOMAIN" "set DOMAIN" "${ACCOUNT[@]}"
long_label="$(printf 'a%.0s' $(seq 1 64)).example.test"; long_name="$(printf 'abcdefghi.%.0s' $(seq 1 25))test"
for d in "app.example.test/evil" "app example.test" "app.example.test{" $'app.example.test\nevil.example' ".app.example.test" \
         "app.example.test." "app..example.test" "-app.example.test" "app-.example.test" "app_1.example.test" "*.example.test" \
         "app.example.test:443" "$long_label" "$long_name"; do
  refused "DOMAIN=$(printf %q "$d" | cut -c1-40)" "DOMAIN must be a host name" "DOMAIN=$d" NF_PORT=18787 "${ACCOUNT[@]}"
done
for p in 80 1023 65536 8787x abc 08787 -1 99999999999999999999 " 8787"; do
  refused "NF_PORT='$p'" "NF_PORT must be a number" DOMAIN=app.example.test "NF_PORT=$p" "${ACCOUNT[@]}"
done
refused "a prebuilt binary without its checksum" "needs NF_BINARY=" "${VALID[@]}" "NF_BINARY=$P/v1"
refused "a checksum without a binary" "needs NF_BINARY=" "${VALID[@]}" "NF_BINARY_SHA256=$SHA_V1"
refused "a checksum of 63 digits" "64 hexadecimal digits" "${VALID[@]}" "NF_BINARY=$P/v1" "NF_BINARY_SHA256=${SHA_V1:0:63}"
refused "a checksum that is not hexadecimal" "64 hexadecimal digits" "${VALID[@]}" "NF_BINARY=$P/v1" "NF_BINARY_SHA256=$(printf 'z%.0s' $(seq 1 64))"
refused "NF_BINARY that is no file" "is not a file" "${VALID[@]}" "NF_BINARY=$T/nothing" "NF_BINARY_SHA256=$SHA_V1"
refused "a prebuilt binary with the wrong checksum" "does not match NF_BINARY_SHA256" "${VALID[@]}" "NF_BINARY=$P/v1" "NF_BINARY_SHA256=$SHA_V2"
refused "NF_ADMIN_USER without a password (not a terminal)" "needs a password" "${VALID[@]}" "${ACCOUNT[@]}" NF_ADMIN_USER=admin
refused "NF_ADMIN_PASSWORD in the environment (refused)" "NF_ADMIN_PASSWORD is refused" "${VALID[@]}" "${ACCOUNT[@]}" NF_ADMIN_USER=admin NF_ADMIN_PASSWORD=correct-horse-9
refused "NF_ADMIN_PASSWORD in the environment, empty" "NF_ADMIN_PASSWORD is refused" "${VALID[@]}" "${ACCOUNT[@]}" NF_ADMIN_USER=admin NF_ADMIN_PASSWORD=
refused "NF_ADMIN_PASSWORD_FILE without NF_ADMIN_USER" "needs NF_ADMIN_USER" "${VALID[@]}" "${ACCOUNT[@]}" NF_ADMIN_PASSWORD_FILE=$PWF
refused "NF_ADMIN_USER with a space" "may contain letters" "${VALID[@]}" "${ACCOUNT[@]}" "NF_ADMIN_USER=a b" NF_ADMIN_PASSWORD_FILE=$PWF
refused "password file that does not exist" "is not a regular file" "${VALID[@]}" "${ACCOUNT[@]}" NF_ADMIN_USER=admin NF_ADMIN_PASSWORD_FILE=$T/nothing
refused "password file with mode 644" "must be mode 600 and owned by root" "${VALID[@]}" "${ACCOUNT[@]}" NF_ADMIN_USER=admin NF_ADMIN_PASSWORD_FILE=$PW644
refused "password file owned by nobody" "must be mode 600 and owned by root" "${VALID[@]}" "${ACCOUNT[@]}" NF_ADMIN_USER=admin NF_ADMIN_PASSWORD_FILE=$PWNOBODY
refused "password file with a double quote" "must not contain a double quote" "${VALID[@]}" "${ACCOUNT[@]}" NF_ADMIN_USER=admin NF_ADMIN_PASSWORD_FILE=$PWQUOTE
refused "password file with a newline inside" "must not contain a double quote" "${VALID[@]}" "${ACCOUNT[@]}" NF_ADMIN_USER=admin NF_ADMIN_PASSWORD_FILE=$PWNL

echo "# insufficient privileges"
before="$(snap)"
( cd / && exec setsid env -i PATH="$TPATH" HOME=/ LANG=C.UTF-8 NF_INSTALL_ROOT="$R" "${VALID[@]}" "${ACCOUNT[@]}" \
    setpriv --reuid=65534 --regid=65534 --clear-groups bash "$CO/deploy/install.sh" ) >"$T/out/not-root.log" 2>&1; rc=$?
check "not root (uid 65534): refused, nothing changed ($(logline not-root))" '[ $rc != 0 ] && grep -q "run as root" "$T/out/not-root.log" && [ "$before" = "$(snap)" ]'
refused "root, no unprivileged account named, a checkout owned by root, no prebuilt binary" "refusing to build as root" "${VALID[@]}"
refused "NF_BUILD_USER=root" "refusing to build as root" "${VALID[@]}" NF_BUILD_USER=root
refused "sudo from root (SUDO_USER=root)" "refusing to build as root" "${VALID[@]}" SUDO_USER=root
refused "NF_BUILD_USER that names no account" "does not exist" "${VALID[@]}" NF_BUILD_USER=nf-no-such-account
refused "a build account whose cargo does not run" "no working Rust toolchain" "${VALID[@]}" "${ACCOUNT[@]}" "PATH=$B:$TC/broken:$SYSPATH"
refused "a build directory the account cannot write" "cannot write its build directory" "${VALID[@]}" NF_BUILD_USER=nobody "NF_BUILD_DIR=$T/rootdir"
check "the stand-in cargo never ran as root" '! grep -q "^0 " "$L/cargo.log" 2>/dev/null'

echo "# a port that is in use"
python3 -m http.server --bind 127.0.0.1 18999 >/dev/null 2>&1 & LISTENER=$!
for _ in $(seq 1 25); do curl -s -o /dev/null http://127.0.0.1:18999/ && break; sleep 0.2; done
refused "NF_PORT=18999, where another program listens" "already in use" DOMAIN=app.example.test NF_PORT=18999 "${ACCOUNT[@]}"
kill "$LISTENER" 2>/dev/null; wait "$LISTENER" 2>/dev/null; LISTENER=""

echo "# success: fresh install, built by the account that ran sudo, with the real server"
printf '%s' "$P/real" >"$S/build-payload"
run_install fresh "${VALID[@]}" SUDO_USER=nobody "NF_BUILD_DIR=$BD" NF_ADMIN_USER=admin NF_ADMIN_PASSWORD_FILE=$PWF; rc=$?
check "fresh install: exit 0 ($(logline fresh))" '[ $rc = 0 ]'
check "  built as uid 65534 in its own build directory; the checkout was not written" 'grep -q "^65534 build .*--target-dir $BD/target" "$L/cargo.log" && [ -f "$BD/src/Cargo.toml" ] && [ ! -e "$CO/target" ]'
check "  the installed binary is the built one, mode 755" '[ "$(sha "$BIN")" = "$SHA_REAL" ] && [ "$(stat -c %a "$BIN")" = 755 ]'
check "  unit: 127.0.0.1:18787, host app.example.test, service account nexusfabric" 'grep -qx "Environment=NF_LISTEN=127.0.0.1:18787" "$UNIT" && grep -qx "Environment=NF_ALLOWED_HOSTS=app.example.test" "$UNIT" && grep -qx "User=nexusfabric" "$UNIT"'
check "  key: 32 bytes, mode 440, given to root:nexusfabric" '[ "$(stat -c %s "$KEY")" = 32 ] && [ "$(stat -c %a "$KEY")" = 440 ] && grep -q "chown root:nexusfabric $KEY.new" "$L/chown.log"'
check "  data directory mode 700, given to nexusfabric" '[ "$(stat -c %a "$R/var/lib/nexus-fabric")" = 700 ] && grep -q "chown nexusfabric:nexusfabric $R/var/lib/nexus-fabric" "$L/chown.log"'
check "  the real server answers on 127.0.0.1:18787" 'health 18787 | grep -q "\"status\""'
check "  first administrator created over loopback; it can sign in; the password is in no output" '[ ! -e "$R/var/lib/nexus-fabric/setup-token" ] && [ "$(curl -s -o /dev/null -w "%{http_code}" -H "X-NF-Request: 1" -H "Content-Type: application/json" -d "{\"username\":\"admin\",\"password\":\"correct-horse-9\"}" http://127.0.0.1:18787/api/login)" = 200 ] && ! grep -rq correct-horse-9 "$T/out"'
check "  Caddy site block for the domain and the port" 'grep -q "^app.example.test {" "$R/etc/caddy/Caddyfile" && grep -q "reverse_proxy 127.0.0.1:18787" "$R/etc/caddy/Caddyfile"'
check "  the installed version is recorded as working" '[ "$(cat "$STATE/verified")" = "$(sha "$BIN") $(sha "$UNIT")" ]'
KEYSUM=$(sha "$KEY")

echo "# upgrade to a prebuilt binary with its checksum"
cg=$(count "$L/cargo.log" " build")
run_install upgrade-v1 "${VALID[@]}" "NF_BINARY=$P/v1" "NF_BINARY_SHA256=${SHA_V1^^}"; rc=$?
check "upgrade: exit 0, v1 answers ($(logline upgrade-v1))" '[ $rc = 0 ] && answers_as 18787 v1'
check "  nothing was built; the real server is kept for rollback; key and database kept" '[ "$(count "$L/cargo.log" " build")" = "$cg" ] && [ "$(sha "$PREV/nf-server")" = "$SHA_REAL" ] && [ "$(sha "$KEY")" = "$KEYSUM" ] && [ -f "$R/var/lib/nexus-fabric/nexus-fabric.sqlite" ]'
check "  the database copy kept with it is a consistent sqlite copy of the database" '[ "$(sqlite3 "$PREV/nexus-fabric.sqlite" "PRAGMA integrity_check")" = ok ] && [ "$(sqlite3 "$PREV/nexus-fabric.sqlite" "PRAGMA user_version")" = "$(sqlite3 "$R/var/lib/nexus-fabric/nexus-fabric.sqlite" "PRAGMA user_version")" ]'
refused "a wrong checksum on an installed system" "does not match NF_BINARY_SHA256" "${VALID[@]}" "NF_BINARY=$P/v2" "NF_BINARY_SHA256=$SHA_V3"
check "  v1 is still installed and answers" '[ "$(sha "$BIN")" = "$SHA_V1" ] && answers_as 18787 v1'

echo "# interrupted during the build, then run again"
/usr/bin/chown -R 65534:65534 "$CO"   # the build account is now the owner of the checkout
printf '%s' "$P/v2" >"$S/build-payload"; touch "$S/pause-build"; rs=$(restarts)
run_install_bg interrupted-build "${VALID[@]}" "NF_BUILD_DIR=$BD"
if wait_for "$M/building" 150; then kill -KILL -- "-$BG" 2>/dev/null; fi
wait "$BG" 2>/dev/null; kill "$(cat "$M/building.pid" 2>/dev/null)" 2>/dev/null; rm -f "$S/pause-build" "$M/building"
check "interrupted while building: v1 is still installed and answers, no restart, rollback copy unchanged" '[ "$(sha "$BIN")" = "$SHA_V1" ] && answers_as 18787 v1 && [ "$(restarts)" = "$rs" ] && [ "$(sha "$PREV/nf-server")" = "$SHA_REAL" ]'
run_install rerun-build "${VALID[@]}" "NF_BUILD_DIR=$BD"; rc=$?
check "run again: exit 0, v2 (built as the owner of the checkout) answers, v1 kept for rollback ($(logline rerun-build))" '[ $rc = 0 ] && answers_as 18787 v2 && [ "$(sha "$PREV/nf-server")" = "$SHA_V1" ] && ! grep -q "^0 " "$L/cargo.log"'

echo "# interrupted between the switch and the restart, then run again"
echo daemon-reload >"$S/pause-at"
run_install_bg interrupted-switch "${VALID[@]}" "NF_BINARY=$P/v3" "NF_BINARY_SHA256=$SHA_V3"
if wait_for "$M/daemon-reload" 100; then kill -KILL -- "-$BG" 2>/dev/null; fi
wait "$BG" 2>/dev/null; rm -f "$S/pause-at" "$M/daemon-reload"
check "interrupted after the switch: the running v2 still answers; v2 is the rollback copy" 'answers_as 18787 v2 && [ "$(sha "$PREV/nf-server")" = "$SHA_V2" ] && [ "$(sha "$BIN")" = "$SHA_V3" ]'
run_install rerun-switch "${VALID[@]}" "NF_BINARY=$P/v3" "NF_BINARY_SHA256=$SHA_V3"; rc=$?
check "run again: exit 0, v3 answers; the rollback copy is still v2, not the unchecked v3 ($(logline rerun-switch))" '[ $rc = 0 ] && answers_as 18787 v3 && [ "$(sha "$PREV/nf-server")" = "$SHA_V2" ]'

echo "# rollback: the new version does not answer its health check"
CADDYSUM=$(sha "$R/etc/caddy/Caddyfile"); DBF="$R/var/lib/nexus-fabric/nexus-fabric.sqlite"; UV_BEFORE=$(sqlite3 "$DBF" "PRAGMA user_version")
run_install rollback DOMAIN=app.example.test NF_PORT=18788 "NF_BINARY=$P/v4" "NF_BINARY_SHA256=$SHA_V4"; rc=$?
check "the install fails and says that the previous version was restored ($(logline rollback))" '[ $rc != 0 ] && grep -q "previous version and its database were restored" "$T/out/rollback.log"'
check "  v3 and its unit (port 18787) are back and answer; nothing on the new port" '[ "$(sha "$BIN")" = "$SHA_V3" ] && grep -qx "Environment=NF_LISTEN=127.0.0.1:18787" "$UNIT" && answers_as 18787 v3 && ! health 18788 >/dev/null'
check "  the database is rolled back with it: the schema is the old one again; the one the new version wrote is kept in failed-*" '[ "$(sqlite3 "$DBF" "PRAGMA user_version")" = "$UV_BEFORE" ] && [ "$UV_BEFORE" != 999 ] && [ "$(sqlite3 "$(ls "$STATE"/failed-*/nexus-fabric.sqlite | head -1)" "PRAGMA user_version")" = 999 ] && [ "$(sqlite3 "$PREV/nexus-fabric.sqlite" "PRAGMA user_version")" = "$UV_BEFORE" ]'
check "  the restored version is recorded as working; Caddy untouched" '[ "$(cat "$STATE/verified")" = "$(sha "$BIN") $(sha "$UNIT")" ] && [ "$(sha "$R/etc/caddy/Caddyfile")" = "$CADDYSUM" ]'
check "  no leftovers beside the installed files" '[ -z "$(find "$R" -name "*.new" -o -name "previous.old" | head -1)" ]'

echo "# the instructions printed for an administrator that does not exist yet"
printf 'token-not-to-print-0123\n' >"$R/var/lib/nexus-fabric/setup-token"
run_install printed "${VALID[@]}" "NF_BINARY=$P/v3" "NF_BINARY_SHA256=$SHA_V3"; rc=$?
rm -f "$R/var/lib/nexus-fabric/setup-token"
check "the command printed reads the password at a prompt and the token from a file; no secret in the output; the command is valid bash ($(logline printed))" \
  '[ $rc = 0 ] && grep -q "read -r -s -p" "$T/out/printed.log" && ! grep -q "token-not-to-print" "$T/out/printed.log" && ! grep -q "NF_ADMIN_PASSWORD" "$T/out/printed.log" && sed -n "/^  read -r -s/,/unset P/p" "$T/out/printed.log" | bash -n'

echo "# Caddy: missing from the search path, and a Caddyfile that does not validate"
NOCADDY="$T/nocaddy"; mkdir -p "$NOCADDY"
for d in "$B" /usr/local/sbin /usr/sbin /usr/bin /sbin /bin; do
  for f in "$d"/*; do n=${f##*/}; [ "$n" = caddy ] || [ -e "$NOCADDY/$n" ] || [ -L "$NOCADDY/$n" ] || ln -s "$f" "$NOCADDY/$n"; done
done
check "no caddy on the search path of this run (the host has one in /usr/bin)" '[ -z "$(PATH=$NOCADDY command -v caddy)" ]'
run_install no-caddy DOMAIN=app.example.test NF_PORT=18787 "NF_BINARY=$P/v3" "NF_BINARY_SHA256=$SHA_V3" "PATH=$NOCADDY"; rc=$?
check "without caddy the installer attempts Caddy's key download (refused by the stand-in) and stops there ($(logline no-caddy))" \
  '[ $rc != 0 ] && [ "$(count "$L/downloads.log" .)" = 1 ] && grep -q "dl.cloudsmith.io/public/caddy/stable/gpg.key" "$L/downloads.log"'
: >"$S/caddy-invalid"
CFSUM=$(sha "$R/etc/caddy/Caddyfile")
run_install caddy-invalid DOMAIN=other.example.test NF_PORT=18787 "NF_BINARY=$P/v3" "NF_BINARY_SHA256=$SHA_V3"; rc=$?
rm -f "$S/caddy-invalid"
check "a Caddyfile that does not validate is refused: the Caddyfile is unchanged, no backup, no leftover ($(logline caddy-invalid))" \
  '[ $rc != 0 ] && grep -q "does not validate" "$T/out/caddy-invalid.log" && grep -q "was left unchanged" "$T/out/caddy-invalid.log" && [ "$(sha "$R/etc/caddy/Caddyfile")" = "$CFSUM" ] && [ -z "$(find "$R/etc/caddy" -name "*.nf-new" -o -name "*.bak-*" | head -1)" ]'

echo "# every run"
check "the only download in any run: Caddy's repository key, once, in the run without caddy" '[ "$(count "$L/downloads.log" .)" = 1 ] && [ "$(count "$L/downloads.log" dl.cloudsmith.io/public/caddy/stable/gpg.key)" = 1 ]'
check "the stand-in cargo never ran as root, in any run" '! grep -q "^0 " "$L/cargo.log"'

echo "----"; echo "passed: $pass  failed: $fail"
[ "$fail" = 0 ]
