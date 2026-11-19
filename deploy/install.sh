#!/usr/bin/env bash
# Run as root on a fresh Ubuntu 22.04/24.04 or Debian 12 VPS, from the repository root:
#   sudo DOMAIN=app.example.com bash deploy/install.sh
set -euo pipefail
: "${DOMAIN:?set DOMAIN=your.domain}"
apt-get update && apt-get install -y build-essential curl ufw debian-keyring debian-archive-keyring apt-transport-https
command -v cargo >/dev/null || { curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal; . "$HOME/.cargo/env"; }
cargo build --release -p nf-server
install -m 0755 target/release/nf-server /usr/local/bin/nf-server
id nexusfabric >/dev/null 2>&1 || useradd --system --home /var/lib/nexus-fabric --shell /usr/sbin/nologin nexusfabric
install -d -o nexusfabric -g nexusfabric -m 0700 /var/lib/nexus-fabric
install -d -o root -g nexusfabric -m 0750 /etc/nexus-fabric
[ -f /etc/nexus-fabric/master.key ] || { head -c 32 /dev/urandom > /etc/nexus-fabric/master.key; chown root:nexusfabric /etc/nexus-fabric/master.key; chmod 0440 /etc/nexus-fabric/master.key; }
install -m 0644 deploy/nf-server.service /etc/systemd/system/nf-server.service
systemctl daemon-reload && systemctl enable --now nf-server
# Caddy: automatic HTTPS certificates
if ! command -v caddy >/dev/null; then
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' > /etc/apt/sources.list.d/caddy-stable.list
  apt-get update && apt-get install -y caddy
fi
sed "s/app.example.com/${DOMAIN}/" deploy/Caddyfile > /etc/caddy/Caddyfile
systemctl reload caddy || systemctl restart caddy
ufw allow OpenSSH && ufw allow 80/tcp && ufw allow 443/tcp && ufw --force enable
systemctl --no-pager status nf-server | head -5
echo "Open https://${DOMAIN} . Back up /var/lib/nexus-fabric and /etc/nexus-fabric/master.key together; without the key the data cannot be read."
