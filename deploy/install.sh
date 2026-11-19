#!/usr/bin/env bash
# Run as root:
#   sudo DOMAIN=app.example.com [NF_PORT=8787] bash deploy/install.sh
# Re-runnable: an existing install is upgraded in place (binary, unit, restart); user, data and key are kept.
# Safe next to other sites: an existing Caddyfile is never overwritten (see deploy/caddy-site.sh), a port in
# use is refused, the firewall is never enabled here, and the script fails unless the service answers.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."
# shellcheck source=caddy-site.sh
. "$SCRIPT_DIR/caddy-site.sh"

die() { echo "install: $*" >&2; exit 1; }
warn() { echo "install: WARNING: $*" >&2; }

[ "$(id -u)" = 0 ] || die "run as root"
: "${DOMAIN:?set DOMAIN=your.domain}"
case "$DOMAIN" in ''|*[!A-Za-z0-9.-]*|.*|*.) die "DOMAIN must be a host name such as app.example.com" ;; esac
NF_PORT="${NF_PORT:-8787}"
case "$NF_PORT" in ''|*[!0-9]*) die "NF_PORT must be a number" ;; esac
[ "$NF_PORT" -ge 1024 ] && [ "$NF_PORT" -le 65535 ] || die "NF_PORT must be between 1024 and 65535"

. /etc/os-release 2>/dev/null || true
case "${ID:-}:${VERSION_ID:-}" in
  ubuntu:22.04|ubuntu:24.04|debian:12) ;;
  ubuntu:26.04) echo "note: Ubuntu 26.04 was verified by a manual install; this script has not been run end to end there." ;;
  *) warn "untested OS '${PRETTY_NAME:-unknown}'. Verified for: Ubuntu 22.04/24.04/26.04, Debian 12. Continuing." ;;
esac

# Port: refuse one that is already in use, unless it is this service's own current port.
existing_port="$(sed -n 's/^Environment=NF_LISTEN=.*:\([0-9][0-9]*\)$/\1/p' /etc/systemd/system/nf-server.service 2>/dev/null | head -1 || true)"
own=0
if systemctl is-active --quiet nf-server 2>/dev/null && [ "$existing_port" = "$NF_PORT" ]; then own=1; fi
if [ "$own" = 0 ] && ss -ltnH "sport = :$NF_PORT" | grep -q .; then
  ss -ltnp "sport = :$NF_PORT" >&2 || true
  die "port $NF_PORT is already in use (listeners above); choose a free port with NF_PORT=<port>"
fi

apt-get update && apt-get install -y build-essential curl ufw iproute2
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
if ! command -v cargo >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
cargo build --release -p nf-server
install -m 0755 target/release/nf-server /usr/local/bin/nf-server.new
mv -f /usr/local/bin/nf-server.new /usr/local/bin/nf-server

id nexusfabric >/dev/null 2>&1 || useradd --system --home /var/lib/nexus-fabric --shell /usr/sbin/nologin nexusfabric
install -d -o nexusfabric -g nexusfabric -m 0700 /var/lib/nexus-fabric
install -d -o root -g nexusfabric -m 0750 /etc/nexus-fabric
if [ ! -f /etc/nexus-fabric/master.key ]; then
  ( umask 077; head -c 32 /dev/urandom > /etc/nexus-fabric/master.key )
  chown root:nexusfabric /etc/nexus-fabric/master.key
  chmod 0440 /etc/nexus-fabric/master.key
  echo "created /etc/nexus-fabric/master.key (back it up; it is not shown here)"
fi

# Unit: the shipped file with the listen address and the public host name filled in; sandboxing is untouched.
# NF_ALLOWED_HOSTS: the server answers only to this name (and loopback); DOMAIN was validated above.
sed -e "s|^Environment=NF_LISTEN=.*|Environment=NF_LISTEN=127.0.0.1:${NF_PORT}|" -e "s|^Environment=NF_ALLOWED_HOSTS=.*|Environment=NF_ALLOWED_HOSTS=${DOMAIN}|" deploy/nf-server.service > /etc/systemd/system/nf-server.service.new
mv -f /etc/systemd/system/nf-server.service.new /etc/systemd/system/nf-server.service
systemctl daemon-reload
systemctl enable nf-server
systemctl restart nf-server

# 'enable' succeeds even when the bind fails, so check that the server really answers.
ok=0
for _ in $(seq 1 30); do
  if curl -fsS -o /dev/null --max-time 2 "http://127.0.0.1:${NF_PORT}/api/health"; then ok=1; break; fi
  sleep 0.5
done
if [ "$ok" != 1 ]; then
  echo "nf-server did not answer on http://127.0.0.1:${NF_PORT}/api/health; last log lines:" >&2
  journalctl -u nf-server -n 30 --no-pager >&2 || true
  exit 1
fi
echo "nf-server answers on 127.0.0.1:${NF_PORT}"

# First administrator BEFORE the site is reachable from outside (V-36). Creating it needs the one-time setup token
# the server wrote to its data directory at first start (readable by root and the service user only). Either pass
#   NF_ADMIN_USER=<name> NF_ADMIN_PASSWORD='<at least 10 characters>'
# to this script, or create the account afterwards from this machine with the command printed at the end.
TOKEN_FILE=/var/lib/nexus-fabric/setup-token
if [ -f "$TOKEN_FILE" ] && [ -n "${NF_ADMIN_USER:-}" ] && [ -n "${NF_ADMIN_PASSWORD:-}" ]; then
  case "$NF_ADMIN_USER" in *[!A-Za-z0-9._-]*) die "NF_ADMIN_USER may contain letters, digits, . _ - only" ;; esac
  case "$NF_ADMIN_PASSWORD" in *[\"\\]*) die "NF_ADMIN_PASSWORD must not contain a double quote or a backslash when given to this script; create the account by hand instead" ;; esac
  code="$(curl -sS -o /dev/null -w '%{http_code}' --max-time 30 -X POST "http://127.0.0.1:${NF_PORT}/api/setup" -H 'Content-Type: application/json' -H 'X-NF-Request: 1' -H "X-NF-Setup-Token: $(cat "$TOKEN_FILE")" -d "{\"username\":\"${NF_ADMIN_USER}\",\"password\":\"${NF_ADMIN_PASSWORD}\"}")"
  [ "$code" = 200 ] || die "creating the first administrator failed (HTTP $code); the site has not been exposed. Fix the cause and run the script again"
  echo "first administrator '${NF_ADMIN_USER}' created"
fi

# Caddy: automatic HTTPS in front of the service.
if ! command -v caddy >/dev/null; then
  apt-get install -y debian-keyring debian-archive-keyring apt-transport-https gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' > /etc/apt/sources.list.d/caddy-stable.list
  apt-get update && apt-get install -y caddy
fi
install -d -m 0755 /var/log/caddy
if id caddy >/dev/null 2>&1; then chown caddy:caddy /var/log/caddy; fi
CADDY_CHANGED=0
caddy_apply_site /etc/caddy/Caddyfile "$DOMAIN" "$NF_PORT" "$SCRIPT_DIR/Caddyfile" \
  || die "Caddy configuration was not applied; the previous Caddyfile is restored"
if [ "$CADDY_CHANGED" = 1 ]; then
  systemctl reload caddy 2>/dev/null || systemctl restart caddy || die "Caddy did not reload; see journalctl -u caddy"
fi

# Firewall: add the rules only. Enabling ufw on a server with other services can lock them out, so it is left to the operator.
ufw allow OpenSSH >/dev/null
ufw allow 80/tcp >/dev/null
ufw allow 443/tcp >/dev/null
fw="$(ufw status 2>/dev/null || true)"
case "$fw" in
  *"Status: active"*) echo "ufw is active: SSH, 80 and 443 allowed." ;;
  *) warn "ufw is inactive, so it was not enabled. Rules for SSH, 80 and 443 are stored. Before enabling, check the ports of your other services: ufw allow <port>/tcp, then ufw enable." ;;
esac

echo "NEXUS FABRIC answers on 127.0.0.1:${NF_PORT}; public address https://${DOMAIN} once DNS points here."
if [ -f "$TOKEN_FILE" ]; then
  echo "NO ADMINISTRATOR EXISTS YET. Create it now, from this machine. It needs the one-time setup token in $TOKEN_FILE:"
  echo "  curl -sS -X POST http://127.0.0.1:${NF_PORT}/api/setup -H 'Content-Type: application/json' -H 'X-NF-Request: 1' -H \"X-NF-Setup-Token: \$(sudo cat $TOKEN_FILE)\" -d '{\"username\":\"<name>\",\"password\":\"<at least 10 characters>\"}'"
  echo "Nobody without that token can create the administrator, also not through https://${DOMAIN}."
fi
echo "Back up /var/lib/nexus-fabric together with /etc/nexus-fabric/master.key (see deploy/README.md and docs/BUILD_AND_TEST_GUIDE.md)."
