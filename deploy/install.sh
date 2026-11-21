#!/usr/bin/env bash
# Install or upgrade NEXUS FABRIC on a Linux server (systemd, Caddy in front). Run as root, from a checkout:
#   sudo DOMAIN=app.example.com [NF_PORT=8787] bash deploy/install.sh
# What it fetches from outside the distribution's packages: Caddy, when it is missing, from Caddy's apt repository. The
# repository's signing key is fetched and not checked against a pinned fingerprint (open: V-44 in
# docs/security/VULNERABILITY_REGISTER.md). When the server is built, cargo fetches the crates pinned in Cargo.lock
# (cargo build --locked). The server comes from one of two places:
#   - built from this checkout by an UNPRIVILEGED account that already has its own Rust toolchain: NF_BUILD_USER=<account>,
#     otherwise the account that ran sudo, otherwise the owner of the checkout. Building as root is refused. The
#     account gets a copy of the sources in its own build directory (NF_BUILD_DIR, default ~/.cache/nexus-fabric-build)
#     and never writes the checkout; root only reads the finished binary back from it;
#   - or a prebuilt binary: NF_BINARY=/path/to/nf-server NF_BINARY_SHA256=<its SHA-256>, checked before anything is installed.
# Re-runnable and safe to interrupt: everything is checked before the first change, and every file is written beside
# its target and renamed into place. Before the switch to a new version, the binary, the unit and a copy of the database
# are kept in /usr/local/lib/nexus-fabric/previous (the new version may migrate the database when it starts, and the old
# one refuses a newer schema). If a new version does not answer within 60 s, the old binary and unit are put back, the
# database copy is restored, and the old version is started again (automatic rollback). The service account, the data
# and the key are kept.
# Safe next to other sites: an existing Caddyfile is never overwritten (see deploy/caddy-site.sh), a port in use is
# refused, and the firewall is never enabled here.
# NF_INSTALL_ROOT is for tests only (tests/install_check.sh): a directory put in front of every path this script
# writes or reads, with stand-ins for systemctl, useradd, apt-get, caddy, ufw … first on PATH.
#
# The whole script is one function, read completely before it runs: nothing the build does to the checkout can
# change what root executes. The templates are copied before the build for the same reason.
set -euo pipefail
umask 022

die() { echo "install: $*" >&2; exit 1; }
warn() { echo "install: WARNING: $*" >&2; }
sha256() { sha256sum "$1" | cut -d' ' -f1; }
listen_port() { sed -n 's/^Environment=NF_LISTEN=.*:\([0-9][0-9]*\)$/\1/p' "$1" 2>/dev/null | head -1 || true; }
answers() { # answers PORT TRIES: the service answers its health check on 127.0.0.1:PORT (one try every 0.5 s)
  local _
  for _ in $(seq 1 "$2"); do
    if curl -fs -o /dev/null --max-time 2 "http://127.0.0.1:$1/api/health"; then return 0; fi
    sleep 0.5
  done
  return 1
}

main() {
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
  cd "$REPO"
  # shellcheck source=caddy-site.sh
  . "$SCRIPT_DIR/caddy-site.sh"

  # ---------------------------------------------------------------- 1. checks; nothing is changed before they pass
  [ "$(id -u)" = 0 ] || die "run as root: sudo DOMAIN=<host name> bash deploy/install.sh (the build itself runs unprivileged)"

  R="${NF_INSTALL_ROOT:-}"
  if [ -n "$R" ]; then
    case "$R" in /*) ;; *) die "NF_INSTALL_ROOT must be an absolute path" ;; esac
    [ -d "$R" ] || die "NF_INSTALL_ROOT: $R is not a directory"
    R="$(cd "$R" && pwd -P)"
    [ "$R" != / ] || die "NF_INSTALL_ROOT=/ is not a test root; leave it unset for a real install"
    echo "install: TEST MODE: every path below $R"
  fi
  BIN="$R/usr/local/bin/nf-server"
  UNIT="$R/etc/systemd/system/nf-server.service"
  DATA="$R/var/lib/nexus-fabric"
  CONF="$R/etc/nexus-fabric"
  KEY="$CONF/master.key"
  STATE="$R/usr/local/lib/nexus-fabric"
  PREV="$STATE/previous"

  DOMAIN="${DOMAIN:-}"
  [ -n "$DOMAIN" ] || die "set DOMAIN=<host name> (the name whose DNS record points at this server)"
  # Labels of letters, digits and inner hyphens, 1 to 63 characters, joined by single dots: nothing that could break out
  # of the Caddyfile or the unit file (no space, newline, brace, slash, wildcard, or leading or trailing dot).
  if ! [[ $DOMAIN =~ ^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*$ ]] || [ ${#DOMAIN} -gt 253 ]; then
    die "DOMAIN must be a host name such as app.example.com (letters, digits, hyphens and dots)"
  fi
  NF_PORT="${NF_PORT:-8787}"
  [[ $NF_PORT =~ ^[1-9][0-9]{3,4}$ ]] && [ "$NF_PORT" -ge 1024 ] && [ "$NF_PORT" -le 65535 ] \
    || die "NF_PORT must be a number from 1024 to 65535"

  # The first administrator (optional). The password never comes from the environment or a command line, where other
  # accounts and sudo's log can see it: it is read from a file (NF_ADMIN_PASSWORD_FILE, mode 600, owned by root) or typed
  # at a prompt when this script runs from a terminal.
  NF_ADMIN_USER="${NF_ADMIN_USER:-}"; NF_ADMIN_PASSWORD_FILE="${NF_ADMIN_PASSWORD_FILE:-}"; ADMIN_PASSWORD=""
  [ -z "${NF_ADMIN_PASSWORD+x}" ] || die "NF_ADMIN_PASSWORD is refused: a password in the environment or on a command line is visible to other accounts and to sudo's log. Give NF_ADMIN_PASSWORD_FILE=<file with mode 600, owned by root> instead, or run this script from a terminal and type the password when asked"
  if [ -n "$NF_ADMIN_USER" ] || [ -n "$NF_ADMIN_PASSWORD_FILE" ]; then
    [ -n "$NF_ADMIN_USER" ] || die "NF_ADMIN_PASSWORD_FILE needs NF_ADMIN_USER too (the name of the first administrator)"
    case "$NF_ADMIN_USER" in *[!A-Za-z0-9._-]*) die "NF_ADMIN_USER may contain letters, digits, . _ - only" ;; esac
    if [ -n "$NF_ADMIN_PASSWORD_FILE" ]; then
      [ -f "$NF_ADMIN_PASSWORD_FILE" ] && [ ! -L "$NF_ADMIN_PASSWORD_FILE" ] \
        || die "NF_ADMIN_PASSWORD_FILE: $NF_ADMIN_PASSWORD_FILE is not a regular file"
      [ "$(stat -c '%a %u' "$NF_ADMIN_PASSWORD_FILE")" = "600 0" ] \
        || die "NF_ADMIN_PASSWORD_FILE must be mode 600 and owned by root (chown root and chmod 600 the file)"
      ADMIN_PASSWORD="$(cat "$NF_ADMIN_PASSWORD_FILE")"
    elif [ -t 0 ]; then
      read -r -s -p "Password for the first administrator '$NF_ADMIN_USER' (at least 10 characters): " ADMIN_PASSWORD || die "no password was typed"
      echo >&2
    else
      die "NF_ADMIN_USER needs a password: give NF_ADMIN_PASSWORD_FILE=<file with mode 600, owned by root>, or run this script from a terminal"
    fi
    case "$ADMIN_PASSWORD" in *[\"\\]*|*[[:cntrl:]]*) die "the administrator password must not contain a double quote, a backslash or a control character (the setup request is built from it); create the account by hand instead" ;; esac
    [ -n "$ADMIN_PASSWORD" ] || die "the administrator password is empty"
  fi

  NF_BINARY="${NF_BINARY:-}"; NF_BINARY_SHA256="${NF_BINARY_SHA256:-}"
  if [ -n "$NF_BINARY" ] || [ -n "$NF_BINARY_SHA256" ]; then
    [ -n "$NF_BINARY" ] && [ -n "$NF_BINARY_SHA256" ] || die "a prebuilt binary needs NF_BINARY=<file> and NF_BINARY_SHA256=<its SHA-256>"
    [[ $NF_BINARY_SHA256 =~ ^[0-9A-Fa-f]{64}$ ]] || die "NF_BINARY_SHA256 must be the 64 hexadecimal digits of a SHA-256"
    [ -f "$NF_BINARY" ] || die "NF_BINARY: $NF_BINARY is not a file"
    NF_BINARY_SHA256="${NF_BINARY_SHA256,,}"
  fi

  if [ -r "$R/etc/os-release" ]; then . "$R/etc/os-release"; fi
  case "${ID:-}:${VERSION_ID:-}" in
    ubuntu:22.04|ubuntu:24.04|debian:12) ;;
    ubuntu:26.04) echo "note: Ubuntu 26.04 was verified by a manual install; this script has not been run end to end there." ;;
    *) warn "untested OS '${PRETTY_NAME:-unknown}'. Verified for: Ubuntu 22.04/24.04/26.04, Debian 12. Continuing." ;;
  esac

  # The build: an unprivileged account with its own Rust toolchain, working in its own directory.
  if [ -z "$NF_BINARY" ]; then
    BUILD_USER="${NF_BUILD_USER:-}"
    if [ -z "$BUILD_USER" ] && [ -n "${SUDO_USER:-}" ] && [ "$SUDO_USER" != root ]; then BUILD_USER="$SUDO_USER"; fi
    if [ -z "$BUILD_USER" ]; then
      owner_uid="$(stat -c %u "$REPO")"
      if [ "$owner_uid" != 0 ]; then BUILD_USER="$(getent passwd "$owner_uid" | cut -d: -f1 || true)"; fi
    fi
    [ -n "$BUILD_USER" ] || die "refusing to build as root: no unprivileged account was named, and the checkout belongs to root. Either
    - build as an account that has its own Rust toolchain: sudo NF_BUILD_USER=<account> DOMAIN=$DOMAIN bash deploy/install.sh
      (run from that account with sudo, it is chosen by itself), or
    - install a prebuilt binary: NF_BINARY=<file> NF_BINARY_SHA256=<its SHA-256> DOMAIN=$DOMAIN bash deploy/install.sh"
    BUILD_UID="$(id -u "$BUILD_USER" 2>/dev/null)" || die "the build account '$BUILD_USER' does not exist (NF_BUILD_USER)"
    [ "$BUILD_UID" != 0 ] || die "refusing to build as root ('$BUILD_USER' has user id 0); name an unprivileged account with NF_BUILD_USER"
    BUILD_HOME="$(getent passwd "$BUILD_USER" | cut -d: -f6 || true)"
    BUILD_DIR="${NF_BUILD_DIR:-${BUILD_HOME:-/nonexistent}/.cache/nexus-fabric-build}"
    case "$BUILD_DIR" in /*) ;; *) die "NF_BUILD_DIR must be an absolute path" ;; esac
    BUILD_PATH="${BUILD_HOME:-/nonexistent}/.cargo/bin:$PATH"
    # The build account runs with a clean environment, from /, without the lock (fd 9): none of root's variables,
    # none of root's files, and nothing it leaves running can hold up the next install.
    as_builder() {
      (cd / && exec runuser -u "$BUILD_USER" -- env -i HOME="${BUILD_HOME:-/nonexistent}" USER="$BUILD_USER" LOGNAME="$BUILD_USER" \
        PATH="$BUILD_PATH" LANG=C.UTF-8 ${RUSTUP_HOME:+RUSTUP_HOME="$RUSTUP_HOME"} \
        ${RUSTUP_TOOLCHAIN:+RUSTUP_TOOLCHAIN="$RUSTUP_TOOLCHAIN"} ${CARGO_HOME:+CARGO_HOME="$CARGO_HOME"} "$@" 9<&-)
    }
    as_builder cargo --version >/dev/null 2>&1 || die "the build account '$BUILD_USER' has no working Rust toolchain (cargo), and this script downloads none. Either
    - install Rust for that account, not for root: as $BUILD_USER, get rustup-init for this machine from https://rustup.rs,
      check it against its published SHA-256, run it, then run this script again (the code needs Rust 1.85 or newer), or
    - install a prebuilt binary: NF_BINARY=<file> NF_BINARY_SHA256=<its SHA-256>"
    { as_builder mkdir -p "$BUILD_DIR" 2>/dev/null && as_builder test -w "$BUILD_DIR"; } \
      || die "the build account '$BUILD_USER' cannot write its build directory $BUILD_DIR; name one it can write with NF_BUILD_DIR"
  fi

  # Port: refuse one that is already in use, unless it is this service's own current port.
  existing_port="$(listen_port "$UNIT")"
  own=0
  if systemctl is-active --quiet nf-server 2>/dev/null && [ "$existing_port" = "$NF_PORT" ]; then own=1; fi
  listeners="$(ss -ltnH "sport = :$NF_PORT" 2>/dev/null || true)"
  if [ "$own" = 0 ] && grep -q . <<<"$listeners"; then
    ss -ltnp "sport = :$NF_PORT" >&2 || true
    die "port $NF_PORT is already in use (listeners above); choose a free port with NF_PORT=<port>"
  fi

  # ---------------------------------------------------------------- 2. one run at a time; leftovers of an interrupted run
  mkdir -p "$(dirname "$BIN")" "$(dirname "$UNIT")"
  exec 9<"$(dirname "$BIN")"
  flock -n 9 || die "another run of this script is installing right now"
  TPL="$(mktemp -d)"
  trap 'rm -rf "$TPL"; rm -f "$BIN.new" "$UNIT.new" "$KEY.new"' EXIT
  cp "$SCRIPT_DIR/nf-server.service" "$SCRIPT_DIR/Caddyfile" "$TPL/"
  rm -f "$BIN.new" "$UNIT.new" "$KEY.new" "$STATE/verified.new"
  if [ -d "$STATE" ]; then
    if [ ! -d "$PREV" ] && [ -d "$STATE/previous.old" ]; then mv "$STATE/previous.old" "$PREV"; fi
    rm -rf "$STATE/previous.new" "$STATE/previous.old"
  fi

  # ---------------------------------------------------------------- 3. the new program, staged beside the installed one
  if [ -n "$NF_BINARY" ]; then
    install -m 0755 "$NF_BINARY" "$BIN.new"
    got="$(sha256 "$BIN.new")"
    if [ "$got" != "$NF_BINARY_SHA256" ]; then
      rm -f "$BIN.new"
      die "NF_BINARY does not match NF_BINARY_SHA256 (expected $NF_BINARY_SHA256, the file has $got); nothing was installed"
    fi
    echo "prebuilt binary checked: SHA-256 $got"
    apt-get update || warn "apt-get update failed; continuing with the package lists as they are"
    apt-get install -y curl ufw iproute2 sqlite3
  else
    apt-get update || warn "apt-get update failed; continuing with the package lists as they are"
    apt-get install -y build-essential curl ufw iproute2 sqlite3
    echo "building as $BUILD_USER in $BUILD_DIR with $(as_builder cargo --version)"
    # The account unpacks a copy of the sources in its own directory (a fresh copy every time; the compiled
    # dependencies in target/ are kept between runs) and builds there.
    # shellcheck disable=SC2016  # $1 is expanded by the build account's shell
    tar -C "$REPO" -cf - Cargo.toml Cargo.lock crates web | as_builder sh -c '
      set -e; cd "$1"; rm -rf src.new; mkdir src.new; tar -C src.new -xf -; rm -rf src; mv src.new src' sh "$BUILD_DIR"
    as_builder cargo build --release --locked -p nf-server --manifest-path "$BUILD_DIR/src/Cargo.toml" --target-dir "$BUILD_DIR/target"
    # Read as the build account, written by root: a link placed in the build directory cannot make root copy another file.
    as_builder cat "$BUILD_DIR/target/release/nf-server" > "$BIN.new"
    chmod 0755 "$BIN.new"
    echo "built binary: SHA-256 $(sha256 "$BIN.new")"
  fi

  # ---------------------------------------------------------------- 4. service account, directories, key
  getent passwd nexusfabric >/dev/null 2>&1 || useradd --system --home /var/lib/nexus-fabric --shell /usr/sbin/nologin nexusfabric
  install -d -m 0700 "$DATA"; chown nexusfabric:nexusfabric "$DATA"; chmod 0700 "$DATA"
  install -d -m 0750 "$CONF"; chown root:nexusfabric "$CONF"; chmod 0750 "$CONF"
  if [ ! -f "$KEY" ]; then
    # complete under another name first, then linked into place: an existing key is never replaced
    ( umask 077; head -c 32 /dev/urandom > "$KEY.new" )
    chown root:nexusfabric "$KEY.new"
    chmod 0440 "$KEY.new"
    ln "$KEY.new" "$KEY"
    rm -f "$KEY.new"
    echo "created $KEY (back it up; it is not shown here)"
  fi

  # ---------------------------------------------------------------- 5. switch to the new version, with rollback
  # The unit: the shipped file with the listen address and the public host name filled in; sandboxing untouched.
  # NF_ALLOWED_HOSTS: the server answers only to this name (and loopback); DOMAIN was checked above.
  sed -e "s|^Environment=NF_LISTEN=.*|Environment=NF_LISTEN=127.0.0.1:${NF_PORT}|" -e "s|^Environment=NF_ALLOWED_HOSTS=.*|Environment=NF_ALLOWED_HOSTS=${DOMAIN}|" "$TPL/nf-server.service" > "$UNIT.new"

  # The installed version is kept for rollback only when it is known to work: it is the binary and unit that last
  # answered the health check here, or (an installation older than that record) the service runs and answers now.
  install -d -m 0700 "$STATE"
  installed_id() { echo "$(sha256 "$BIN") $(sha256 "$UNIT")"; }
  record_verified() { installed_id > "$STATE/verified.new"; mv -f "$STATE/verified.new" "$STATE/verified"; }
  good=0
  if [ -f "$BIN" ] && [ -f "$UNIT" ]; then
    if [ -f "$STATE/verified" ]; then
      if [ "$(cat "$STATE/verified")" = "$(installed_id)" ]; then good=1; fi
    elif [ -n "$existing_port" ] && systemctl is-active --quiet nf-server 2>/dev/null && answers "$existing_port" 4; then
      good=1
    fi
  fi
  # The database: the new version may migrate it when it starts, and the old version refuses a newer schema. So the copy
  # kept with the old binary is the rollback's database. It is a consistent copy made with sqlite3 .backup while the
  # service keeps running; without sqlite3 the service is stopped (it folds its write-ahead log in) and the files are copied.
  DB="$DATA/nexus-fabric.sqlite"
  # snapshot_db DIR: copies the database into DIR (only when there is one). Returns 1 when the copy failed.
  snapshot_db() {
    [ -f "$DB" ] || return 0
    if command -v sqlite3 >/dev/null 2>&1; then
      sqlite3 "$DB" ".backup '$1/nexus-fabric.sqlite'" || return 1
    else
      systemctl stop nf-server || return 1
      cp -p "$DB" "$1/nexus-fabric.sqlite" || return 1
      if [ -e "$DB-wal" ]; then cp -p "$DB-wal" "$1/nexus-fabric.sqlite-wal" || return 1; fi
      if [ -e "$DB-shm" ]; then cp -p "$DB-shm" "$1/nexus-fabric.sqlite-shm" || return 1; fi
    fi
    chmod 0600 "$1/nexus-fabric.sqlite"
  }
  # restore_db: puts the database copy in PREV back. The database as the new version left it is moved to a failed-<time>
  # folder beside the state, not deleted. The stale -wal and -shm files go with it.
  restore_db() {
    local f failed
    failed="$STATE/failed-$(date +%Y%m%d%H%M%S)"
    for f in nexus-fabric.sqlite nexus-fabric.sqlite-wal nexus-fabric.sqlite-shm; do
      if [ -e "$DATA/$f" ]; then
        [ -d "$failed" ] || mkdir -m 0700 "$failed"
        mv -f "$DATA/$f" "$failed/$f"
      fi
    done
    cp "$PREV/nexus-fabric.sqlite" "$DATA/nexus-fabric.sqlite.new" \
      && chmod 0600 "$DATA/nexus-fabric.sqlite.new" \
      && chown nexusfabric:nexusfabric "$DATA/nexus-fabric.sqlite.new" \
      && mv -f "$DATA/nexus-fabric.sqlite.new" "$DATA/nexus-fabric.sqlite"
  }

  if [ "$good" = 1 ]; then
    mkdir -m 0700 "$STATE/previous.new"
    cp -p "$BIN" "$UNIT" "$STATE/previous.new/"
    if ! snapshot_db "$STATE/previous.new"; then
      systemctl restart nf-server 2>/dev/null || true
      rm -rf "$STATE/previous.new"
      die "could not copy the database for a rollback; nothing was changed"
    fi
    if [ -d "$PREV" ]; then mv "$PREV" "$STATE/previous.old"; fi
    mv "$STATE/previous.new" "$PREV"
    rm -rf "$STATE/previous.old"
    echo "the installed version and its database are kept for rollback in $PREV"
  elif [ -f "$BIN" ]; then
    echo "the installed version is not known to work (an earlier run was interrupted?); the rollback copy in $PREV is kept as it is"
  fi

  mv -f "$UNIT.new" "$UNIT"
  mv -f "$BIN.new" "$BIN"
  systemctl daemon-reload || warn "systemctl daemon-reload failed"
  systemctl enable nf-server || warn "systemctl enable nf-server failed"
  systemctl restart nf-server || warn "systemctl restart nf-server failed"

  # 'enable' succeeds even when the bind fails, so check that the server really answers (up to 60 s).
  if answers "$NF_PORT" 120; then
    record_verified
    echo "nf-server answers on 127.0.0.1:${NF_PORT}"
  else
    echo "nf-server did not answer on http://127.0.0.1:${NF_PORT}/api/health within 60 s; last log lines:" >&2
    journalctl -u nf-server -n 30 --no-pager >&2 || true
    [ -f "$PREV/nf-server" ] && [ -f "$PREV/nf-server.service" ] \
      || die "there is no earlier version to return to; fix the cause and run this script again"
    echo "install: returning to the previous version" >&2
    systemctl stop nf-server 2>/dev/null || true
    cp -p "$PREV/nf-server.service" "$UNIT.new"; mv -f "$UNIT.new" "$UNIT"
    cp -p "$PREV/nf-server" "$BIN.new"; mv -f "$BIN.new" "$BIN"
    db_note=""
    if [ -f "$PREV/nexus-fabric.sqlite" ]; then
      if restore_db; then db_note=" and its database were"; else warn "the database copy could not be restored; the previous version may refuse the database (see $STATE/failed-*)"; fi
    else
      warn "there is no database copy in $PREV (made by an older installer): the database was left as it is"
    fi
    systemctl daemon-reload || warn "systemctl daemon-reload failed"
    systemctl restart nf-server || warn "systemctl restart nf-server failed"
    prev_port="$(listen_port "$UNIT")"
    if [ -n "$prev_port" ] && answers "$prev_port" 120; then
      record_verified
      die "the new version did not answer its health check. The previous version${db_note:- was} restored and answers on 127.0.0.1:${prev_port}; Caddy and the firewall were not touched"
    fi
    journalctl -u nf-server -n 30 --no-pager >&2 || true
    die "the new version did not answer its health check, and the restored previous version does not answer either; see journalctl -u nf-server"
  fi

  # ---------------------------------------------------------------- 6. first administrator, over loopback
  # BEFORE the site is reachable from outside (V-36). Creating it needs the one-time setup token the server wrote to its
  # data directory at first start (readable by root and the service user only). Either give this script
  #   NF_ADMIN_USER=<name> NF_ADMIN_PASSWORD_FILE=<file>   (or NF_ADMIN_USER alone, from a terminal: the password is asked)
  # or create the account afterwards from this machine with the command printed at the end.
  TOKEN_FILE="$DATA/setup-token"
  if [ -f "$TOKEN_FILE" ] && [ -n "$NF_ADMIN_USER" ]; then
    # The password and the token go to curl on its standard input and in a header file, never on its command line
    # (a command line is visible to every account on the machine).
    hdr="$TPL/setup-headers"
    ( umask 077; printf 'X-NF-Setup-Token: %s\n' "$(cat "$TOKEN_FILE")" > "$hdr" )
    code="$(printf '{"username":"%s","password":"%s"}' "$NF_ADMIN_USER" "$ADMIN_PASSWORD" \
      | curl -sS -o /dev/null -w '%{http_code}' --max-time 30 -X POST "http://127.0.0.1:${NF_PORT}/api/setup" -H 'Content-Type: application/json' -H 'X-NF-Request: 1' -H "@$hdr" --data-binary @-)"
    rm -f "$hdr"
    ADMIN_PASSWORD=""
    [ "$code" = 200 ] || die "creating the first administrator failed (HTTP $code); the site has not been exposed. Fix the cause and run the script again"
    echo "first administrator '${NF_ADMIN_USER}' created"
  fi

  # ---------------------------------------------------------------- 7. Caddy: automatic HTTPS in front of the service
  if ! command -v caddy >/dev/null; then
    apt-get install -y debian-keyring debian-archive-keyring apt-transport-https gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o "$R/usr/share/keyrings/caddy-stable-archive-keyring.gpg"
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' > "$R/etc/apt/sources.list.d/caddy-stable.list"
    apt-get update && apt-get install -y caddy
  fi
  install -d -m 0755 "$R/var/log/caddy"
  if id caddy >/dev/null 2>&1; then chown caddy:caddy "$R/var/log/caddy"; fi
  mkdir -p "$R/etc/caddy"
  CADDY_CHANGED=0
  caddy_apply_site "$R/etc/caddy/Caddyfile" "$DOMAIN" "$NF_PORT" "$TPL/Caddyfile" \
    || die "Caddy configuration was not applied; the Caddyfile is unchanged"
  if [ "$CADDY_CHANGED" = 1 ]; then
    systemctl reload caddy 2>/dev/null || systemctl restart caddy || die "Caddy did not reload; see journalctl -u caddy"
  fi

  # ---------------------------------------------------------------- 8. firewall: rules only
  # Enabling ufw on a server with other services can lock them out, so it is left to the operator.
  # The service is already switched and running here, so a failed rule is a warning, not an error of the whole install.
  ufw allow OpenSSH >/dev/null || warn "ufw: could not add the SSH rule; allow it by hand: ufw allow OpenSSH"
  ufw allow 80/tcp >/dev/null || warn "ufw: could not add the rule for port 80; allow it by hand: ufw allow 80/tcp"
  ufw allow 443/tcp >/dev/null || warn "ufw: could not add the rule for port 443; allow it by hand: ufw allow 443/tcp"
  fw="$(ufw status 2>/dev/null || true)"
  case "$fw" in
    *"Status: active"*) echo "ufw is active: SSH, 80 and 443 allowed." ;;
    *) warn "ufw is inactive, so it was not enabled. Rules for SSH, 80 and 443 are stored. Before enabling, check the ports of your other services: ufw allow <port>/tcp, then ufw enable." ;;
  esac

  echo "NEXUS FABRIC answers on 127.0.0.1:${NF_PORT}; public address https://${DOMAIN} once DNS points here."
  if [ -f "$TOKEN_FILE" ]; then
    # Nothing secret goes on a command line: the password is typed at a prompt, and the token reaches curl through a
    # process substitution that root reads (curl sees a path to it, not the token). Run in bash.
    echo "NO ADMINISTRATOR EXISTS YET. Create it now, from this machine, in bash (the token is in $TOKEN_FILE, root and the service user only):"
    cat <<EOF
  read -r -s -p 'Password for the first administrator (at least 10 characters): ' P; echo
  printf '{"username":"<name>","password":"%s"}' "\$P" | curl -sS -X POST http://127.0.0.1:${NF_PORT}/api/setup -H 'Content-Type: application/json' -H 'X-NF-Request: 1' -H @<(sudo sed 's/^/X-NF-Setup-Token: /' ${TOKEN_FILE}) --data-binary @-; unset P
EOF
    echo "Replace <name>. The password must not contain a double quote or a backslash. Nobody without that token can create the administrator, also not through https://${DOMAIN}."
  fi
  echo "Back up /var/lib/nexus-fabric together with /etc/nexus-fabric/master.key (see deploy/README.md and docs/BUILD_AND_TEST_GUIDE.md)."
}

main "$@"; exit
