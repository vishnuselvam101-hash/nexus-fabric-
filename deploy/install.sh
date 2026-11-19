#!/usr/bin/env bash
# Run as root from the repository root:
#   sudo DOMAIN=app.example.com [NF_PORT=8787] bash deploy/install.sh
# Safe on a server that already runs other sites: it never overwrites an existing Caddyfile
# (it backs it up and appends one site block), refuses a port that is in use, and fails if the service does not come up.
set -euo pipefail
: "${DOMAIN:?set DOMAIN=your.domain}"
NF_PORT="${NF_PORT:-8787}"
[ "$(id -u)" = 0 ] || { echo "run as root" >&2; exit 1; }
. /etc/os-release 2>/dev/null || true
case "${ID:-}:${VERSION_ID:-}" in
  ubuntu:22.04|ubuntu:24.04|ubuntu:26.04|debian:12) ;;   # 26.04: installed manually once, script path itself untested there
  *) [ "${NF_FORCE_OS:-0}" = 1 ] || { echo "unsupported OS '${PRETTY_NAME:-unknown}'. Supported: Ubuntu 22.04/24.04/26.04, Debian 12. Set NF_FORCE_OS=1 to try anyway." >&2; exit 1; } ;;
esac
case "$NF_PORT" in ''|*[!0-9]*) echo "NF_PORT must be a number" >&2; exit 1;; esac
upgrade=0; systemctl is-active --quiet nf-server 2>/dev/null && upgrade=1
if [ "$upgrade" = 0 ] && ss -ltnH "sport = :$NF_PORT" | grep -q .; then
  echo "port $NF_PORT is already in use; choose another with NF_PORT=<port>" >&2; exit 1
fi

apt-get update && apt-get install -y build-essential curl ufw iproute2
command -v cargo >/dev/null || { curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal; . "$HOME/.cargo/env"; }
cargo build --release -p nf-server
install -m 0755 target/release/nf-server /usr/local/bin/nf-server
id nexusfabric >/dev/null 2>&1 || useradd --system --home /var/lib/nexus-fabric --shell /usr/sbin/nologin nexusfabric
install -d -o nexusfabric -g nexusfabric -m 0700 /var/lib/nexus-fabric
install -d -o root -g nexusfabric -m 0750 /etc/nexus-fabric
[ -f /etc/nexus-fabric/master.key ] || { head -c 32 /dev/urandom > /etc/nexus-fabric/master.key; chown root:nexusfabric /etc/nexus-fabric/master.key; chmod 0440 /etc/nexus-fabric/master.key; }
sed "s/^Environment=NF_LISTEN=.*/Environment=NF_LISTEN=127.0.0.1:${NF_PORT}/" deploy/nf-server.service > /etc/systemd/system/nf-server.service
systemctl daemon-reload && systemctl enable nf-server && systemctl restart nf-server
# 'systemctl enable --now' succeeds even when the bind fails, so check that the server really answers
ok=0; for _ in $(seq 1 20); do
  if systemctl is-active --quiet nf-server && curl -fsS -o /dev/null "http://127.0.0.1:${NF_PORT}/"; then ok=1; break; fi; sleep 0.5
done
[ "$ok" = 1 ] || { echo "nf-server did not come up on 127.0.0.1:${NF_PORT}:" >&2; journalctl -u nf-server -n 30 --no-pager >&2; exit 1; }

# Caddy: automatic HTTPS. An existing configuration is kept: backed up, then one site block appended.
if ! command -v caddy >/dev/null; then
  apt-get install -y debian-keyring debian-archive-keyring apt-transport-https gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' > /etc/apt/sources.list.d/caddy-stable.list
  apt-get update && apt-get install -y caddy
fi
CF=/etc/caddy/Caddyfile
block="$(sed -e "s/app.example.com/${DOMAIN}/" -e "s/127.0.0.1:8787/127.0.0.1:${NF_PORT}/" -e '/^#/d' deploy/Caddyfile)"
if [ -f "$CF" ] && grep -qF "$DOMAIN" "$CF"; then
  echo "Caddyfile already mentions ${DOMAIN}; left unchanged. Check that it proxies to 127.0.0.1:${NF_PORT}."
else
  [ -f "$CF" ] && cp -p "$CF" "$CF.bak.$(date +%Y%m%d%H%M%S)"
  printf '\n%s\n' "$block" >> "$CF"
  if ! caddy validate --config "$CF" --adapter caddyfile >/dev/null 2>&1; then
    echo "new Caddyfile does not validate; restoring the previous one" >&2
    latest="$(ls -t "$CF".bak.* 2>/dev/null | head -1)"; [ -n "$latest" ] && cp -p "$latest" "$CF"; exit 1
  fi
  systemctl reload caddy || systemctl restart caddy
fi
# firewall: add rules only; enable only if it was not already managed
ufw allow OpenSSH >/dev/null; ufw allow 80/tcp >/dev/null; ufw allow 443/tcp >/dev/null
ufw status | grep -q "Status: active" || ufw --force enable
echo "NEXUS FABRIC answers on 127.0.0.1:${NF_PORT}; public address https://${DOMAIN} once DNS points here."
echo "Create the first administrator: see docs/BUILD_AND_TEST_GUIDE.md. Back up /var/lib/nexus-fabric together with /etc/nexus-fabric/master.key."
