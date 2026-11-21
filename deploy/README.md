# Deploying the development build on a VPS (for example Hostinger VPS)

**Status: development build. Not validated. No output may be used for manufacturing. Do not upload confidential drawings.** See `docs/RELEASE_READINESS.md` and `docs/security/VULNERABILITY_REGISTER.md`.

Requirements: a VPS with root SSH access, 2 GB RAM or more for the build, and a domain whose A record points at it. Supported: Ubuntu 22.04, 24.04, 26.04 and Debian 12. Other systems get a warning. Ubuntu 26.04 was verified with a manual install of the same steps; the script itself has not yet been run end to end there. Shared web hosting cannot run this server: it is a long-running native program, not PHP.

The script downloads no program and never builds as root. The server binary comes from one of two places:

- **Built here by an unprivileged account that has its own Rust toolchain** (Rust 1.85 or newer). Install the toolchain for that account, not for root: as that account, get `rustup-init` for the machine from https://rustup.rs, check it against its published SHA-256 and run it. Then, from that account:

  ```
  git clone <repository> && cd nexus-fabric-
  sudo DOMAIN=app.example.com [NF_PORT=8787] bash deploy/install.sh
  ```

  The build account is `NF_BUILD_USER` if given, otherwise the account that ran `sudo`, otherwise the owner of the checkout. Root (user id 0) is refused. If you are logged in as root, name an account: `NF_BUILD_USER=<account> DOMAIN=... bash deploy/install.sh`.
- **A prebuilt binary with its SHA-256**: `sudo NF_BINARY=/path/to/nf-server NF_BINARY_SHA256=<64 hex digits> DOMAIN=app.example.com bash deploy/install.sh`. The file is copied and its checksum checked before anything is installed; a mismatch stops the script with nothing changed.

## What the script does

- Checks everything first (root, `DOMAIN`, `NF_PORT`, the checksum format, the build account and its toolchain, the port) and changes nothing when a check fails.
- Builds the release binary as the build account, in that account's own build directory (`NF_BUILD_DIR`, default `~/.cache/nexus-fabric-build`): the account receives a copy of the sources and never writes the checkout, and root only reads the finished binary back. Or it checks the prebuilt binary against `NF_BINARY_SHA256`. Then it installs the binary to `/usr/local/bin/nf-server` (root, mode 0755).
- Creates the unprivileged system user `nexusfabric`, the data directory `/var/lib/nexus-fabric` and `/etc/nexus-fabric/master.key` (32 random bytes, readable by the service only; created once, never printed, never replaced).
- Installs `nf-server.service` as a sandboxed systemd unit listening on `127.0.0.1:$NF_PORT`. Only the listen address (`NF_LISTEN`) and the public host name (`NF_ALLOWED_HOSTS=$DOMAIN`) are changed from the shipped file; the sandboxing is untouched.
- Checks `http://127.0.0.1:$NF_PORT/api/health` for up to 15 seconds. If the service does not answer, it prints the last 30 journal lines, **puts the previous binary and unit back**, restarts and checks them, and exits 1 (Caddy and the firewall are not touched). The binary and unit that last answered the health check are kept in `/usr/local/lib/nexus-fabric/previous` for this. On a first install there is nothing to return to; the script then only reports the failure.
- Puts Caddy in front for HTTPS with automatic certificates.
- Adds the rules for SSH, 80 and 443 to ufw. It does **not** enable ufw. If ufw is inactive, the script warns; enable it deliberately after checking the ports of your other services.
- Can be run again, and can be interrupted at any point (Ctrl+C, a lost SSH session): every file is written beside its target and renamed into place, the key is never replaced, and a run started afterwards finishes the job. A binary that was put in place but never answered the health check is not taken as the version to return to. User, data and key are kept; the binary and unit are reinstalled and the service restarted. Only one run at a time.

## Options

| Variable | Default | Meaning |
| --- | --- | --- |
| `DOMAIN` | (required) | Host name that points at this server. Letters, digits, dots and hyphens only. |
| `NF_PORT` | `8787` | Local port for the service (1024 to 65535). The script refuses a port that is already listening, unless it is this service's own current port. |
| `NF_BUILD_USER` | the account that ran `sudo`, else the owner of the checkout | Unprivileged account that builds the binary with its own Rust toolchain. Root is refused. |
| `NF_BUILD_DIR` | `~<build account>/.cache/nexus-fabric-build` | Build directory of that account (absolute path it can write). Compiled dependencies are kept there between runs. |
| `NF_BINARY`, `NF_BINARY_SHA256` | (none) | Install this prebuilt binary instead of building; both are needed, the checksum is 64 hexadecimal digits. |
| `NF_ADMIN_USER`, `NF_ADMIN_PASSWORD` | (none) | Create the first administrator over loopback before Caddy is configured (both or neither). |
| `NF_INSTALL_ROOT` | (none) | **Tests only** (`tests/install_check.sh`, run as `NF_SANDBOX_STRICT=1 nf-sandbox bash tests/install_check.sh`): a directory put in front of every path the script touches, with stand-in commands first on `PATH`. Never set it for a real install. |

## Servers that already host other sites

The script is written to coexist with an existing Caddy and other applications:

- **Port.** Pick a free port with `NF_PORT=<port>`. If the port is in use, the script stops before changing anything and lists the listeners (`ss -ltnp`).
- **Caddyfile.** `/etc/caddy/Caddyfile` is never overwritten when it has content of its own.
  - If the file is missing, or is the distribution's stock sample (a single `:80` site serving `/usr/share/caddy`), the site block is written.
  - Otherwise the file is backed up to `/etc/caddy/Caddyfile.bak-<timestamp>`. If no block for `$DOMAIN` exists, the block is appended and the other sites are kept. If a block for `$DOMAIN` already exists, the file is left untouched and the script warns; check that it proxies to `127.0.0.1:$NF_PORT`.
  - The new file is assembled beside the old one (`Caddyfile.nf-new`) and checked with `caddy validate` before it replaces the old one. If it is invalid, the old file is left unchanged and the script exits 1. Otherwise Caddy is reloaded.
- **Firewall.** Existing rules are kept; the script only adds the SSH, 80 and 443 rules.

Each run that changes the Caddyfile leaves a dated backup next to it. Remove old backups when you no longer need them.

## After installing

1. Create the first administrator. This needs the **one-time setup token** that the server writes to `/var/lib/nexus-fabric/setup-token` (root and the service user only) at its first start. Either pass `NF_ADMIN_USER` and `NF_ADMIN_PASSWORD` to `install.sh`, which then creates the account over loopback before Caddy is configured, or run the `curl` command the script prints at the end (also in `docs/BUILD_AND_TEST_GUIDE.md`). Without the token nobody can create the administrator, also not through the public address; the token file is deleted once the account exists.
2. Check the public address once DNS points here: `https://$DOMAIN`.
3. Set up backups (below).

What this closes and what it does not: HTTPS in front of the server addresses the "no TLS" finding for traffic between browser and server. Still open: database not encrypted at rest as a whole, single key without rotation, no multi-factor sign-in. Keep the service for evaluation with non-confidential data.

## Update an existing install

```
cd nexus-fabric-
git pull
sudo DOMAIN=app.example.com NF_PORT=<your port> bash deploy/install.sh
```

This builds as the unprivileged build account (or installs `NF_BINARY` after checking `NF_BINARY_SHA256`), keeps the running version for rollback, reapplies the unit file, restarts the service and checks it; if the new version does not answer within 15 seconds, the previous one is put back and started again. To return to the previous version by hand: `sudo cp -p /usr/local/lib/nexus-fabric/previous/nf-server /usr/local/bin/nf-server.new && sudo mv -f /usr/local/bin/nf-server.new /usr/local/bin/nf-server && sudo systemctl restart nf-server` (and the same for `nf-server.service` into `/etc/systemd/system/`, followed by `sudo systemctl daemon-reload`).

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

## Settings that matter behind the proxy (since 19 November 2026)

- `NF_ALLOWED_HOSTS` (unit file; `install.sh` sets it to `DOMAIN`): the server answers only to this host name and to loopback names; any other `Host` gets 421. **When upgrading an installation whose unit file predates this setting, add `Environment=NF_ALLOWED_HOSTS=<your domain>` before restarting, or every request through Caddy is refused.** Several names: comma-separated.
- `NF_TRUSTED_PROXY=127.0.0.1` (unit file): client addresses are taken from Caddy's `X-Forwarded-For`, so sign-in throttling is per client. Without it every visitor shares Caddy's address and one flood can throttle sign-in for all.
- `NF_COOKIE_SECURE=1` (unit file) also tells the server that browsers reach it over HTTPS (since 21 November 2026, V-47): a change requested from a page whose `Origin` is not exactly `https://<the Host the request names>` is refused with 403 — also an `http://` page, for example the service opened through an SSH tunnel at `http://127.0.0.1:<port>`. Use the HTTPS address in the browser. Scripts such as `curl`, which send no `Origin`, are not affected (they send `X-NF-Request: 1`). Without `NF_COOKIE_SECURE` the expected scheme is `http`, as in desktop mode.
- The service refuses to start, and says why in `journalctl -u nf-server`, when the key file is missing, world-readable or not the key the data was created with, or when the database is missing, empty or damaged while the data directory holds an installation. It never creates a new key or falls back to first-run setup beside existing data: restore from the backup.
- On `systemctl stop` / `restart` the server stops accepting connections, lets requests in flight finish (20 s by default) and folds its write-ahead log into the database file.
- New unit directives (`UMask`, `ProtectProc`, `ProtectKernelLogs`, `RestrictSUIDSGID`, `SystemCallArchitectures`, `IPAddressDeny=any` with `IPAddressAllow=localhost`) have not been run on a server yet; if the service fails to start after an upgrade, `journalctl -u nf-server` names the directive.
