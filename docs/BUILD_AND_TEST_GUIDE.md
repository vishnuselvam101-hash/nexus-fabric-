# Build and Test Guide

Updated 18 November 2026

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

First start: open the address in a browser and create the administrator account. Then: *Projects & Documents* → new project → *New development calculation*.

**Keep `storage.key` with the data and back both up together; without the key the stored documents cannot be decrypted. Protect it: anyone with the key and the files can read them.**

## Tests

```
cargo test --workspace            # 34 unit and integration tests
bash tests/e2e_smoke.sh           # 61 end-to-end checks
bash tests/security_smoke.sh      # 52 adversarial security checks (port 8792)
cargo audit                       # dependency advisories (cargo install cargo-audit) against a fresh temporary server (port 8791, override with NF_TEST_PORT)
```

Browser test (needs an empty data directory because it performs first-run setup):

```
NF_DATA_DIR=$(mktemp -d) NF_LISTEN=127.0.0.1:8793 target/debug/nf-server &
NF_BASE=http://127.0.0.1:8793 node tests/ui_smoke.js
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
