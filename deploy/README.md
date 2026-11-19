# Deploying the development build on a VPS (for example Hostinger VPS)

**Status: development build. Not validated. No output may be used for manufacturing. Do not upload confidential drawings.** See `docs/RELEASE_READINESS.md` and `docs/security/VULNERABILITY_REGISTER.md`.

Requirements: a VPS with root SSH access, 2 GB RAM or more for the build, and a domain whose A record points at it. Supported: Ubuntu 22.04, 24.04, 26.04 and Debian 12. Other systems get a warning. Ubuntu 26.04 was verified with a manual install of the same steps; the script itself has not yet been run end to end there. Shared web hosting cannot run this server: it is a long-running native program, not PHP.

```
git clone <repository> && cd nexus-fabric-
sudo DOMAIN=app.example.com [NF_PORT=8787] bash deploy/install.sh
```

## What the script does

- Builds the release binary and installs it to `/usr/local/bin/nf-server`.
- Creates the unprivileged system user `nexusfabric`, the data directory `/var/lib/nexus-fabric` and `/etc/nexus-fabric/master.key` (32 random bytes, readable by the service only; created once, never printed, never replaced).
- Installs `nf-server.service` as a sandboxed systemd unit listening on `127.0.0.1:$NF_PORT`. Only the listen address is changed from the shipped file; the sandboxing is untouched.
- Checks `http://127.0.0.1:$NF_PORT/api/health` for up to 15 seconds. If the service does not answer, it prints the last 30 journal lines and exits 1.
- Puts Caddy in front for HTTPS with automatic certificates.
- Adds the rules for SSH, 80 and 443 to ufw. It does **not** enable ufw. If ufw is inactive, the script warns; enable it deliberately after checking the ports of your other services.
- Can be run again. User, data and key are kept; the binary and unit are reinstalled and the service restarted.

## Options

| Variable | Default | Meaning |
| --- | --- | --- |
| `DOMAIN` | (required) | Host name that points at this server. Letters, digits, dots and hyphens only. |
| `NF_PORT` | `8787` | Local port for the service (1024 to 65535). The script refuses a port that is already listening, unless it is this service's own current port. |

## Servers that already host other sites

The script is written to coexist with an existing Caddy and other applications:

- **Port.** Pick a free port with `NF_PORT=<port>`. If the port is in use, the script stops before changing anything and lists the listeners (`ss -ltnp`).
- **Caddyfile.** `/etc/caddy/Caddyfile` is never overwritten when it has content of its own.
  - If the file is missing, or is the distribution's stock sample (a single `:80` site serving `/usr/share/caddy`), the site block is written.
  - Otherwise the file is backed up to `/etc/caddy/Caddyfile.bak-<timestamp>`. If no block for `$DOMAIN` exists, the block is appended and the other sites are kept. If a block for `$DOMAIN` already exists, the file is left untouched and the script warns; check that it proxies to `127.0.0.1:$NF_PORT`.
  - The result is checked with `caddy validate`. If it is invalid, the backup is restored and the script exits 1. Otherwise Caddy is reloaded.
- **Firewall.** Existing rules are kept; the script only adds the SSH, 80 and 443 rules.

Each run that changes the Caddyfile leaves a dated backup next to it. Remove old backups when you no longer need them.

## After installing

1. Create the first administrator as described in `docs/BUILD_AND_TEST_GUIDE.md` (it needs the header `X-NF-Request: 1`), then change that password immediately.
2. Check the public address once DNS points here: `https://$DOMAIN`.
3. Set up backups (below).

What this closes and what it does not: HTTPS in front of the server addresses the "no TLS" finding for traffic between browser and server. Still open: database not encrypted at rest as a whole, single key without rotation, no multi-factor sign-in. Keep the service for evaluation with non-confidential data.

## Update an existing install

```
cd nexus-fabric-
git pull
cargo build --release -p nf-server
sudo install -m 0755 target/release/nf-server /usr/local/bin/nf-server.new
sudo mv -f /usr/local/bin/nf-server.new /usr/local/bin/nf-server
sudo systemctl restart nf-server
curl -fsS http://127.0.0.1:8787/api/health   # use your NF_PORT
```

Re-running `sudo DOMAIN=... bash deploy/install.sh` does the same and also reapplies the unit file.

## Backups

Back up three things together: the SQLite database and blob files in `/var/lib/nexus-fabric`, and `/etc/nexus-fabric/master.key`.

- Take a consistent snapshot of the database while the service runs: `sqlite3 /var/lib/nexus-fabric/nexus-fabric.sqlite ".backup '/path/to/snapshot.sqlite'"`. Do not copy the live `.sqlite` file (with its `-wal` file) directly.
- Copy `blobs/` and the key file alongside the snapshot.
- The database and blobs are unreadable without `master.key`, so a backup without the key cannot be restored. The key is equally sensitive: the archive that contains it must be stored as carefully as the key itself, and encrypted before it leaves the server.

`deploy/nf-backup.sh` does this daily (snapshot, integrity check, archive with checksum, last 14 kept) and is optional:

```
sudo apt-get install -y sqlite3
sudo install -m 0755 deploy/nf-backup.sh /usr/local/sbin/nf-backup
sudo install -m 0644 deploy/nf-backup.service deploy/nf-backup.timer /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now nf-backup.timer
```

Archives go to `/root/backups/nexus-fabric` (set `NF_BACKUP_DIR` in the unit to change this). The script has no offsite copy; section 7 of the script is a commented hook to add one, encrypted.

To restore: stop the service, put the database and blobs back in `/var/lib/nexus-fabric` (owned by `nexusfabric`), put the key back at `/etc/nexus-fabric/master.key` (`root:nexusfabric`, mode 0440), then start the service.
