# Backend Audit: NEXUS FABRIC

Audit date: 19 November 2026. Scope: `crates/uem`, `crates/nf-dxf`, `crates/nf-core`, `crates/nf-server`, `deploy/`, and the backend claims in `docs/IMPLEMENTATION_STATUS.md`. This is an evidence-gathering audit only: no product code was changed.

Method:
- Read every line of the four crates (3,734 lines).
- Ran a private copy of the release binary (`target/release/nf-server`, sha256 `e1e69f98…f1e6cf`, built 02:31 from a clean tree at `284a5bb`) on `127.0.0.1:8801` with a scratch data directory.
- Drove it with curl, observed it with `strace`, and inspected and altered its scratch SQLite file with Python's `sqlite3`.
- Unless stated otherwise, every request below was sent with `X-NF-Request: 1` and an admin session.

Defect IDs (BE-xxx) refer to [DEFECT_REGISTER.md](DEFECT_REGISTER.md). Architecture diagrams are in [ARCHITECTURE.md](ARCHITECTURE.md).

**Headline.**
- The pure engineering crates are carefully built: deterministic, content-addressed, independently verified, and refusing to claim what they cannot show.
- The server around them is not yet trustworthy as a system of record:
  - it stores refusals as jobs;
  - it accepts geometrically impossible radius declarations;
  - it derives the release state from stored JSON in a way that fails *open* to `APPROVED`;
  - it never uses a transaction;
  - it writes blobs non-atomically;
  - it can regenerate its encryption key silently;
  - it turns an operating-system error during replay into a permanent integrity rejection.

---

## 1. Architecture and module boundaries

- **Sound.** `uem`, `nf-dxf` and `nf-core` have no I/O or global state. Records are content-addressed (`nf-core/src/lib.rs:72-82`). The job is a pure function of `(request, entered_by)` (`job.rs:439-462`). This makes replay possible, and it should be preserved.
- **Concentration.**
  - All persistence, authorisation and policy live in one file, `nf-server/src/main.rs` (1,115 lines). That includes the read-time gates G00, G18, G19 and G20 (`main.rs:667-718`).
  - SQL is built inline in handlers; there is no repository or service layer.
  - The engineering release policy is split between `nf_core::job::evaluate_gates` (write time) and `main.rs::effective_gates` (read time). Neither has the authoritative list of gates, which is the root cause of BE-002.
- **Single shared resource.** `App.db: Mutex<Connection>` (`main.rs:24`) serialises every request, readers included, even though the database is in WAL mode, which allows concurrent readers. Lock poisoning is recovered on purpose (`main.rs:75-77`). Because nothing runs in a transaction, a panic leaves partly written state behind (BE-005, BE-007).
- **Async misuse.**
  - Every handler is `async`, but all of them do blocking work on tokio worker threads: SQLite calls, Argon2 (`main.rs:239,301,307`), up to 50 MiB of AES encryption and file writes (`main.rs:473`), and `Child::wait_with_output` (`main.rs:767`).
  - There is no `spawn_blocking` anywhere.
  - Measured effect is in §6 (BE-012).
- **Dead or duplicate logic** (BE-030):
  - `job::replay_matches` (`job.rs:465`) is used only by tests; the server re-implements it (`main.rs:785-805`).
  - `calculations.dxf_sha256` (`main.rs:59`) is never written.
  - `Family::Relation` (`lib.rs:34`) is never produced.
  - `Affected.mandatory` (`deps.rs:51`) is always `true`.
  - `resolve_obligation` looks up the same project twice (`main.rs:921`, `main.rs:928`).
  - Family-name extraction is duplicated (`main.rs:508` and `main.rs:526-528`).
  - The membership filter is rewritten by string replacement (`main.rs:398`, `main.rs:963`).

## 2. API routes

All routes are in `router()` at `main.rs:1050-1087`. There is no API versioning: no `/v1` prefix and no version header. The engine version appears only inside results.

| Route | Auth / role | Observed status codes | Notes |
|---|---|---|---|
| `GET /api/health` | none | 200 | Reports `setup_required`, uptime and engine version without authentication. Checks nothing: DB errors read as 0 users (`main.rs:200`). BE-021, BE-034 |
| `POST /api/setup` | none, only while `users` is empty | 200 / 400 / 409 | First caller becomes admin; `install.sh` exposes the service publicly before setup (BE-025) |
| `POST /api/login` | none | 200 / 401 / 429 | Argon2 runs while the DB mutex is held; global throttle locks everyone out (BE-013) |
| `POST /api/logout` | none | always 200 | |
| `GET /api/me` | any | 200 / 401 | |
| `GET,POST /api/users` | admin | 200 / 400 / 403 | `prepare(...).unwrap()` (`main.rs:271-272`) |
| `POST /api/users/{id}/active` | admin | 200 / 400 / 404 | |
| `POST /api/password` | any | 200 / 400 / 403 | Two Argon2 runs while the lock is held |
| `POST /api/projects/{id}/members` | admin | 200 / 404 | Engineers who create a project cannot add colleagues |
| `GET /api/audit/verify`, `GET /api/audit`, `GET /api/security/status` | admin | 200 / 403 | Audit listing has a fixed `LIMIT 300`, no paging |
| `GET,POST /api/projects` | any / admin+engineer | 200 / 400 | Create returns 200, not 201; `unwrap` on insert panics on a DB error (§4) |
| `GET /api/projects/{id}` | member | 200 / 404 | Embeds up to 200 calculations, all documents and all members; 0.93 s at 50k jobs (§6) |
| `POST /api/projects/{id}/documents` | admin+engineer, member | 200 / 404 / 413 / 422 | Raw body. Duplicates accepted; records collide across projects (BE-014, BE-027) |
| `POST /api/projects/{id}/calculations` | admin+engineer, member | 200 / 400 / 403 / 404 | **Engine refusals are 200 and stored** (BE-003). Job runs before the membership check (BE-028) |
| `GET /api/calculations` | any (scoped) | 200 | `LIMIT 200`, no paging; `state` column is stale (BE-009) |
| `GET /api/calculations/{id}` | member | 200 / 404 | Fail-open release state (BE-002) |
| `POST /api/calculations/{id}/replay` | admin+engineer | 200 | Not idempotent; process errors become integrity failures (BE-008) |
| `GET /api/calculations/{id}/dxf/{part}` | member | 200 / 404 / 409 / 500 | Served for REVIEW_REQUIRED jobs, including BE-001 jobs |
| `POST /api/calculations/{id}/corrections` | admin+engineer | 200 / 400 / 404 / 409 / 500 | Accepts values the engine refuses (BE-004); 500 for a client-caused state (BE-026) |
| `GET /api/calculations/{id}/dependencies` | member | 200 / 404 | |
| `GET /api/lessons` | any (scoped) | 200 | `LIMIT 300` |
| `POST /api/advisories/{id}/acknowledge` | admin+engineer, member | 200 / 400 / 404 / 409 | |
| `GET /api/obligations` | any (scoped) | 200 | `LIMIT 500` |
| `POST /api/obligations/{id}/resolve` | admin+engineer, member | 200 / 400 / 404 / 409 | |
| `GET /api/dashboard`, `GET /api/amoeba/summary` | any (scoped) | 200 | Counts come from the stale `state` column (BE-009); full scans (BE-017) |

### 2.1 Error contract is inconsistent (BE-026)

Handler errors are JSON `{"error": "..."}`. Axum extractor rejections are plain text:

```
POST /api/projects  body '{bad'                -> 400 "Failed to parse the request body as JSON: key must be a string at line 1 column 2"
POST /api/projects  no Content-Type            -> 415 "Expected request with `Content-Type: application/json`"
GET  /api/calculations/abc                     -> 400 "Invalid URL: Cannot parse `abc` to a `i64`"
POST /api/projects/1/documents  51 MiB         -> 413 "Failed to buffer the request body: length limit exceeded"
POST /api/calculations/11/corrections (job whose surface_reference was invalid)
                                               -> 500 {"error":"parameter record not found"}
```

### 2.2 Validation of engineering input

- **Unknown fields are silently dropped** (BE-016). The following stores job #4 with neither field used, and the response does not say so:

  ```
  POST /api/projects/1/calculations
  {"family":"cylinder_shell","r_dev":100,"length":250,"thickness":12,"surface_reference":"outside"}
  -> 200 {"advisories":0,"id":4,"state":"REVIEW_REQUIRED"}
  ```

- **No upper bounds** (BE-015). `uem::finite_pos` checks only `> 0` and finite (`uem/src/lib.rs:128-134`). The following request was stored as REVIEW_REQUIRED with `quantities.area = null` (∞ serialised as null). `GET /api/calculations/5/dxf/shell` then returned 200 with a 301-digit coordinate (`6283185307179586000…000.0`).

  ```
  POST {"family":"cylinder_shell","r_dev":1e300,"length":1e300} -> 200 {"id":5,"state":"REVIEW_REQUIRED"}
  ```

  At the small end (`1e-300`) and for `header_length: 1e12` the verifiers catch the problem (REJECTED), so this is a missing plausibility check, not an engine error.

### 2.3 Known issue (b): invalid requests become stored jobs (BE-003)

Exactly what is sent, stored and returned:

```
POST /api/projects/1/calculations
{"family":"header_with_branch","header_radius":-150,"header_dev_radius":150,"header_length":1200,
 "branch_station":600,"branch_radius":100,"branch_dev_radius":100,"beta_deg":60,"delta":0,"s_end":400,
 "surface_reference":"outside"}
-> HTTP 200 {"advisories":0,"id":1,"state":"REVIEW_REQUIRED"}

GET /api/calculations/1
-> state "REVIEW_REQUIRED"
   outcome "out of range (header_radius): must be finite and > 0, got -150"
   gates [G05 class_assigned INCONCLUSIVE "No pattern produced: ...", G20 known_mistakes PASS]
   patterns [] , 16 records appended to `records`

POST ... same with "branch_radius":400,"branch_dev_radius":400 (header_radius 150)
-> HTTP 200 {"advisories":0,"id":2,"state":"REVIEW_REQUIRED"}
   outcome "out of range (delta+r): requires |delta| + r < R: tangent and partial intersections
            (including the equal-diameter tee) are not implemented in this build"
   gates [G05 INCONCLUSIVE, G20 PASS]
```

Mechanism:
1. Every `UemError::OutOfRange` is classed as non-critical (`job.rs:337`).
2. `err_output` then produces a gate table with only G05 = INCONCLUSIVE (`job.rs:344`).
3. `release_state` maps that to REVIEW_REQUIRED (`lib.rs:149-157`).
4. The server persists it like any other job (`main.rs:611-625`).
5. At read time `effective_gates` appends G20 = PASS (`main.rs:700-712`).

So the UI's "1 PASS · 1 INCONCLUSIVE" is exactly what the server returns. G16 (approval NOT_IMPLEMENTED) and every other gate are absent from refused jobs.

Consequences:
- The refusal is counted in `review_required` on the dashboard.
- It appears in the job lists.
- It accumulates lesson-driven advisories: job #11, a refusal for `surface_reference:"whatever"`, received an open advisory. This explains the "spurious advisory" in `docs/design/UI_CRITIC_REPORT_R1.md:69`.
- The DXF endpoint correctly answers 404 `no DXF for this part`.

`Degenerate` and `NotDevelopable` errors, by contrast, give G05 = FAIL, which is REJECTED.

### 2.4 Known issue (a): declared "outside" contact radius smaller than the development radius (BE-001)

```
POST /api/projects/1/calculations
{"family":"header_with_branch","header_radius":150,"header_dev_radius":160,"header_length":1200,
 "branch_station":600,"branch_radius":100,"branch_dev_radius":110,"beta_deg":60,"delta":0,"s_end":400,
 "surface_reference":"outside"}
-> 200 {"advisories":0,"id":3,"state":"REVIEW_REQUIRED"}
GET /api/calculations/3 -> 12/12 checks PASS; G10, G11, G15 PASS; G02 INCONCLUSIVE ("declared ... recorded, not checked")
   header developed_width 1005.3096491487338 (= 2π·160), branch developed_width 691.1503837897545 (= 2π·110)
GET /api/calculations/3/dxf/header -> 200, NOT-RELEASED_REVIEW_REQUIRED_calc3_header_e3f8a1b9.dxf
```

The relation between the contact radius, the declared reference surface and the development radius is never checked (`job.rs:366-436`). With `outside`, a neutral radius larger than the outside radius means a negative wall thickness. Nothing in the gate table distinguishes this job from a correct one. The verifiers cannot catch it, because they check each pattern against the development radius it was built from (`verify.rs:214-243`, `verify.rs:174-201`), and the interface check uses the same contact radii on both sides.

The same gap exists for:
- `inside` with development radius < contact radius;
- `neutral` with development radius ≠ contact radius;
- the single-part `branch_stub` family, which has no `surface_reference` at all.

### 2.5 Pagination, limits, idempotency

- **Pagination** (BE-018): hard `LIMIT`s with no cursor, offset or total count: calculations 200 (`main.rs:424`), lessons 300 (`main.rs:570`), obligations 500 (`main.rs:653`), audit 300 (`main.rs:1007`). Project documents and members are unbounded (`main.rs:442,451`).
- **Body limits**: 256 KiB globally, 50 MiB + 1 KiB for uploads (`main.rs:1069,1084`). A 1 MiB upload was accepted, so the per-route limit takes precedence as intended; 51 MiB got 413. Free-text fields (project description, reasons, notes) have no length limit below 256 KiB and are copied into records and the audit chain (BE-029).
- **Idempotency** (BE-027):
  - Repeated `POST .../calculations` creates duplicate jobs.
  - Uploading the same 1 MiB PDF twice to project 1 created documents #1 and #2 with the same sha256, plus a new snapshot each time.
  - Forty concurrent `POST .../replay` calls each appended a `replay_audit` record and an audit entry.
  - Corrections are the exception: a second correction of the same job gets 409, because the job is already superseded.

## 3. SQLite schema, constraints, indexes, transactions

**Schema** (`main.rs:53-66`): 12 tables.
- Indexes: only `sqlite_autoindex_users_1`, `sessions_1`, `records_1` and `project_members_1` (queried from the live file).
- No foreign keys: `pragma foreign_key_list(calculations)` returns `[]`, so `PRAGMA foreign_keys = ON` (`main.rs:1103`) does nothing.
- No CHECK constraints on `role`, `state`, `direction` or `decision`.
- No schema version: `pragma user_version` = 0. Migrations are `ALTER TABLE` statements whose errors are ignored (`main.rs:1105-1108`). BE-023.

**Journal settings.**
- `journal_mode = WAL`, set with `.ok()`, so a failure goes unnoticed (`main.rs:1103`).
- `synchronous` is not set, so FULL applies.
- `busy_timeout` is not set, so rusqlite's default of 5 s applies; observed failure at 5.07 s.

**Transactions: none.**
- No `transaction()`, `BEGIN` or `SAVEPOINT` appears in the code base. Each statement autocommits and, under WAL plus FULL, fsyncs.
- `strace -c` on one cone calculation counted more than a dozen `fsync(7)` calls on the database descriptor. For an assembly whose records already existed: 4 fsyncs, 84 `pwrite64`.

Multi-statement writes, and what a crash or panic between statements leaves behind (BE-005):

| Handler | Statements (in order) | State after a crash in the middle |
|---|---|---|
| `calculate` `main.rs:604-629` | N × record insert, DXF blob write(s), calculation insert, audit, advisories, optional lesson record + lesson row | Orphan records/blobs (harmless). Calculation without its advisories, so G20 PASS for a job that repeats a corrected mistake. Lesson record without lesson row |
| `upload` `main.rs:457-498` | blob, source record, document row, snapshot record, audit | Blob with no document; document with no snapshot, so the next snapshot's predecessor chain skips it |
| `correct` `main.rs:846-898` | attestation record, M × records, blob(s), **new calculation with `supersedes`**, lesson, advisories, then K × (finding record + obligation row), audit | **The old job is superseded but the new job has some or none of its obligations.** Simulated by deleting job #21's obligation rows: G18 changed from "3 of 11 mandatory scrutiny obligations are OPEN" to "No scrutiny obligations recorded for this result", and the state stayed REVIEW_REQUIRED with nothing open. The engineer's decision on the common-cause radii is silently bypassed. The `unwrap()` calls at `main.rs:889,894` turn any DB error inside the loop into exactly this state |
| `replay` `main.rs:774-819` | record, `replay_json`, `state='REJECTED'`, lesson, audit | Mismatch recorded without REJECTED, or the reverse |
| `acknowledge_advisory`, `resolve_obligation` | attestation record, update, audit | Attestation without state change (benign) |

Crashes are not hypothetical. There is no graceful shutdown (`axum::serve(...)` without `with_graceful_shutdown`, `main.rs:1114`), so every `systemctl restart` or upgrade during a request ends it mid-way (BE-022).

**Blob store** (BE-006): `store_blob` (`main.rs:102-109`) does `if !path.exists() { std::fs::write(path, sealed) }`.
- `strace` shows `openat(... blobs/b47f….bin, O_WRONLY|O_CREAT|O_TRUNC)`, with no temporary file, no `rename`, and no fsync of the file or directory.
- The DB row that references the blob is fsynced; the blob is not.
- A torn file is never repaired. Experiment:
  1. Truncate `b47f….bin` to 0 bytes (what a crash during `write` leaves).
  2. Re-run the identical cone job: `POST {"family":"cone_frustum","r1":210,"r2":100,"h":300}` gives 200, id 19. The file stays 0 bytes, because `exists()` is true.
  3. `GET /api/calculations/19/dxf/reducer` returns `500 {"error":"sealed blob too short"}`, permanently, for every job that produces that DXF.

**Record identity across projects** (BE-014): `put_record` uses `INSERT OR IGNORE` on a globally unique content id but stores a `project_id` (`main.rs:89-96`).
- Running in project 2 the same cylinder job already run in project 1 stored **0 of its 12 records under project 2** (`stored under project: [(1, 12)]`).
- One PDF uploaded to projects 1 and 2 produced three `documents` rows sharing one `source_artifact` record owned by project 1.
- Effects:
  - per-project record counts (`/api/dashboard.records`, `/api/amoeba/summary`) are wrong;
  - a project-1 member sees counts that include project-2 work;
  - per-project export or deletion cannot be implemented correctly on this table.

**Indexes and query cost** (BE-017). `calc_rows` (`main.rs:423-428`) has two correlated `COUNT(*)` subqueries per row, over `calculations.supersedes` and `obligations.target_calc`, neither indexed. `amoeba_summary` parses the JSON of every activity record on each call (`main.rs:983-990`). With 50,000 synthetic calculations, 50,000 obligations and 200,000 activity records on a copy of the database:

```
/api/calculations      0.976 s   /api/projects/1   0.929 s   /api/dashboard   0.796 s
/api/amoeba/summary    0.215 s   /api/obligations  0.016 s   /api/projects    0.005 s
GET /api/health issued 0.2 s after a dashboard request: 0.891 s (blocked on the mutex)
```

## 4. Error handling, panics, lock poisoning

- **Request-reachable panics** (BE-007). About 40 `.unwrap()` calls on SQLite results sit in handlers, for example `main.rs:271-272, 357-358, 398-402, 416, 425-427, 442-452, 479-494, 546-547, 570-576, 599, 654-660, 741-742, 809-812, 872, 889-894, 981-992, 1007-1008`.
  - Experiment: another process held `BEGIN IMMEDIATE` on the database for 8 s while `POST /api/projects {"name":"busy"}` was sent.
  - The request failed after 5.07 s with **no HTTP response** (curl exit 52, "empty reply").
  - The log shows `panicked at crates/nf-server/src/main.rs:416:166: called Result::unwrap() on an Err value: SqliteFailure(... DatabaseBusy ...) "database is locked"`.
  - The process survived, because the poisoned mutex is recovered (`main.rs:75-77`). Disk-full, read-only filesystem, `SQLITE_CORRUPT`, and an operator's `sqlite3` shell or backup tool trigger the same path.
  - Other panics: `&sha[..8]` in `get_dxf` (`main.rs:839`) panics if a stored hash is shorter than 8 bytes (BE-032). `canonical()` uses `expect("json")` (`lib.rs:68`), which cannot fail for a `Value`.
- **Fail-open release state** (BE-002, critical). `load_calc` turns unparseable `result_json` into `Value::Null` (`main.rs:649`). `effective_gates` deserialises the gate list with `unwrap_or_default()` (`main.rs:669`), rewrites G18 and G19 only *if present* (`main.rs:673,683`), appends G20, and calls `release_state`. An empty or partial table is all PASS, so the result is **APPROVED**. Experiment on the scratch database:

  ```
  UPDATE calculations SET result_json='{}' WHERE id=4;        GET /api/calculations/4 -> "state":"APPROVED", gates [G20 PASS]
  UPDATE calculations SET result_json='not json' WHERE id=2;  GET /api/calculations/2 -> "state":"APPROVED"
  job 9 (assembly, 3 OPEN obligations): remove only the "gates" key from result_json
      GET /api/calculations/9 -> "state":"APPROVED", gates [G20 PASS], 2 patterns
      GET /api/calculations/9/dxf/header -> 200 (file still named NOT-RELEASED_REVIEW_REQUIRED...)
  ```

  The open obligations of job 9 were ignored, because G18 was no longer in the stored table to be rewritten. This contradicts "APPROVED cannot be reached" (`IMPLEMENTATION_STATUS.md:17`) and is not covered by `tests/security_smoke.sh`, which tampers with `records`, not `result_json`.
- **Swallowed errors** (BE-020):
  - `audit()` ignores a failed insert (`main.rs:85`), and so do `record_lesson` (`main.rs:533-537`) and session/throttle bookkeeping (`main.rs:225,241,245,247-248`).
  - Several COUNT queries map a DB error to 0: `effective_gates` open obligations and advisories (`main.rs:670-671,697-698`), so G18 says "no obligations" and G20 says PASS when the database errs.
  - `can_access` and `setup` fail closed (correct).
- **Internal details in responses**: OS errors are written into stored replay reports (`"process_error":"No such file or directory (os error 2)"`), and SQLite messages from `create_user` go to clients (`main.rs:188`).

## 5. Job pipeline and replay child

- **Pipeline** (`job.rs:175-290`): develop → evidence → profile → DXF write → independent re-import → gates → closure. It is deterministic. Each step is an Activity record with declared work units. Sound; preserve it.
- **No input-consistency stage** between deserialisation and development. This is where BE-001, BE-003, BE-015 and BE-016 belong.
- **Replay child** (`main.rs:749-772`):
  - It is spawned from `std::env::current_exe()`, fed JSON on stdin, and awaited with the blocking `wait_with_output()` on an async worker.
  - There is no timeout, no concurrency limit, and stderr goes to `/dev/null`.
  - 40 concurrent replays raised `GET /api/health` latency to 123 ms. Each replay costs about 16 ms today.
- **Process failure is recorded as an integrity failure** (BE-008). Experiment: replace the executable on disk the way an in-place upgrade does (unlink, copy new file; `/proc/<pid>/exe` then reads `… (deleted)`), then `POST /api/calculations/12/replay`:
  - The report: `"matches":false,"process_error":"No such file or directory (os error 2)","replayed_closure_hash":""`.
  - The `calculations.state` column became `'REJECTED'`, G19 = FAIL, and the DXF returned 409.
  - A lesson `integrity_failure` was recorded (lesson #6, source calc 12).
  - After a restart, a second replay matched: the detail view went back to REVIEW_REQUIRED, but the list still showed `(12, 'REJECTED')`.
  - The next identical request (job #13) received the advisory "This exact request produced a stored result that failed its integrity check".
  - Separately, if the replacement binary *does* start, the replay runs a different build, while the stored limitation text says "same machine with the same build".
- **Engine provenance** (BE-011):
  - `ENGINE_VERSION = "nf-core 0.1.0 / uem 0.1.0"` (`lib.rs:27`) has not changed since the first commit (`git log -S'ENGINE_VERSION: &str'` shows only `f1ba9c0`).
  - Commit `8978f62` changed the header-with-branch geometry ("outline and opening now share one frame").
  - Header jobs stored before and after that fix carry the same engine string. Nothing in the database identifies results produced by the defective build, and no mechanism flags or recalls them.

## 6. Concurrency and resource use

- **One mutex for everything** (BE-012).
  - Argon2id with library defaults runs while the lock is held: login takes 25 ms.
  - 30 concurrent failed logins raised `GET /api/health` from 0.0005 s to **0.534 s**.
  - Uploads encrypt and write up to 50 MiB while holding the lock (`main.rs:461-476`).
  - `correct` runs `job::run` while holding the lock (`main.rs:854-874`).
  - Calculation and replay do release the lock around the computation (`main.rs:611`, `main.rs:777-788`), which is good.
- **Throttling as denial of service** (BE-013). 40 failed logins with 40 different invented usernames within a minute produced, for the real admin with the correct password: `429 {"error":"too many failed sign-in attempts; try again in 15 minutes"}`. The global window is actually 60 s, but the message says 15 minutes. Any 5 failures lock a known account for 15 minutes. There is no client-address dimension, and no handling of `X-Forwarded-For` behind the documented Caddy proxy.
- **Time limits**: no request timeout layer, no child timeout, no limit on concurrent requests apart from systemd's `TasksMax=256` (`deploy/nf-server.service`).
- **Numeric magnitudes**: unbounded above (§2.2). Sampling sizes are fixed (n = 360 or 720), so compute time does not grow with the input; good.

## 7. Configuration, key handling, backup and recovery

- **Environment variables**: `NF_DATA_DIR` (default `./data`, relative to the working directory), `NF_LISTEN` (default `127.0.0.1:8787`), `NF_KEY_FILE` (default inside the data directory), `NF_COOKIE_SECURE` (only the exact value `1`). There is no config file, no validation, and no startup summary except the listen line.
- **Silent key regeneration** (BE-010). `load_or_create_key` creates a fresh key whenever `std::fs::read` fails (`crypto.rs:28-45`). Experiment: restart on a copy of the data with `NF_KEY_FILE` pointing to a non-existent path.
  - The server starts normally and writes a new `missing.key` (0600).
  - `GET /api/calculations/17/dxf/header` then returns `500 {"error":"decryption failed (wrong key or corrupted blob)"}`.
  - Every existing document and DXF becomes unreadable, and new blobs are sealed under the new key, so the store becomes mixed.
  - Triggers include a typo, an unmounted `/etc`, or a permission change.
- **Backup**: `deploy/README.md:19` says stop the service, copy the data directory and the key, start again. There is no online backup (SQLite backup API or `VACUUM INTO`), no `PRAGMA integrity_check` on start, no check that blobs referenced in the DB exist, no restore test and no retention policy (BE-024). `IMPLEMENTATION_STATUS.md:130` correctly lists Backup/restore as NOT STARTED.
- **First-run setup**: `install.sh` starts the service and opens Caddy and the firewall, and only then tells the operator to create the administrator. Until then, anyone who reaches `/api/setup` becomes admin (BE-025).

## 8. Logging and observability

There is no access log, no structured logging, no request ID, and no metrics or tracing. The only output is two `println!` lines at start (`main.rs:1112-1113`) and panic backtraces on stderr. `GET /api/health` returns `"status":"ok"` without checking that the database is writable, that the blob directory exists, or that the key decrypts anything (BE-021). Security-relevant events do go to the hash-chained audit table, which is good, but silently, and errors in it are dropped (BE-020).

## 9. Derived state that is stored once and then drifts (BE-009)

`calculations.state` is written at insert time and changed later only by a replay mismatch. The list, dashboard and project endpoints use it; the detail endpoint recomputes the state. Observed after one correction and one replay round:

```
GET /api/calculations   -> (3,'REVIEW_REQUIRED', superseded True)   detail of #3: REJECTED (G00 superseded)
                           (12,'REJECTED')                            detail of #12: REVIEW_REQUIRED
GET /api/dashboard      -> review_required 8 of 12, including superseded jobs and two refusals
```

This is the server-side cause of UI critic finding 2 (`docs/design/UI_CRITIC_REPORT_R1.md:70-72`).

## 10. Corrections (BE-004)

`correct` validates only that `new_value` is finite (`main.rs:850`). It then:
1. supersedes the old job whatever the new job's outcome (`main.rs:875`);
2. records a `corrected_value` lesson regardless (`main.rs:879`).

Experiment: correct valid job #3 to `header_radius: -5` with the reason "engineer typo test":

```
-> 200 {"new_calculation":10,"obligations":11,"open":11}
job 10: REVIEW_REQUIRED, outcome "out of range (header_radius): must be finite and > 0, got -5", no pattern
job 3:  REJECTED (superseded), GET .../3/dxf/header -> 409
lesson #5: header_radius wrong 150 -> right -5
next valid job with header_radius 150 -> advisory "header_radius = 150 repeats a value that was corrected
   in this project (job #3: 150 -> -5 ...)"
```

There is no endpoint to revoke a correction or a lesson. One typo replaces the only valid revision with a refusal, and teaches the system that the correct value is a known mistake.

## 11. IMPLEMENTATION_STATUS.md contradicted by code (BE-031)

| Claim | Location | Code |
|---|---|---|
| "Bend lines, marking layers, **inner profiles**, kerf: NOT STARTED" | `IMPLEMENTATION_STATUS.md:111` | Inner profiles exist: `Layer::CutInner` (`uem/src/lib.rs:43`) is written as layer `CUT_INNER` (`job.rs:120`). The header opening is a clockwise inner profile (`develop.rs:330-335`), checked by G11 (`job.rs:239-243`) and asserted in the DXF by test (`job.rs:509`). The same document's line 63 calls it INTEGRATED |
| "`APPROVED` cannot be reached" | `:17`, `:93` | Reachable through stored-data corruption (BE-002) |
| "a job with altered records becomes REJECTED" | `:33` | Only once someone runs a replay; reads never verify |
| Gate list "G01, G02, G05, …, G19" | `:83` | G00 (superseded) and G20 (known mistakes) are added by the server (`main.rs:700-715`) |
| "enter geometry parameters for four part families" | `:17` | Five request families (`job.rs:24-44`) |
| nf-core module doc: "dependency obligations … not implemented" | `nf-core/src/lib.rs:11-17` | Implemented in `deps.rs` and integrated (`main.rs:873-895`) |
| "Per-project membership enforced on every … summary endpoint" | `:29`, `security_status` | Enforced on access, but record counts are attributed to the wrong project (BE-014) |

## 12. What was not determined

- Behaviour under real power loss: torn blobs and partial correction state were simulated by editing files and rows, not produced by a crash.
- Whether `axum` 0.8's default server applies any header-read timeout (slow-client exposure). Not tested; the documented deployment puts Caddy in front.
- Load beyond 50k synthetic jobs, and memory use for very large projects. GET of one assembly is about 148 KB.
- Frontend handling of the inconsistent error formats (out of scope).
- The engineering correctness of the formulas themselves (out of scope; see the existing stress and verification documents).
