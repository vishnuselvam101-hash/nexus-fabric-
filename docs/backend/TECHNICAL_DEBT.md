# Backend Technical Debt

Audit of 19 November 2026. This document separates what should be **kept** from what is **missing** for production. The missing items are ranked by risk to engineering results first and operability second. Defect IDs refer to [DEFECT_REGISTER.md](DEFECT_REGISTER.md).

No rewrite is recommended. The stack is appropriate for a single-site, offline-capable engineering tool: one Rust binary, axum, SQLite in WAL mode, and a content-addressed blob directory. Every item below can be done inside the current architecture.

## 1. Sound, and to be preserved

| What | Where | Why it matters |
|---|---|---|
| Pure, deterministic engineering crates with no I/O | `uem`, `nf-dxf`, `nf-core` | Makes replay, closure hashing and testing possible. Keep all persistence out of them |
| Content-addressed immutable records, canonical JSON, exact float round trip | `nf-core/src/lib.rs:59-82`, `serde_json/float_roundtrip` | Gives tamper evidence and deduplication; tested by a round-trip regression |
| Release closure and closure hash independent of unrelated records | `lib.rs:88-117` | The right basis for replay and for read-time integrity checks (BE-002 should use it) |
| Gates that never pass what was not evaluated; G16 hard NOT_IMPLEMENTED | `job.rs:297-334` | Conservative by construction. Keep, and close the read-time hole (BE-002) |
| Independent DXF re-import of every file written | `nf-dxf/src/lib.rs:151-287`, `job.rs:261-270` | Catches writer defects (regression 001 proved its worth) |
| Corrections as attestations plus supersession, never edits; obligations discharged only by evidence or a reasoned decision | `main.rs:846-898`, `deps.rs` | The right model; needs atomicity (BE-005) and pre-validation (BE-004) |
| Replay in a separate OS process that receives only the request | `main.rs:749-819` | Good isolation; needs error classification (BE-008) and a timeout (BE-019) |
| Per-project membership in every query, from integer ids only | `main.rs:150-158` | No injection surface; consistent 404 hiding |
| Anti-forgery header plus Origin check, strict CSP, SameSite=Strict, Argon2id, AES-GCM blobs checked by hash on read | `main.rs:1018-1048`, `crypto.rs`, `main.rs:111-118` | A solid baseline |
| Hash-chained audit log with a verifier | `main.rs:68-86,353-371` | Keep; add an external anchor later |
| Engine refusals are explicit and named (`UemError::OutOfRange` with a reason) | `uem/src/lib.rs:108-134` | The information for a proper 422 already exists (BE-003) |
| systemd sandboxing in the unit file | `deploy/nf-server.service` | Good defence in depth |

## 2. Missing or weak for production, ranked

### Tier 1: affects the correctness or trustworthiness of results

1. **Request-consistency validation before any job is stored** (BE-001, BE-003, BE-015, BE-016).
   - One function from `Request` to `Result<ValidatedRequest, Vec<FieldError>>` in `nf-core`.
   - It covers the surface-reference vs development-radius relation, plausible ranges, unknown fields, and geometric pre-conditions that `uem` checks today only by failing.
   - The server returns 422 and stores nothing.
2. **Fail-closed release state on read** (BE-002).
   - Use a required gate set owned by code, not by stored JSON.
   - Verify `closure_hash` against stored record ids on every read.
   - An unreadable result is REJECTED.
3. **Transactions around every multi-statement write** (BE-005), with errors propagated instead of `unwrap` (BE-007) or ignored (BE-020).
   - This is a mechanical change. With rusqlite it means `let tx = db.transaction()?; …; tx.commit()?`.
   - It also removes most of the fsyncs per job.
4. **Atomic, self-healing blob store** (BE-006): temporary file, fsync, rename; verify an existing file before skipping the write.
5. **Engine build identity per result and a recall mechanism** (BE-011). Without it, a fixed engine defect cannot be traced to the stored results it affected.
6. **One source of truth for effective state** (BE-009). List, dashboard and detail must agree.
7. **Replay error classification** (BE-008). "Could not replay" must never become "integrity failure".
8. **Corrections validated before they supersede, and revocable** (BE-004).

### Tier 2: operability and resilience

9. **Key safety** (BE-010): create a key only on first initialisation, store its fingerprint, refuse to start on mismatch.
10. **Backup and restore** (BE-024): online snapshot (`VACUUM INTO` or the backup API) plus a blob manifest, a reconciliation and integrity command, and a rehearsed restore. Today's procedure stops the service and has never been tested by restoring.
11. **Monitoring and logging** (BE-021): `tracing` with request ids and an access log, panic capture (`CatchPanicLayer`), a deep health check, and minimal metrics (requests, latency, jobs by outcome, refusals, replay failures, DB busy events).
12. **Graceful shutdown** (BE-022) and a request timeout layer.
13. **Concurrency model** (BE-012): one writer connection plus a read pool; `spawn_blocking` for Argon2, file I/O and replay waits. There is no need for a different database at this scale.
14. **Indexes and versioned migrations** (BE-017, BE-023): `user_version`, FKs and CHECK constraints, and the six indexes listed in BE-017.
15. **Throttling redesign** (BE-013) and a **setup token** (BE-025).

### Tier 3: API maturity

16. **Pagination** with cursors and totals (BE-018).
17. **Idempotency keys** for job creation and uploads (BE-027).
18. **Uniform JSON error contract**, correct status codes, and an API version prefix (BE-026).
19. **Cancellation.** Not needed while jobs take 8–25 ms. It becomes necessary once jobs are queued (item 20).
20. **Background job execution.** Today every job runs inside its HTTP request; that is acceptable because the measured cost is 8–25 ms. A queue (a table of pending jobs plus one in-process worker, which needs no extra service) is justified only when slower families, drawing interpretation or batch replays arrive. Do it then, not before.
21. **Data model cleanup** (BE-014, BE-030): project-record membership table; stop embedding all records in `result_json`; remove dead columns and functions.

### Not in scope of this debt list (capabilities that do not exist)

These are features, not debt:
- structural frames, trusses and structural analysis;
- drawing or external DXF ingestion (uploads are stored, never parsed);
- a background job queue or scheduler;
- multi-service architecture.

File upload exists as encrypted storage only. Adding a parser to it will require the sandboxed worker described in the security documents before any of the above.

## 3. Suggested order of work for the fixing agent

Each step is small and independently testable:
- BE-001 + BE-003 + BE-015 + BE-016 (validation stage), then BE-002 (fail-closed gates).
- BE-005 + BE-007 + BE-020 (transactions and error propagation), then BE-006 (blob store).
- BE-004, BE-008, BE-009 (corrections, replay classification, effective state).
- BE-010, BE-011 (key fingerprint, engine id).
- Then Tier 2 and Tier 3.

Add the regression test named in the register with each fix, and extend `tests/e2e_smoke.sh` with the BE-001/003/004 request transcripts from [BACKEND_AUDIT.md](BACKEND_AUDIT.md).
