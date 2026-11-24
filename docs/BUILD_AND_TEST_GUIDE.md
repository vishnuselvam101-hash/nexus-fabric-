# Build and Test Guide

Updated 19 November 2026 (fix pass: setup token, host allow-list, start-up refusals, new API answers)

## Requirements

Rust toolchain (built with 1.97), a C compiler (for bundled SQLite). For the tests additionally: `bash`, `curl`, `python3`; for the browser test Node.js with Playwright and Chromium. No internet access is needed at run time; building needs the crates (online, or a vendored copy).

## Build

```
cargo build --release -p nf-server        # binary: target/release/nf-server
```

The web client is embedded in the binary; there is no separate front-end build.

## Run

```
NF_DATA_DIR=/path/to/data NF_LISTEN=127.0.0.1:8787 target/release/nf-server
```

| Variable | Default | Meaning |
|---|---|---|
| `NF_DATA_DIR` | `./data` | Database (`nexus-fabric.sqlite`), encrypted blobs (`blobs/`), storage key (`storage.key`) |
| `NF_KEY_FILE` | `<data>/storage.key` | Location of the storage key. **Set it to a path outside the data directory**, on separately protected storage |
| `NF_COOKIE_SECURE` | unset | Set to `1` when the server is reached through TLS, so the session cookie is marked Secure |
| `NF_LISTEN` | `127.0.0.1:8787` | Listen address. The server speaks plain HTTP: bind to localhost or put a TLS-terminating proxy in front |
| `NF_ALLOWED_HOSTS` | (none) | Comma-separated host names the server answers to, in addition to `localhost`, `127.0.0.1` and `[::1]`. A request with any other `Host` is refused with 421. **Set it to the public name when a proxy is in front** |
| `NF_TRUSTED_PROXY` | (none) | Comma-separated IP addresses of the reverse proxy. Only connections from these addresses may state the client address in `X-Forwarded-For` (right-most hop that is not a trusted proxy). Unset: the header is ignored and the connection's address is used, so behind a proxy all clients share one address for sign-in throttling |
| `NF_SETUP_TOKEN` | (generated) | One-time secret required to create the first administrator (at least 16 characters). Unset: the server generates one at first start and writes it to `<data>/setup-token` (mode 600); the file is deleted once the administrator exists |
| `NF_HEADER_TIMEOUT_SECS` | 15 | Longest wait for a complete request head, also on an idle connection |
| `NF_REQUEST_TIMEOUT_SECS` | 120 | Longest time for a whole request (body and work); then 408 |
| `NF_MAX_CONNECTIONS` | 512 | Open connections served at once; further ones wait |
| `NF_SHUTDOWN_GRACE_SECS` | 20 | After SIGTERM/SIGINT no new connection is accepted and requests in flight get this long to finish |

First start: the server prints where the one-time setup token is. Open the address in a browser, enter a user name, a password and the setup token, and the administrator account is created. Then: *Projects & Documents* → new project → *New development calculation*.

The server refuses to start, with a message that says why and what to do, when: the key file is missing, unreadable, world-readable, of the wrong length or not the key the data was created with, while data exists (it never creates a new key beside existing data); the database is missing, empty or damaged while the data directory holds an earlier installation (it never falls back to first-run mode); the database fails SQLite's consistency check; the schema is newer than the program.

**Keep `storage.key` with the data and back both up together; without the key the stored documents cannot be decrypted. Protect it: anyone with the key and the files can read them.**

## Tests

```
cargo test --workspace            # 82 unit, integration, stress and regression tests
cargo clippy --workspace --all-targets -- -D warnings
bash tests/e2e_smoke.sh           # 87 end-to-end checks
bash tests/security_smoke.sh      # 66 adversarial security checks (port 8792)
NF_SETUP_TOKEN=r6-setup-token-0000001 NF_R6_SETUP_ARGS='-H X-NF-Setup-Token:r6-setup-token-0000001' bash tests/security_r6.sh   # round-6 findings; checks of open findings fail by design
cargo audit                       # dependency advisories (cargo install cargo-audit) against a fresh temporary server (port 8791, override with NF_TEST_PORT)
```

Browser test (needs an empty data directory because it performs first-run setup):

```
NF_SETUP_TOKEN=ui-smoke-setup-token NF_DATA_DIR=$(mktemp -d) NF_LISTEN=127.0.0.1:8793 target/release/nf-server &
NF_BASE=http://127.0.0.1:8793 node tests/ui_smoke.js      # uses the same token (override with NF_SETUP_TOKEN)
```

The script exits 0 on success and prints what it observed (single-part and assembly results, obligations before and after a decision, no script errors, no overflow at tablet width).

The replay audit starts the same binary as a child process (`nf-server replay-check`); the binary must be allowed to execute itself.

## What passing tests do and do not show

They show that the code reproduces the documented reference values, that verifiers catch the seeded faults, that the API enforces authentication and roles, and that results are reproducible. They do **not** show that the mathematical models are right for real fabrication (no independent review), that DXF files are accepted by any CAM system, or that anything is fit for manufacturing.

## Repository layout

```
crates/uem        mathematics: developments, profile checks, verification
crates/nf-dxf     DXF writer and independent reader
crates/nf-core    AMOEBA records, closure, gates, development job
crates/nf-server  HTTP server, storage, authentication
web/              client (embedded into the server binary)
tests/            end-to-end and browser smoke tests
docs/             specifications, decisions, status
```

## Creating the first administrator from the command line

Creating the first administrator needs the installation's one-time setup token (the value of `NF_SETUP_TOKEN`, or the content of `<data>/setup-token` written at first start). The browser client asks for it on first visit. From a shell on the server itself:

```
curl -sS -X POST http://127.0.0.1:8787/api/setup \
  -H 'Content-Type: application/json' -H 'X-NF-Request: 1' \
  -H "X-NF-Setup-Token: $(sudo cat /var/lib/nexus-fabric/setup-token)" \
  -d '{"username":"<name>","password":"<a password of at least 10 characters>"}'
```

Without the token, or with a wrong one, the answer is 403; ten wrong attempts are followed by 429 for ten minutes. The token can also be sent as `"setup_token"` in the body. Do this from the server (loopback) before giving anyone the public address; even if the site is already reachable, nobody without the token can create the administrator.

Rules for every state-changing request under `/api/` (POST, PUT, PATCH, DELETE):

- The header `X-NF-Request: 1` is required; without it the server answers 403 "missing anti-forgery header".
- If the request carries an `Origin` header, it must match the `Host` header (`scheme://host[:port]` must equal `Host`); otherwise the server answers 403 "cross-origin request refused". `curl` sends no `Origin` unless you add one, so the example above needs only `X-NF-Request`.
- Passwords need at least 10 characters (setup and password change alike); a shorter one answers 400. Usernames are 3 to 64 characters: letters, digits, `.`, `_`, `-`.

- Passwords must not equal the user name or be one of a short list of very common passwords, and are at most 1024 bytes.
- The `Host` header must be `localhost`, `127.0.0.1`, `[::1]` or a name in `NF_ALLOWED_HOSTS`; otherwise 421.

Answers worth knowing (changed on 19 November 2026):

| Request | Answer |
|---|---|
| `POST /api/projects/{id}/calculations`, computed | 200 `{id, state, advisories}`; `state` is `REVIEW_REQUIRED` or `REJECTED` |
| same, refused by the engine (invalid, unsupported, inconsistent or out-of-range input) | **422** `{error, id, state: "REFUSED", outcome, field, advisories: 0}`. The attempt is kept as job `id` in state `REFUSED` and in the audit log; it has no pattern, no DXF, cannot be replayed or corrected, and is counted as `refused`, not as review required |
| same, unknown family, wrong type or unknown field | 400, nothing stored |
| `POST /api/calculations/{id}/corrections`, corrected value refused or rejected | **422** `{error, outcome, field}`; nothing changes: the job stays current, no lesson, no obligations; the audit log records `correction_refused` |
| `POST /api/calculations/{id}/replay`, replay could not be run | **503** `{error, infrastructure: true}`; the job is unchanged |
| any write while the database is locked by another process | 503 `{error, retry: true}` with `Retry-After` |
| any write while storage is full | 507 `{error, storage_full: true}`; nothing changed |
| `GET /api/calculations?limit=&before=` | newest first, at most 200 per request |
| `GET /api/audit?limit=&before=` | newest first, default 300, at most 1000 |
| `GET /api/audit/verify` | `intact` is true only if the chain, the anchored end of the log and SQLite's integrity check all hold (`chain_intact`, `anchor_intact`, `database_intact`) |
| `GET /api/health` (no sign-in) | `status`, `setup_required`, `build_notice` only; engine and uptime are in `/api/dashboard` |

Job states are `REVIEW_REQUIRED`, `REJECTED` and `REFUSED`. `APPROVED` cannot occur. A result computed by another engine build than the running one is `REJECTED` with gate G21 "computed by a superseded engine version — recompute"; a stored result that fails its integrity check is `REJECTED` with gate G22.

Setup works once: a second call answers 409. Replace the port if `NF_PORT` was set at install time (the install script defaults to 8787). `GET /api/health` answers `{"status":"ok", "setup_required": true, ...}` while no account exists, which is a quick check that the server is up.
