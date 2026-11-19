# Backend Architecture (as built)

Audit date 19 November 2026 · source of truth: `crates/` at commit `8f352ef` (release binary built 02:31 from a clean tree) · companion documents: [BACKEND_AUDIT.md](BACKEND_AUDIT.md), [DEFECT_REGISTER.md](DEFECT_REGISTER.md), [TECHNICAL_DEBT.md](TECHNICAL_DEBT.md).

This document describes what exists, not what the specifications plan. Everything here runs in **one operating-system process** (`nf-server`), plus a short-lived copy of the same executable started for each replay audit.

## 1. Components

```mermaid
flowchart LR
  subgraph Client["Browser (untrusted)"]
    UI["web/ index.html, app.js, viewer.js, styles.css<br/>(compiled into the binary with include_str!)"]
  end
  subgraph Edge["Optional reverse proxy (deploy/Caddyfile)"]
    CADDY["Caddy: TLS, HSTS, gzip/zstd"]
  end
  subgraph Proc["nf-server process (tokio multi-thread runtime, axum 0.8)"]
    SEC["security_layer middleware<br/>X-NF-Request header + Origin==Host check,<br/>CSP and other headers"]
    LIM["DefaultBodyLimit 256 KiB<br/>(50 MiB + 1 KiB on /documents)"]
    H["Handlers in crates/nf-server/src/main.rs<br/>auth, users, projects, upload, calculations,<br/>replay, corrections, obligations, lessons, audit"]
    MEM["membrane.rs<br/>content-type sniffing, size limit"]
    CRY["crypto.rs<br/>Argon2id, AES-256-GCM, random"]
    CORE["nf-core<br/>Record model, closure hash,<br/>job::run, gates, deps::propagate"]
    UEM["uem<br/>develop, verify, profile"]
    DXF["nf-dxf<br/>writer + independent reader"]
    MTX["std::sync::Mutex&lt;rusqlite::Connection&gt;<br/>ONE connection for all requests"]
  end
  subgraph Child["Replay child (same executable, argv: replay-check)"]
    RC["job::run(request) -> closure hash + record ids on stdout"]
  end
  subgraph Disk["NF_DATA_DIR (default ./data)"]
    DB[("nexus-fabric.sqlite<br/>WAL, synchronous default (FULL)")]
    BLOBS[("blobs/&lt;sha256&gt;.bin<br/>AES-256-GCM sealed")]
    KEY[["storage.key (32 bytes)<br/>or NF_KEY_FILE elsewhere"]]
  end

  UI -->|HTTPS| CADDY -->|HTTP 127.0.0.1:8787| SEC
  UI -.->|plain HTTP if no proxy| SEC
  SEC --> LIM --> H
  H --> MEM
  H --> CRY
  H --> CORE
  CORE --> UEM
  CORE --> DXF
  H --> MTX --> DB
  H -->|std::fs::write / read| BLOBS
  CRY --> KEY
  H -->|spawn, JSON on stdin, blocking wait| RC
```

Module boundaries:

| Crate | Lines | Role | I/O |
|---|---|---|---|
| `uem` | 1,124 | Developments (cylinder, cone, eccentric cone, branch stub, header with opening), verification checks, profile topology, bend formulas (library only) | none |
| `nf-dxf` | 328 | R12 subset writer, independent reader, re-import comparison | none |
| `nf-core` | 997 | Content-addressed `Record`, closure and closure hash, `job::run` (develop → verify → profile → DXF → re-import → gates), release-state rule, dependency propagation and obligation discharge | none |
| `nf-server` | 1,285 | Everything else: HTTP, auth, SQLite schema and queries, blob store, membrane, replay child, lessons/advisories, read-time gate evaluation | SQLite, files, child process |

The three library crates are pure and deterministic; all state, authorisation, persistence and read-time policy (G00, G18, G19, G20) live in one 1,115-line file, `crates/nf-server/src/main.rs`.

## 2. Data flow of a calculation job

```mermaid
sequenceDiagram
  autonumber
  participant B as Browser
  participant M as security_layer
  participant H as calculate()
  participant J as nf_core::job::run
  participant L as DB mutex
  participant S as SQLite
  participant F as blobs/
  B->>M: POST /api/projects/{id}/calculations (X-NF-Request, JSON Request)
  M->>H: header and Origin pass
  H->>L: lock (auth: session lookup), unlock
  H->>H: role admin or engineer, serde -> Request (unknown fields ignored)
  H->>J: run(request, username) on the async worker thread
  J-->>H: JobOutput: records, patterns, checks, gates, closure_hash, DXF text<br/>(engine refusal => no pattern, gates = [G05], stored anyway)
  H->>L: lock
  H->>S: project exists? member?
  loop each record (about 16-60)
    H->>S: INSERT OR IGNORE INTO records (autocommit, fsync)
  end
  H->>F: std::fs::write(sealed DXF) if file absent (no fsync, no rename)
  H->>S: INSERT INTO calculations (state column frozen here)
  H->>S: INSERT audit (hash-chained)
  H->>S: SELECT lessons, INSERT advisories
  opt REJECTED with patterns
    H->>S: INSERT records + lessons (engine_failure)
  end
  H-->>B: 200 {id, state, advisories}
  Note over H,S: No transaction: every statement commits on its own.
  B->>H: GET /api/calculations/{id}
  H->>S: result_json + obligations + advisories + lessons
  H->>H: effective_gates(): G18/G19 rewritten if present, G20 appended,<br/>G00 if superseded, release_state() recomputed
  H-->>B: full result incl. records (about 150 KB for an assembly)
```

Corrections (`POST /api/calculations/{id}/corrections`) hold the DB lock for the whole handler: attestation record → `job::run(new request)` → `persist_calc(supersedes = old)` → lesson → advisories → one finding record plus one `obligations` row per affected record.

Replay (`POST /api/calculations/{id}/replay`) reads the stored request, releases the lock, starts `current_exe() replay-check`, waits for it synchronously, then re-checks the identity of each stored record. It writes `replay_json` and may set `state = 'REJECTED'` plus an `integrity_failure` lesson.

## 3. Storage

```mermaid
erDiagram
  users ||--o{ sessions : "user_id (no FK)"
  users ||--o{ project_members : "user_id (no FK)"
  projects ||--o{ project_members : "project_id"
  projects ||--o{ documents : "project_id"
  projects ||--o{ calculations : "project_id"
  projects ||--o{ records : "project_id = FIRST writer only"
  projects ||--o{ lessons : "project_id"
  calculations ||--o| calculations : "supersedes"
  calculations ||--o{ obligations : "origin_calc / target_calc"
  calculations ||--o{ advisories : "calc_id"
  lessons ||--o{ advisories : "lesson_id"
  documents }o--|| records : "record_id (source_artifact)"
  calculations }o--o{ records : "ids embedded in result_json only"
  documents }o--|| BLOB : "sha256"
  calculations }o--o{ BLOB : "patterns[].dxf_sha256 inside result_json"
```

- 12 tables, created with `CREATE TABLE IF NOT EXISTS` (`main.rs:53-66`). The only indexes are the implicit primary-key and unique ones (`users.username`, `sessions.token_hash`, `records.id`, `project_members` PK). There are no declared foreign keys (so `PRAGMA foreign_keys = ON` at `main.rs:1103` has no effect), no CHECK constraints, and no schema version (`PRAGMA user_version` = 0). Migrations are four `ALTER TABLE` statements whose errors are ignored (`main.rs:1105-1108`).
- `calculations.result_json` holds the whole `JobOutput`, including every record (about 148 KB for an assembly). The same records are also written to `records`. `calculations.state` is written once, at insert, and changed again only by a replay mismatch. `calculations.dxf_sha256` is never written.
- `records` is append-only through `INSERT OR IGNORE`, so a record keeps the `project_id` of the first project that produced it.
- Blob store: `blobs/<sha256-of-plaintext>.bin` = 12-byte nonce plus AES-256-GCM ciphertext. Uploaded documents and generated DXF share it. A blob is written only if no file with that name exists.
- Key: `NF_KEY_FILE`, or `NF_DATA_DIR/storage.key`. A new key is created whenever the file cannot be read.
- SQLite settings: `journal_mode = WAL` (`main.rs:1103`). `synchronous` is not set, so the SQLite default (FULL) applies. Busy timeout is not set, so rusqlite's 5,000 ms default applies (a request panicked after 5.07 s; see BACKEND_AUDIT §4). There are no explicit transactions anywhere.

## 4. Trust boundaries

```mermaid
flowchart TB
  subgraph Z_net["Untrusted: network / browser"]
    A1["Any HTTP client"]
  end
  subgraph Z_edge["Boundary 1: TLS proxy (optional, deploy/)"]
    P["Caddy"]
  end
  subgraph Z_app["Boundary 2: nf-server request pipeline"]
    X["Anti-forgery: X-NF-Request header required on non-GET /api/*;<br/>Origin, if present, must equal Host"]
    AU["Session cookie -> sessions JOIN users (active=1, unexpired)"]
    RO["Role check: admin / engineer / viewer"]
    PM["Project membership: can_access() / scope() SQL filter"]
    MB["Membrane (uploads): magic-byte type, 1..50 MiB, never parsed"]
    VAL["Engineering input: serde types + uem finite_pos/range checks.<br/>A refusal is STORED as a job, not rejected"]
  end
  subgraph Z_proc["Boundary 3: OS process"]
    CH["Replay child: receives only {request, entered_by} on stdin"]
  end
  subgraph Z_disk["Boundary 4: data at rest"]
    D1["SQLite file: plaintext; trusted on read with no integrity check<br/>(result_json drives gates and DXF selection)"]
    D2["Blobs: AES-GCM + SHA-256 checked on read"]
    D3["Key file: 0600 if created by the server"]
  end
  A1 --> P --> X --> AU --> RO --> PM --> MB
  PM --> VAL
  VAL --> D1
  MB --> D2
  PM --> CH
```

Unauthenticated endpoints: `GET /api/health`, `POST /api/setup` (only while the `users` table is empty), `POST /api/login`, `POST /api/logout`, and the four static assets.

What is trusted without verification: everything read back from SQLite. Stored records are re-hashed only during a replay. `result_json` (gates, patterns, DXF hashes) is never checked against its closure hash on the read path; see BE-002.

## 5. Capabilities that do not exist (not audited)

| Capability named in the brief | Status in code |
|---|---|
| Structural frames, trusses, structural analysis | **Not implemented.** No code, types or endpoints. |
| Drawing ingestion / external DXF ingestion | **Not implemented.** `nf_dxf::read` is used only to re-import DXF text the server itself just wrote (`job.rs:262`). Uploaded files are never parsed (`membrane.rs:1-3`; the upload response says `"interpretation":"not implemented"`). |
| File upload | **Partly exists, contrary to the brief.** `POST /api/projects/{id}/documents` accepts raw bytes (≤ 50 MiB), sniffs the type (PDF, PNG, JPEG, TIFF, STEP, ASCII DXF), encrypts and stores the file, and records a snapshot. No content is interpreted. It is audited below as storage only. |
| Background job queue / scheduler | **Not implemented.** Every job runs synchronously inside the HTTP request. The only out-of-request work is the replay child, which is still awaited by the request. |
| Multi-service architecture | **Not implemented.** One binary, one process, one SQLite connection. The replay child is the same executable. |
