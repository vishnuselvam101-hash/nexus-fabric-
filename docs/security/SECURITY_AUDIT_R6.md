# Security Audit — Round 6

19 November 2026 · Scope: whole repository at commit `8f352ef` plus the uncommitted working tree, with special attention to today's rewritten client (`web/app.js`, `web/viewer.js`, `web/styles.css`, `web/index.html`, new static route `/viewer.js`) and the new deployment recipe (`deploy/`). Method: manual code review plus black-box and grey-box testing of a **local, disposable** instance (a copy of `target/release/nf-server` built 19 Nov 02:31, on `127.0.0.1:8820`, temporary data directories). The live VPS instance and every external host were **not** touched or probed.

**This is a self-assessment at evidence level EV-1 that has not been independently reviewed, not an independent penetration test. No certification or compliance is claimed. NEXUS FABRIC is not "hack-proof"; finding no critical issue in one review is not evidence that none exist.**

Reproducible checks: `tests/security_r6.sh` (40 checks, each labelled with its ID). On the audited build it reports **2 passed, 38 failed** — the failures are the open findings below, by design. Run: `NF_BIN=<binary> bash tests/security_r6.sh` (refuses to run if anything already answers on the port).

## Summary

| Severity | New findings (V-33 … V-49) |
|---|---|
| Critical | 0 |
| High | 4 — V-33, V-34, V-35, V-36 |
| Medium | 2 — V-37, V-38 |
| Low | 8 — V-39 … V-46 |
| Informational | 3 — V-47, V-48, V-49 |

Most important conclusion: three of the four High findings are **unauthenticated denial-of-service or takeover paths that only became reachable from the internet with today's HTTPS proxy deployment**. The pre-existing open Highs (V-20 … V-23) are reassessed at the end.

**Action for the owner now (no code change needed):** on the live instance, confirm that audit entry #1 (`setup`) names your own administrator account. If it does not, assume compromise (V-36).

---

## High

### V-33 — Any anonymous client can lock every user out of sign-in
- **Severity:** High. AV:N/AC:L/PR:N/UI:N/A:H (≈7.5). One request every 1.5 s, no credentials, blocks all sign-ins including administrators.
- **Where:** `crates/nf-server/src/main.rs:223-231` (global rule `overall >= 40` per minute, checked before the password); no client address is used anywhere. Behind Caddy every request comes from `127.0.0.1`; the server neither trusts nor reads `X-Forwarded-For` (good: it cannot be spoofed — but per-client throttling is impossible).
- **Reproduction:**
  ```
  for i in $(seq 1 40); do curl -s -o /dev/null -H 'X-NF-Request: 1' -H 'Content-Type: application/json' \
     -d "{\"username\":\"rnd$i\",\"password\":\"x\"}" http://127.0.0.1:8820/api/login; done
  curl -H 'X-NF-Request: 1' -H 'Content-Type: application/json' -d '{"username":"<admin>","password":"<correct>"}' http://127.0.0.1:8820/api/login
  → 429 {"error":"too many failed sign-in attempts; try again in 15 minutes"}   (also with any X-Forwarded-For)
  ```
- **Impact:** service unusable for everyone who is not already signed in, for as long as the attacker continues (cost: 40 requests/min). Combined with V-24 per-account lockout.
- **Fix:** remove the global hard block. Throttle per client address (taken from `X-Forwarded-For` **only** when the TCP peer is in a configured `NF_TRUSTED_PROXIES` list, rightmost untrusted hop) plus per-account exponential back-off; consider a global *slowdown* (delay) instead of a refusal; record the client address in the audit log.
- **Regression check:** `V-33 administrator can sign in after 40 failures by other names`.

### V-34 — Anonymous audit-log and disk flooding
- **Severity:** High. AV:N/AC:L/PR:N/UI:N/A:H (≈7.5). Disk exhaustion stops the database for all users; it also degrades the audit trail.
- **Where:** `main.rs:229` (every throttled attempt is audited), `:242`, `:81-86` (`audit()` stores the caller-supplied user name unbounded), `:1003-1009` (`/api/audit` returns 300 newest rows unpaged). Login user name is limited only by the 256 KB body limit.
- **Reproduction (measured):** while throttled, 100 requests `POST /api/login {"username":"A"*250000,"password":"x"}` took 0.81 s and grew the data directory from 2.8 MB to 26.8 MB (≈30 MB/s from one client). Afterwards `GET /api/audit` as administrator returned **25 MB** (the client then renders it into the DOM).
- **Impact:** disk full → every write fails (and `audit()` ignores insert errors, see V-41); administrator console unusable; attacker-chosen strings fill the audit trail.
- **Fix:** reject user names longer than 64 characters (same rule as `create_user`) before any database work; do not write one audit row per throttled attempt (aggregate: one row per account/client per window); paginate `/api/audit` and cap `detail` length; monitor free disk space.
- **Regression checks:** `V-34 … database grows < 1 MB`, `… user names bounded (<= 64)`, `… audit API response < 1 MB`.

### V-35 — Anonymous memory exhaustion through the upload route
- **Severity:** High. AV:N/AC:L/PR:N/UI:N/A:H (≈7.5); needs upload bandwidth (≈1.6 GB per outage), which is cheap.
- **Where:** `main.rs:457` (`body: Bytes` extractor) and `:1069` (`DefaultBodyLimit::max(50 MB + 1 KB)`). Axum extractors run before the handler, so the full 50 MB body is buffered **before** `auth()` is called. The anti-forgery middleware only requires a header any non-browser client can send.
- **Reproduction:**
  ```
  curl -H 'X-NF-Request: 1' -H 'Expect: 100-continue' --data-binary @52MB.pdf http://127.0.0.1:8820/api/projects/1/documents \
       -w '%{http_code} %{size_upload}'      → 401 52000009   (body fully read, then refused)
  32 such requests in parallel: server RSS 330 MB → 944 MB.
  ```
  `deploy/nf-server.service` sets `MemoryMax=1G`; the kernel then kills the service (systemd restarts it; a sustained attack keeps it down). Signed-in engineers can do the same with accepted uploads (8 parallel: +390 MB).
- **Fix:** authenticate and authorise (session, role, membership) **before** reading the body: read the request as `axum::extract::Request`, check, then stream with a limit; add a concurrency limit (e.g. `tower::limit::ConcurrencyLimitLayer`, 2–4) on the upload route; optionally `request_body { max_size 50MB }` in Caddy; stream to a temporary file instead of memory.
- **Regression check:** `V-35 unauthenticated upload refused before the body is read`.

### V-36 — First administrator can be claimed by anyone on a fresh deployment; no Host validation
- **Severity:** High (Critical for an instance that is left un-initialised). AV:N/AC:H/PR:N/UI:N/C:H/I:H/A:H (≈8.1). AC:H because a time window is needed; new TLS certificates appear in Certificate Transparency logs within minutes and are scanned for exactly this.
- **Where:** `main.rs:205-218` (`/api/setup` is anonymous; first caller becomes administrator); `main.rs:198-203` (`/api/health` tells anyone `setup_required: true`); `deploy/install.sh:44-56,61` publishes the site through Caddy and only then tells the operator to create the administrator by hand (`docs/BUILD_AND_TEST_GUIDE.md`, curl to 127.0.0.1). No `Host` allow-list: `security_layer` (`main.rs:1018-1031`) accepts any Host, so a **local** development instance on 127.0.0.1 is reachable by DNS rebinding from any web page the developer visits (Origin and Host are then both the attacker's name and match).
- **Reproduction:**
  ```
  # fresh instance; foreign Host, matching Origin (what a DNS-rebinding page produces)
  curl -H 'X-NF-Request: 1' -H 'Content-Type: application/json' -H 'Host: rebind.attacker.example:8820' \
       -H 'Origin: http://rebind.attacker.example:8820' -d '{"username":"rebinder","password":"rebinder-password-1"}' \
       http://127.0.0.1:8820/api/setup                       → 200 {"created":true}
  ```
  Setup is not racy (20 parallel calls → exactly one 200; the handler holds the database lock) — the issue is *who* may call it.
- **Impact:** complete takeover: user management, all projects, audit log.
- **Fix:** require a one-time setup secret generated at first start and written to a root-only file / the journal (or a `nf-server create-admin` command run on the host); have `install.sh` create the administrator **before** the Caddy site is enabled; add `NF_PUBLIC_ORIGIN` and refuse requests whose Host does not match it (421) and Origins that are not exactly it; stop reporting `setup_required` once a public origin is configured. Note: a "loopback only" rule would **not** help — behind Caddy every request is from 127.0.0.1.
- **Regression checks:** `V-36 setup through a foreign Host … is refused`, `V-36 anonymous setup without a one-time setup secret is refused`.

## Medium

### V-37 — Argon2 runs under the single database lock; password change is unthrottled
- **Severity:** Medium. AV:N/AC:L/PR:L/UI:N/A:H, partial (≈6.5): any account, including a viewer.
- **Where:** `main.rs:75-77` (one `Mutex<Connection>` for everything), `:290-305` (`change_password` hashes while holding it; no failure counter), also `:239` (login) and `:186` (user creation). The lock is a blocking `std::sync::Mutex` inside async handlers, so waiting requests also tie up runtime worker threads.
- **Reproduction:** as viewer, 12 wrong `POST /api/password {"current":"guess-N",...}` → twelve 403, never 429. With 60 such requests queued, `GET /api/health` took **1.51 s** (idle: 0.017 s).
- **Impact:** one low-privilege user (or a stolen session) stalls the whole service, and can guess the account's current password online without limit.
- **Fix:** read salt/hash, drop the guard, hash in `tokio::task::spawn_blocking`, re-acquire; count failed password changes like failed sign-ins and throttle; longer term a connection pool (SQLite WAL readers) instead of one mutex.
- **Regression checks:** `V-37 repeated wrong current passwords are throttled (429)`, `V-37 health answers in < 0.5 s while 40 password checks run`.

### V-38 — Test scripts can run against the live instance
- **Severity:** Medium. Operational: integrity/availability of production when a test is run on the VPS.
- **Where:** `tests/security_smoke.sh:6,9` (default port **8792**), `tests/e2e_smoke.sh:6` (8791); `deploy/README.md:23` records the live instance on **127.0.0.1:8792**. `start()` only waits for *something* to answer on the port and never checks that its own process is alive.
- **Reproduction (observed during this audit):** a second server on an occupied port panicked with `AddrInUse`, yet the health poll succeeded because the already-running instance answered. Run on the VPS, `security_smoke.sh` would then send its whole sequence to production: setup attempt, sign-in as `admin/correct-horse-9` (the documented example password), user creation, uploads, and six failed sign-ins that lock out an account called `admin` for 15 minutes.
- **Fix:** move the live instance off 8792 (or tests off it); in both scripts refuse to start when the port answers and check `kill -0 $PID` while waiting (as `tests/security_r6.sh` does); never run test scripts on a production host.
- **Regression checks:** three static checks labelled V-38.

## Low

### V-39 — Session cookie hardening
- **Where:** `main.rs:250` (name `nf_session`, no `__Host-` prefix), `:259` (sign-out cookie never carries `Secure`), `:248` (12 h absolute lifetime, no idle timeout), no way for a user or administrator to list or revoke individual sessions (only password change / deactivation).
- **Tested:** with `NF_COOKIE_SECURE=1`: `Set-Cookie: nf_session=…; HttpOnly; SameSite=Strict; Path=/; Max-Age=43200; Secure`; without it, no `Secure` (expected, documented). Token is 256-bit random, stored as SHA-256; a forged cookie is refused; deactivation and password change revoke sessions.
- **Why it matters:** the deploy README invites installing on a VPS that "already hosts other sites"; a sibling sub-domain can set a `Domain=` cookie named `nf_session` (cookie tossing / login CSRF into an attacker account). `session_token()` (`main.rs:121-124`) takes the first match.
- **Fix:** `__Host-nf_session` when Secure; `Secure` on the clearing cookie; idle timeout (e.g. 30 min) with sliding expiry; session list/revoke.
- **Regression checks:** `V-39 …__Host- prefix`, `V-39 sign-out cookie carries Secure`.

### V-40 — A replay that cannot start is recorded as tampering
- **Where:** `main.rs:761-772` (synchronous `wait_with_output` inside an async handler), `:798-816` (`Err(process error)` → `matches=false` → `UPDATE … state='REJECTED'` + `integrity_failure` lesson).
- **Reproduction:** lower the server's open-file limit (`prlimit --pid <pid> --nofile=<maxfd+3>`), `POST /api/calculations/11/replay` → `"process_error":"Too many open files (os error 24)"`; the job is now `REJECTED` in the job list, its DXF answers 409, and lesson `integrity_failure` raises advisories on every later identical request. Same effect from `TasksMax=256` or memory pressure (e.g. V-35).
- **Fix:** a process error is *inconclusive*: report it, change nothing; only a hash/identity mismatch rejects. Use `tokio::process`, limit concurrent replays.
- **Regression checks:** two V-40 checks (SKIP if `prlimit` is unavailable).

### V-41 — Audit log misses security events and context
- **Where:** `main.rs:254-260` (sign-out not audited); authorisation refusals (404/403 on others' objects) and anti-forgery refusals (`:1021-1031`) not audited; no client address in any entry; `:85` insert errors ignored (`let _ =`) so actions continue unaudited when the disk is full; anonymous callers choose the `username` column of `login_failed`/`login_throttled` entries (e.g. a real administrator's name), and `setup` audits the untrimmed name (`:213`).
- **Fix:** audit sign-out, refusals (rate-limited), anti-forgery refusals; add client address (see V-33 trusted proxy); fail the request if the audit insert fails; label anonymous entries (`username = "(anonymous)"`, attempted name in `detail`, bounded).
- **Regression checks:** `V-41 sign-out is audited`, `V-41 refused access to another project is audited`.

### V-42 — Storage key handling
- **Where:** `crates/nf-server/src/crypto.rs:28-45`, `main.rs:1099-1101`; `deploy/README.md:19`, `deploy/install.sh:61`.
- **Reproduction:** start with existing data and a mistyped `NF_KEY_FILE` → the server silently creates a new key and starts; `GET /api/calculations/3/dxf/header` → `500 {"error":"decryption failed (wrong key or corrupted blob)"}`. A key file with mode 644 is accepted without warning. A new key is written with `fs::write` (umask mode) and only then `chmod 600`. Backup instructions copy the key together with the data, which defeats the separation that V-09 introduced.
- **Mitigated in the deploy recipe:** key in `/etc/nexus-fabric` (dir 0750 root:nexusfabric, file 0440), `ProtectSystem=strict` makes `/etc` read-only so the service cannot create a replacement key.
- **Fix:** never create a key when the database or blobs already exist (explicit `--init-key`); create with `OpenOptions::mode(0o600).create_new(true)`; refuse keys readable by "other"; back up the key to a different medium/location than the data.
- **Regression checks:** two V-42 checks.

### V-43 — Data directory and database created with the process umask
- **Where:** `main.rs:1097` (`create_dir_all`), `:1102`; `deploy/nf-server.service` has no `UMask=`.
- **Observed:** data dir 755, `nexus-fabric.sqlite` and blobs 644 (umask 022). The database is plaintext (V-21): names, descriptions, file names, results, audit log, Argon2 hashes. The deploy recipe's 0700 data directory mitigates it on the VPS; local and manual installs are exposed to other local users.
- **Fix:** set 0700/0600 explicitly at start; `UMask=0077` in the unit.
- **Regression checks:** two V-43 checks.

### V-44 — Deployment recipe hardening
- `deploy/install.sh:22` downloads rustup and pipes it to `sh` as root (TLS-protected, but unpinned); `:23` builds as **root**, so the build scripts of all 104 crates run as root (supply chain, V-32).
- `:40-41` Caddy repository key and source list fetched without a fingerprint check.
- `:45` `DOMAIN` is not validated before `sed` into the Caddyfile (a `/`, newline or `{` breaks or injects configuration; `.` in the pattern is a wildcard); `:46` `grep -qF "$DOMAIN"` substring match silently skips configuration when e.g. `app.example.com.au` or a comment is present. Operator-supplied, hence Low.
- `:58-59` `ufw --force enable` with only the `OpenSSH` profile locks out SSH served on another port.
- `deploy/nf-server.service`: good baseline (`NoNewPrivileges`, `ProtectSystem=strict`, empty capability set, `MemoryMax`, `SystemCallFilter=@system-service`). Missing: `IPAddressDeny=any` + `IPAddressAllow=localhost` (the server needs no outbound network), `UMask=0077`, `ProtectProc=invisible`, `ProcSubset=pid`, `RestrictSUIDSGID=true`, `SystemCallArchitectures=native`, `ProtectKernelLogs=true`, `ProtectClock=true`, `ProtectHostname=true`, `RestrictRealtime=true`, `PrivateUsers=true`.
- `deploy/Caddyfile`: HSTS without `includeSubDomains` (acceptable); relies on the application for all other headers (they pass through Caddy unchanged); no `request_body` limit (V-35). The proxy's access log is the only place client addresses are recorded; Caddy redacts `Cookie`/`Authorization` in logs by default.
- **Fix:** as listed. **Regression checks:** nine static V-44 checks.

### V-45 — CSP allows inline styles; no Trusted Types
- **Where:** `main.rs:1038`: `style-src 'self' 'unsafe-inline'` (14 inline `style="…"` attributes in the new `web/app.js`), `img-src 'self' data:`. The client builds almost every view with `innerHTML`.
- **Assessment:** escaping is consistent today (see "sound" below), `script-src 'self'` holds and `default-src 'none'` blocks exfiltration targets, so the residual risk is CSS injection and future regressions. The old smoke check matches only the CSP prefix and did not notice the change.
- **Fix:** move inline styles to classes (CSSOM `style.setProperty` does not need `'unsafe-inline'`), drop `'unsafe-inline'`, add `require-trusted-types-for 'script'` with a single escaping policy.
- **Regression check:** `V-45 CSP has no 'unsafe-inline'`.

### V-46 — Password policy is length only
- **Where:** `main.rs:179`, `:293`. 10 characters minimum; no maximum, no common/breached-password list, no check against the user name; user names are case-sensitive (`admin` and `Admin` can coexist). With V-33's global budget an attacker can still spray ≈57 000 guesses/day across accounts.
- **Reproduction:** creating a user with password `password12`, or `samesame12` for user `samesame12` → 200.
- **Fix:** deny-list of common passwords, reject password = user name, maximum length (e.g. 256), case-insensitive unique user names; MFA (V-23).
- **Regression checks:** two V-46 checks.

## Informational

### V-47 — Origin check ignores the scheme and accepts non-web schemes
`main.rs:1026-1031` compares only the part after `://` with `Host`: `Origin: https://127.0.0.1:8820`, `file://127.0.0.1:8820`, `chrome-extension://127.0.0.1:8820` are accepted on an http listener; a missing `Origin` is accepted. Not exploitable from a browser today because the required custom header forces a CORS preflight that the server answers 405 without any `Access-Control-*` header (tested). Fix: compare the full origin with `NF_PUBLIC_ORIGIN` (V-36); also check `Sec-Fetch-Site`. Checks: two V-47 checks.

### V-48 — Client router passes unvalidated hash segments into API paths; bidi characters in names
`web/app.js:777,784` passes raw `location.hash` segments (`#/calc/<id>`, `#/project/<id>`, `#/amoeba/dependencies/<id>`) into fetch paths; the URL parser normalises `\` and `..`, so a crafted link can point the view at another resource. No exploitable target was found (views need matching GET responses; POSTs use the same normalised path). Stored names keep Unicode bidi control characters (U+202E) that can disguise file names. Fix: accept only `^\d+$` ids in the router; strip bidi controls server-side. Not automated (manual review item).

### V-49 — Unauthenticated information disclosure and verbose errors
`/api/health` (`main.rs:201`) gives anyone the exact engine version, uptime (restart times) and `setup_required`; 500 responses return raw SQLite error text (`main.rs:188,627,877`), 400 echoes serde messages. Fix: health returns only `status` (and `setup_required` only before V-36's fix is configured); generic error bodies, details to the server log. Check: `V-49 unauthenticated health does not disclose engine version or uptime`.

---

## Tested and found sound (at the level of this review)

- **Setup race:** 20 parallel `/api/setup` calls → exactly one 200, nineteen 409 (lock held across count and insert).
- **Authorisation matrix:** 17 routes × {anonymous, non-member engineer, non-member viewer}: anonymous 401 everywhere; non-members 404 on every project-scoped object (project, job, dependencies, DXF, replay, correction, obligation, advisory, upload, calculation); role checks 403 (viewer cannot replay/correct/resolve/acknowledge/calculate/create projects; non-admins cannot list/create/deactivate users, add members, read audit/verify/security status). Viewer as member can read and download DXF only. Removing a member takes effect on the next request. List endpoints (projects, calculations, obligations, lessons, dashboard, summary) are scoped. Role is read from the database per request; no role-change endpoint exists; no mass-assignment (typed request enum; `role` only via the admin-only user creation).
- **Deactivated users:** sessions revoked and `active = 1` checked on every request.
- **Timing / enumeration:** wrong password for existing vs unknown user: 20.0–22.6 ms vs 20.2–22.4 ms; identical messages and status codes.
- **SQL injection:** every request value is bound; the only formatted SQL fragments are integer ids and fixed strings (`scope()`, `calc_rows`, `WHERE project_id = {id}` with `id: i64`). SQL metacharacters stored as data.
- **Stored XSS through the new client:** payloads (`<img onerror>`, `<svg onload>`, attribute breakers, `javascript:` links) in project name and description, document file name, correction reason, lesson detail, advisory message and note, obligation note, `surface_reference`, audit details; headless Chrome visited 21 views (every tab, every record expanded) as administrator: no dialog, no `alert`, no injected element, no CSP violation, no page error. `viewer.js` builds SVG with `createElementNS`/`setAttribute`/`textContent` only; curve, layer and part names are engine constants. Re-run by `tests/security_r6.sh` (V-14 check passes).
- **CORS:** no `Access-Control-Allow-*` header on any response; preflight answers 405. Simple (text/plain) cross-site POST refused (header missing → 403). PUT/DELETE/PATCH → 405.
- **Anti-forgery behind Caddy (by review):** Caddy's `reverse_proxy` passes the client's `Host` unchanged, so `Origin: https://<domain>` matches `Host: <domain>`; the server ignores `X-Forwarded-Host`/`-For`, so a forwarded header cannot fool the check. A non-browser client can set both Host and Origin, but has no victim cookie to abuse.
- **Path traversal:** static files are compiled into the binary (`include_str!`); `/..%2f..`, `//etc/passwd`, `/index.html`, `/web/app.js` → 404. DXF part name is a lookup key (`..%2f..%2fstorage.key` → 404). Blobs addressed by their SHA-256.
- **Downloads:** `Content-Type: application/dxf`, `Content-Disposition: attachment; filename="NOT-RELEASED_…_calc3_header_fa453579.dxf"` (built from integer id, engine part name and hash — no header injection), `nosniff`, `no-store`, CSP. Superseded/rejected → 409.
- **DXF content injection:** the writer emits only fixed layer names (`CUT_OUTER`, `CUT_INNER`, `REFERENCE`), numbers and a fixed comment; no user string reaches DXF text.
- **Input limits:** 300 KB JSON → 413; JSON nested 100 000 deep → 400, server healthy; extreme numbers (1e300, 1e-310, near-90° angles) computed in ≤ 0.1 s; arc sampling is a fixed 96 segments; list endpoints capped (200/300/500 rows). Replay: 64 parallel replays, health 0.31 s, results correct.
- **Secrets:** passwords and session tokens never stored in clear (Argon2id hex, SHA-256 of token); the server prints nothing per request; error messages contain no key material.
- **Dependencies:** `cargo audit` 0.23.0, advisory database commit `472a4c9` (18 Nov 2026, 1 533 advisories): **0 vulnerabilities, 0 warnings** (unmaintained/yanked) over 104 crates. Key versions: axum 0.8.9, hyper 1.12.0, tokio 1.54.1, rusqlite 0.43.0 (bundled SQLite via libsqlite3-sys 0.40.0), aes-gcm 0.11.1, argon2 0.6.0, rand 0.10.4, sha2 0.11.1, serde_json 1.0.154. Licences: all permissive (MIT/Apache-2.0 family, Zlib, BSD-3-Clause, Unicode-3.0, BSL-1.0; `r-efi` offers MIT). `cargo deny` is not installed; no licence policy file exists (V-32 stays open).
- **File upload / document or CAD parsing:** no parser exists. `membrane::admit` (`crates/nf-server/src/membrane.rs`) only compares magic bytes and looks for the word `SECTION` in the first 2 KB; files are stored encrypted and never interpreted; there is no endpoint that returns a stored document. **Standing merge condition (M1, V-26):** no PDF, image, CAD or STEP parser may be merged unless it runs in a separate, network-less, resource-limited (CPU, memory, time, file descriptors) process per file, with fuzz tests and malformed-file tests.

## Status of the four previously open High findings

| ID | Before | Now | Evidence |
|---|---|---|---|
| V-20 No TLS | Open | **Mitigated by the deployment recipe only.** Browser↔Caddy is HTTPS with HSTS; nf-server binds 127.0.0.1; unit sets `NF_COOKIE_SECURE=1` (verified locally: cookie carries `Secure`). The product itself still speaks plain HTTP, and any deployment without the proxy is unprotected. The live VPS was installed by hand behind an existing Caddy; its Caddyfile and environment were not inspected (out of scope) — the owner should confirm `NF_COOKIE_SECURE=1` and the 127.0.0.1 bind there. Keep High for the product; residual for the recipe: Low. | `deploy/*` review; local test |
| V-21 Database not encrypted | Open | **Open, unchanged.** Plaintext observed: project names/descriptions, file names, usernames, results, lessons, audit log. On the VPS only file permissions (0700 data dir) and whatever disk encryption the provider offers protect it; backups copy it in clear. | SQLite inspection |
| V-22 Single key, no rotation | Open | **Open; placement improved.** Deploy keeps the key in `/etc/nexus-fabric/master.key` (0440 root:nexusfabric, read-only to the service), separate from `/var/lib/nexus-fabric`. Still one key, no rotation, no KMS; the documented backup puts key and data together; see V-42. | `deploy/install.sh:26-28`, README:19 |
| V-23 No MFA | Open | **Open; exposure increased.** The instance is now reachable from the internet; together with V-33 (anonymous lockout), V-36 (setup claim) and V-46 (weak policy) authentication is the weakest area. | — |

Other earlier findings re-checked: V-03 holds per account but its global rule is now V-33; V-06 headers present, CSP widened (V-45); V-10 holds for JSON, upload route see V-35; V-14 re-verified with the new client; V-25 still open (`security_r6.sh` deletes the newest five audit rows; `/api/audit/verify` still reports `intact: true`).

## What could not be tested

- The live VPS, its Caddy configuration, TLS settings, firewall and environment (forbidden by scope). Caddy was not installed locally; header behaviour through the proxy is from configuration review, not observation.
- `deploy/install.sh` was not executed (needs root on a supported OS and network downloads).
- Session expiry after 12 h (code review only); systemd sandbox directives (no systemd in the test container); actual OOM-kill under `MemoryMax=1G` (RSS growth measured instead).
- Browsers other than Chromium; no fuzzing campaign; no independent penetration test.
