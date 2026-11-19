# Deploying the development build on a VPS (for example Hostinger VPS)

**Status: development build. Not validated. No output may be used for manufacturing. Do not upload confidential drawings.** See `docs/RELEASE_READINESS.md` and `docs/security/VULNERABILITY_REGISTER.md`.

Requirements: a VPS with root SSH access (Ubuntu 22.04/24.04 or Debian 12, 2 GB RAM or more for the build) and a domain whose A record points at it. Shared web hosting cannot run this server: it is a long-running native program, not PHP.

```
git clone <repository> && cd nexus-fabric-
sudo DOMAIN=app.example.com bash deploy/install.sh
```

What the script does: builds the release binary; runs it as an unprivileged, sandboxed systemd service bound to 127.0.0.1:8787; creates a random key file readable only by the service; puts Caddy in front for HTTPS with automatic certificates, HTTP/2 or HTTP/3 and compression; opens only SSH, 80 and 443 in the firewall.

After installing: check `docs/BUILD_AND_TEST_GUIDE.md` for how the first administrator account is created, and change that password immediately.

What this closes and what it does not: HTTPS in front of the server addresses the "no TLS" finding for traffic between browser and server. Still open: database not encrypted at rest as a whole, single key without rotation, no multi-factor sign-in. Keep the service for evaluation with non-confidential data.

Updating: `git pull && cargo build --release -p nf-server && sudo install -m 0755 target/release/nf-server /usr/local/bin/nf-server && sudo systemctl restart nf-server`.
Backup: stop the service, copy `/var/lib/nexus-fabric` and `/etc/nexus-fabric/master.key`, start the service.

The values of `NF_LISTEN`, `NF_DATA_DIR`, `NF_KEY_FILE`, `NF_COOKIE_SECURE` in `nf-server.service` are the server's documented settings; this deployment has not yet been run on a real VPS.
