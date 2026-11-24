# Review A: architecture, reliability, security and maintainability of the two fix passes

Reviewer A (independent). 19 November 2026. Reviewed tree: `3b222bc` (fix passes `0c0a577` and `4c05456`). I did not change any product code, test or other document. Everything below was checked by reading the code and by running it. I built `nf-server` from this tree in my scratch directory (engine `nf-engine 0.3.0+src.51e755711d5e5d55`), ran copies on ports 8840 to 8852 with temporary data directories, and changed databases only in those directories. Mutation tests ran in a scratch copy of the repository.

Commands and scripts I used (all in the scratchpad `reviewA/`): `start.sh` and `lib.sh` (start a server and call it), `tamper.py` (edits stored results), `burst.py` (simultaneous sign-ins), `slow.py` (slow connections), `gen.py` (bulk jobs), `mutate.py` (reverts a fix and runs its test).

## 0. Verdict

**No CRITICAL finding remains.** I found no API path that marks a job APPROVED, and no API path that serves a DXF for a refused, rejected, superseded or stale-engine job. Every corruption of a stored result that I tried is rejected (§2.1).

The fix passes are real work, but several "Fixed" entries in the registers do not hold:

- **V-25 (audit-chain anchor): not effective.** Anyone with database write access can truncate the log and have the server re-sign the shortened chain. That access is exactly the V-25 threat model, and the attack needs no key (RA-003).
- **V-33 / BE-013 (sign-in throttling): partly fixed.**
  - The per-account limit can be beaten by sending guesses at the same moment: 90 guesses were checked instead of 5 (RA-002).
  - Any named account can still be locked by anyone with 5 requests every 15 minutes, and this is not documented as an accepted trade-off (RA-006).
- **S-3 (connection limits): partly fixed.** The new 512-connection cap makes denial of service cheaper. 512 connections that send their body slowly, from one client, make the server unreachable for 120 s, and the attack can be repeated (RA-001).
- **CI quality gate: cannot pass as written.** Its last step fails every time, because it checks a defect (D4) that is known to be open (RA-004).

None of these regressions is detected by the new tests.

I would not put a paying customer's engineering data on this build. Overall score: **5/10**.

| Area | Score | Reason |
|---|---|---|
| Architecture | 5 | One clearly defined state function (`evaluate`) and fail-closed reads are good. But one 3,636-line `main.rs` holds HTTP, authentication, state, storage, replay and the server loop; everything runs behind one `std::sync::Mutex<Connection>`; and the integrity checks use no key, so they catch accidents, not tampering. |
| Reliability | 6 | Transactions, atomic and verified blobs, refusal to start on bad key or damaged database, and correct disk-full handling all work under test. Slow-client exhaustion (RA-001), blocking work on the async runtime and audit writes outside transactions remain. |
| Security | 5 | The setup token, Host allow-list, upload authorised before the body, and rate-limited anonymous audit work. The throttle race (RA-002), named-account lockout (RA-006), anchor laundering (RA-003) and the DXF reference not tied to the job's record (RA-005) are open. |
| Scalability | 4 | One global lock, blocking work on async workers, and summaries that read every record. `/api/health` takes 0.25–0.33 s with 3,000 jobs and 4 clients reading summaries. Adequate for a handful of users, not more. |
| Maintainability | 5 | Error handling is consistent and there are no unwraps in product code. But it is a monolithic file, with repeated transaction, authorisation and audit boilerplate, writes inside GET handlers, SQL built with `format!`, and dead or doubled code (for example `new_value` parsed twice in `correct`). |
| Test quality | 7 | All 7 fixes I reverted were caught by their tests (§3). There are no concurrency or adversarial tests, so RA-001, RA-002, RA-003 and RA-005 pass the suite. The CI gate fails deterministically. |
| **Overall** | **5** | Fail-closed on corruption, but not yet robust against a hostile client or an operator mistake. |

## 1. Critical and high register items: re-test results

Each line gives the verdict and the evidence from this build.

| Item | Verdict | Evidence (this build) |
|---|---|---|
| BE-001 radii vs declared surface | **Partly** | `outside` 150/160 gives 422 `REFUSED`, `field: header_dev_radius`, and the DXF returns 404. `neutral` 150/150.5 and branch `outside` 100/110 are refused too. Still open: `branch_stub` without `surface_reference` and with `branch_dev_radius` 110 > `branch_radius` 100 (the original reproduction for that family) gives **200 `REVIEW_REQUIRED`, branch DXF 200**. `header_with_branch` with `surface_reference:"unknown"` and 150/160 gives 200 with **both DXFs 200** (G02 only INCONCLUSIVE). `outside` 150 / dev 1, a 149 mm wall, is accepted with DXF 200: there is no plausibility range. See RA-008. |
| BE-002 fail-open release state | **Fixed for corruption**; not proof against tampering | Every corruption I wrote with `tamper.py` gives `REJECTED` with an integrity reason and DXF **409**: `{}`, `not json`, gates removed, truncated, unknown gate state `"APPROVED"`, `gates:"PASS"`. Duplicate `gates` key: SQLite reads the first copy and the result stays consistent. Duplicate `patterns` key with the first copy empty: `REFUSED`, DXF 404 (but see RA-012). Not covered: a stored `dxf_sha256` swapped to another job's file is **served as REVIEW_REQUIRED with integrity []** (RA-005). |
| BE-003 refusals stored as review | **Fixed** | `header_radius:-150` and `branch_radius:400` on 150 give 422 `{state:"REFUSED", field, outcome, id}`. The dashboard counts these under `refused`, not `review_required`. |
| BE-004 refused correction | **Fixed** | Correcting `header_radius` to -5 gives 422 "Job #19 is still the current result". The job stays REVIEW_REQUIRED with DXF 200, and no lesson is recorded. A valid correction to 151 gives 200 and 11 obligations; the old job becomes REJECTED in detail and list, with DXF 409. |
| BE-005 transactions | **Fixed** (minor gaps: RA-013) | All 18 handler write paths use `BEGIN IMMEDIATE`. A Rust compile check makes it impossible to hold the lock across an `await`, and an early return rolls back. Mutation M3 (calculate's transaction made to commit when dropped) is caught by `be005_…`. The gaps are audit writes made outside a transaction (RA-013). |
| BE-006 blob atomicity | **Fixed** | Blob truncated to 100 bytes, then the same job re-run: the log says "failed its integrity check and was rewritten" and the size is restored. Truncated again, then GET DXF: 200, restored by recomputation with the same hash. |
| BE-007 panics on SQLite errors | **Fixed** (caveat: RA-011) | An outside connection held `BEGIN IMMEDIATE` for 8 s. `POST /api/projects` then answered **503** after 2.0 s with `Retry-After: 2` and a JSON body; health and list stayed 200. There is no `unwrap`/`expect` in server code; the only remaining one in engine code is `lib.rs:102` (JSON serialisation of a value). |
| BE-008 replay infrastructure failure | **Fixed** | With the executable deleted, replay gives 503 `{infrastructure:true}`, the job stays REVIEW_REQUIRED with G19 NOT_RUN, and the lesson count is unchanged. |
| BE-009 list vs detail | **Fixed** | Detail, list and dashboard agree after a correction and after tampering by another process. Mutation M6 (list reads the stored column) is caught. |
| BE-010 key regeneration | **Fixed** | Missing key: exit 2 "MISSING … No key was created". Foreign key: "fingerprint mismatch". Mode 644: refused. |
| BE-011 engine identity | **Fixed, with caveats** | Setting `$.engine` to the old identifier gives REJECTED, G21, DXF 409. But the check reads only that one string, which is tied to no record (RA-005). The fingerprint ignores dependencies and the compiler, yet changes on edits to comments (RA-007). |
| V-25 audit anchor | **Not effective** | The 4 newest audit rows deleted: verify reports `intact:false`, as claimed. Then `meta.audit_count` deleted and **one anonymous failed sign-in** sent: verify reports **`intact:true`, 25 entries** (RA-003). |
| V-33 global lockout | **Partly** | The global lockout is gone. The per-account limit can be raced: 90 simultaneous wrong passwords for one account gave **401 ×90** and 90 failure rows (RA-002). 5 wrong guesses lock any named user, including the administrator, for 15 min (RA-006). |
| V-34 audit flooding | **Fixed** | 150 sign-ins with 5,000-character names added 30 audit rows, names cut to 64 characters. |
| V-35 upload before authentication | **Fixed** | 16 parallel unauthenticated 45 MB uploads: 401 in under 5 ms each, peak memory (VmHWM) 49.7 MB, unchanged. |
| V-36 setup takeover / DNS rebinding | **Fixed** (low residue: RA-009) | No token or wrong token: 403. Correct token: 200, then 409. `Host: evil.example` gives 421. A request without a Host header is refused. Residue: 10 wrong tokens lock out the real installer for 10 minutes, and this can be repeated (RA-009). |
| S-2 damaged/lost database | **Fixed** | Truncated to 0 bytes, deleted, 4 KiB random page in the middle, marker and database both lost while files remain: all four exit 2 with a clear message. |
| S-3 no timeouts / connection cap | **Partly** | Slow request heads are cut after 15 s. But **520 connections with a slow body** (`POST /api/login`, `Content-Length: 1000`) make `/api/health` **time out after 40 s, and again 30 s later**, until the 120 s request timeout (RA-001). |
| S-4 disk full | **Fixed** (low residue: RA-010) | 3 MB tmpfs, 120 jobs: 5×200, 1×507, then **500** whenever the file store is full. No panics, no partial rows, audit chain intact, sign-in sessions stored (`/api/me` 200). Residue: a full file store answers 500 "internal error", not 507 (RA-010). |

## 2. New findings (worst first)

### RA-001 · High · The S-3 connection cap turns slow clients into a cheap, repeatable outage

`serve()` admits at most `NF_MAX_CONNECTIONS` (512) connections. A connection holds its slot until the request ends. A request body may take the whole `NF_REQUEST_TIMEOUT_SECS` (120 s) to arrive, and nothing enforces a minimum transfer rate.

**Reproduction:** `python3 slow.py 8840 520 body 6` opens 520 sockets, each sending `POST /api/login` headers with `Content-Length: 1000` and then one byte every 5 s. Results:
- `GET /api/health`: **timed out at 40.05 s**; repeated 30 s later: **timed out again**.
- Slow request heads only: health answered after 12.98 s.

The request needs no authentication. Caddy does not buffer request bodies by default, so the attack also works through the shipped proxy. Before the fix, the same attack needed about 20,000 sockets; it now needs 512.

**Fix:**
1. Add a minimum body rate or a short body-read timeout (for example 10 s for JSON routes).
2. Cap connections per client address.
3. In the shipped Caddyfile, set server `timeouts` (`read_body`, `read_header`) and request-body limits.
4. Add a test that opens more than the cap and asserts health still answers.

### RA-002 · High · The per-account and per-client sign-in limits can be raced

`login` checks `failures_of`/`failures_from` and releases the database lock. It then waits for one of 2 hash slots and runs Argon2, and records the failure only afterwards (`main.rs:727-781`). Every request that arrives before the first failure is committed passes the check.

**Reproduction:** `burst.py 8840 racer2 90` sends 90 wrong passwords for one account at the same moment. Result: `{401: 90}`, and 90 failure rows for `racer2` with 5 allowed. A sequential test of 60 gave 11×401. Repeated every 15 minutes, and from many addresses, guessing is bounded only by Argon2 throughput, not by the documented 5 per 15 minutes.

The same 2-slot hash semaphore also lets an anonymous client crowd out legitimate sign-ins.

**Fix:** reserve the attempt before hashing. Inside the same transaction as the check, insert a "pending" failure row, or increment a per-account in-flight counter. Remove the reservation only on success. Add a concurrency test.

### RA-003 · High · V-25 anchor laundering: truncation is erased by the next audited event

`App::audit` extends the chain from the newest *row* and takes the count from the plain `meta.audit_count` value (recounting when it is missing). It then MACs and stores a new anchor **without first checking the existing anchor**. Someone with database write access, which is the V-25 threat model, can:

1. delete the tail of the log;
2. delete or adjust `audit_count`;
3. trigger any audited event, for example one anonymous failed sign-in.

The server then signs the shortened chain itself.

**Reproduction:**
- Before: verify `intact True entries 28`.
- After deleting 4 rows: `intact False … anchor records entry #28 and 28 entries`.
- After one `POST /api/login` with a wrong password: **`intact True entries 25 anchor_problem None`**.

No key was needed. The test `v25_…` covers truncation but not this.

**Fix:**
- In `audit()`, verify the current anchor (MAC, head equal to the newest row's hash, `last_id`, count) before appending. On a mismatch, refuse to extend the chain and raise an alarm; never recount.
- Put the count inside the MAC'd anchor only, not in a separate unauthenticated key.
- Better: copy the anchor head to somewhere outside the database (syslog, a remote system, or a file that is append-only).

### RA-004 · High (process) · The CI quality gate cannot pass as written

Problems in `.github/workflows/quality-gate.yml`, checked against the repository:

1. **The last step fails deterministically.** `python3 validation/run_all.py --spawn target/release/nf-server --quick` against this build exits **1**: "FAIL (126 s) … EPS_NUM_EXCEEDED 2 DEFECT e.g. N-tangent-d-b90-g2e-06". Quick mode deliberately includes `N-tangent-d-b90-g2e-06`, which checks D4/BE-038, a defect that is still open. The committed `validation/results/latest.json` says `"verdict": "FAIL"`. So no change can merge, or the gate gets bypassed.
2. `python3 -m pip install -r validation/requirements.txt` uses the system Python on `ubuntu-24.04`. PEP 668 ("externally-managed-environment") may refuse this. I could not run Actions to confirm. Use `actions/setup-python` or a virtual environment.
3. **Browser tests are flaky.** The commit message of `4c05456` admits `tests/ui_smoke.js` "obligation: waived" failed 1 run in 4. The server is started with `&` and `sleep 2` instead of polling health.
4. **Steps that do pass locally:**
   - `cargo build --release --locked`, `cargo clippy --all-targets --locked -D warnings`, `cargo test --workspace` (98 tests);
   - `tests/e2e_smoke.sh` (89/89) and `tests/security_smoke.sh` (66/66), run in a scratch copy.
5. **Gaps and inefficiencies:**
   - `tests/security_r6.sh` (the Round 6 checks the registers cite) is not in the gate.
   - The scripts compile a separate debug binary.
   - `cargo install cargo-audit` is rebuilt on every run.
   - `push: ["**"]` plus `pull_request` runs every PR twice.

**Fix:** mark known-open defects as expected failures with an expiry date, or remove them from `--quick`. Use `setup-python`. Poll health instead of sleeping. Add `security_r6.sh`. Cache `cargo-audit`.

### RA-005 · Medium · The DXF served and the engine identity are not tied to the integrity-checked record

`evaluate` compares the stored gates and state with the gate-evaluation record, and the closure hash with a column. But `get_dxf` takes `patterns[].dxf_sha256` straight from `result_json`, and `engine_current` reads only `$.engine`. Neither is compared with the records, which do contain `pattern` and `build.dxf` records with the engine, or with the `calculations.engine` column.

**Reproduction:** `tamper.py … 16 swapdxf 7`. Then `GET /api/calculations/16/dxf/shell` returns 200 with filename `…calc16_shell_f9c2f454.dxf`, and the bytes are **identical to job 7's DXF** (r_dev 101 instead of 110). Detail: `REVIEW_REQUIRED`, `integrity: []`.

In the same way, editing one string turns a superseded-engine result into a current one. The record identities are plain SHA-256 values with no key, so someone with database write access can also rewrite the records consistently.

**Fix:**
- Before serving, check that `dxf_sha256` equals the hash in the job's `build.dxf`/`pattern` record, and that the result's engine equals that record's engine and the column.
- State plainly in BE-002 and V-25 that these checks detect corruption, not deliberate tampering. Detecting tampering would need a MAC keyed with the storage key over `closure_hash`, the engine and the DXF hashes.

### RA-006 · Medium · Anyone can lock out a named account; BE-013 is marked Fixed but this half is unchanged

BE-013's own text includes "5 failures lock any known account for 15 min". The fix kept exactly that.

**Reproduction:** 5 wrong guesses for `victim`, then the correct password gives **429**. One client can keep the administrator locked out indefinitely with 20 requests an hour, far below the per-client limit of 100 per 10 min.

Users behind one NAT, or every user when `NF_TRUSTED_PROXY` is missing, share the per-client bucket. During this review my own client reached it, and then the correct admin password got 429.

No document records this as an accepted trade-off.

**Fix:**
- Throttle per (account, client) pair, or back off progressively instead of blocking.
- Let a correct password from a client that recently signed in successfully pass.
- Record the residual risk in the register.

### RA-007 · Medium · The upgrade path rejects everything, offers no recompute, and the engine ID is both too sensitive and not sensitive enough

On the owner's live instance, the first start of this build re-evaluates every job. 3,000 jobs took 2.2 s, so 20,000 should take about 15 s. Every result becomes **REJECTED, G21 "recompute"**, with its DXF withheld; correction and replay of these jobs return 409. There is **no recompute endpoint**, so each job has to be re-entered by hand.

The fingerprint (`engine_fingerprint.rs`) hashes only the top-level `src/*.rs` of `uem`, `nf-dxf` and `nf-core`:
- Any edit, even to a comment, invalidates every stored result again.
- Changes to `Cargo.lock` dependencies (for example `serde_json` float formatting), the `rustc` version or build flags can change results **without** changing the identifier.

Operator obligations on upgrade are documented in `deploy/README.md` but easy to miss if only the binary is replaced:
- Missing `NF_ALLOWED_HOSTS` means **421 for every public request**.
- Missing `NF_TRUSTED_PROXY` means one shared throttle bucket.

Running `install.sh` again rewrites the unit file and handles both.

**Fix:**
- Add an administrator "recompute" action that creates a new job from the stored request and links it as the successor.
- Hash a normalised form of the engine sources plus `Cargo.lock` and the `rustc` version.
- Have the server refuse to start when it is bound to a non-loopback address and `NF_ALLOWED_HOSTS` is unset.
- Write an upgrade checklist.

### RA-008 · Medium · BE-001 remains open for undeclared surfaces and implausible walls

The original `branch_stub` reproduction still gives a downloadable DXF. `"unknown"` lets any radius combination through with only an INCONCLUSIVE gate. No upper bound on thickness relative to radius exists: `outside` 150 / dev 1 is accepted.

The register explains this as "the specification gives none". Either way, the defect as filed (no plate with an impossible wall may be offered for download) is not closed.

**Fix:** for `unknown`, withhold the DXF until the surface is declared, or make G02 FAIL. Add an engineering plausibility range that an engineer can override with a recorded attestation.

### RA-009 · Low · Anyone can block first-run setup indefinitely

Failed setup attempts are counted under one global key (`"setup:"`): 10 per 10 min. 10 wrong tokens make the real installer's correct token get **429**. `install.sh` creates the administrator before Caddy is configured only when `NF_ADMIN_USER` is given, so a public instance can be held in setup for as long as the attacker likes. `NF_SETUP_TOKEN` accepts 16 identical characters.

**Fix:** count setup failures per client, and require an entropy or minimum-length policy on the token.

### RA-010 · Low · A full file store answers 500 "internal error", not 507

`storage::store_blob` returns a `String`, so `ENOSPC` becomes `ApiError::Internal`. In the S-4 run, 114 of 120 requests got 500 and only the database path got 507. Clients and operators cannot tell a full disk from a bug.

**Fix:** use a typed blob error and map `ENOSPC` to 507.

### RA-011 · Low · Any log write can panic

`eprintln!` panics when stderr cannot be written. With the server's stderr on the full tmpfs, every failing calculation became an **empty reply** with no log line: 92 of 120. Under systemd and journald this is unlikely but possible (for example while journald restarts). The claim "no request-reachable panic" is therefore not strictly true, and there is no panic-catching layer (BE-007 says so).

**Fix:** `writeln!(std::io::stderr(), …)` with errors ignored, or a logging crate; add `CatchPanicLayer`.

### RA-012 · Low · Duplicate JSON keys are read differently by SQLite and serde

`evaluate` reads through SQLite's `json_extract`, which takes the first copy of a duplicated key. `get_calc` and `get_dxf` use serde, which takes the last.

**Reproduction:** with `patterns` duplicated and the first copy empty, the detail shows `state: REFUSED` together with a full pattern (`tamper.py … duppatterns_first_empty`). The DXF stays withheld, but a refused job is displayed with geometry.

**Fix:** reject stored JSON with duplicate keys (`integrity`), or derive everything from one parser.

### RA-013 · Low · Some audit writes are outside a transaction

`audit()` makes 3 statements (insert row, `audit_count`, anchor). Some calls run on a plain connection, in autocommit:
- `audit_anon` in the throttled sign-in path (`main.rs:742`);
- `project_guard`/`calc_guard` access-denied entries in GET handlers.

A crash between those statements leaves the anchor behind the log, and verify then reports a false "newest entries removed". When the second `project_guard` inside a write transaction refuses access, its audit entry is rolled back and lost.

**Fix:** always audit inside a transaction, and commit denial entries separately.

### RA-014 · Low · Restoring a pre-upgrade database backup is refused

After the upgrade writes `.installation-marker`, restoring an older database that has no `installation_id` stops start-up with "installation marker present, but the database carries no installation id". I reproduced this with a copy that had its `installation_id` removed.

The message tells the operator to "restore from a backup", which is what they just did. The marker step is not documented.

**Fix:** document the step, or accept a database with no ID when its key fingerprint matches.

### RA-015 · Medium (known, BE-012 open) · Scalability is limited by one mutex on async workers

The `std::sync::Mutex` is taken inside async handlers. That blocks tokio workers (4 on this machine), so request timeouts cannot fire while threads wait. Several heavy operations run on the runtime and some under the lock:
- `job::run` (`calculate`, `correct`, and DXF restore under the lock);
- encryption of 50 MB and fsync (`upload`, under the lock);
- 200 autocommit `UPDATE`s in the first list after a restart (`calc_rows` writes from GET).

Measured with 3,000 jobs (207 MB):

| Endpoint | Time |
|---|---|
| `/api/calculations` | 112 ms |
| `/api/amoeba/summary` (reads every activity record) | 78 ms |
| `/api/audit/verify` | 182 ms |
| `/api/health` with 4 clients reading summaries | 0.25–0.33 s |
| `/api/health` during 3 parallel 45 MB uploads | 0.39 s |

These grow linearly with data.

**Fix:**
- A connection pool with WAL readers, plus one writer task.
- `spawn_blocking` for engine work and encryption.
- A materialised summary for the amoeba view.

### RA-016 · Low · Address handling details

- Per-client throttling keys on the full IPv6 address, so one /64 gives 2^64 "clients". Aggregate to /64.
- `NF_ALLOWED_HOSTS=2001:db8::1` (an IPv6 address without brackets) is stored as `2001:db8:` by `host_name` and never matches (fails closed, but confuses the operator). Reject it or normalise it.
- Any local process can reach 127.0.0.1:8787 and set `X-Forwarded-For` freely, because 127.0.0.1 is trusted. This is acceptable on a single-tenant machine; document it.

### RA-017 · Low · A failing gate counts only if storage says it is critical

`release_state` ignores `critical:false` gates, and `with_required_gates` accepts a required gate that is present but non-critical. The engine never emits `critical:false`, so this matters only with tampering. Still, the server should force `critical=true` for required gates.

## 3. Code quality of the changes

**Size and structure.** `main.rs` grew from 1,115 to 3,636 lines and `tests.rs` from 0 to 2,557. The single file mixes transport (`serve`, `security_layer`), authentication, audit, state evaluation, replay process control, DXF serving and start-up. The rule "one state function" is well kept: `evaluate` is the only producer of state, and `refresh_state`/`calc_rows` reuse it.

**Repetition.**
- Transaction-plus-audit boilerplate appears 18 times.
- The audit-on-denied block is written twice (`project_guard`, `calc_guard`).
- The access checks in `acknowledge_advisory` and `resolve_obligation` are copies of each other.
- `correct` parses `new_value` twice, with a `let _ = new_value;` and a `NaN` fallback.
- `CORE_EXPR` duplicates the core derivation in SQL and in Rust (`core_row`), with a positional 8-element array that is easy to break.

**Errors.** Errors are consistent: one `ApiError`, JSON bodies, 503 or 507 for the database. The exceptions are the file-store errors that go through `String` (RA-010) and errors that are only logged in `calc_rows`.

**Panics.**
- Server code has no `unwrap`/`expect`.
- Indexing is guarded: `&sha[..8]` after `is_sha256_hex`, `parts[i]` after a length filter, `&sealed[..12]` after a length check.
- Arithmetic: `bytes.len() + chunk.len()` is bounded.
- The remaining path is a panic from `eprintln!` (RA-011).

**Blocking and lock scope.** Covered under RA-015. Argon2 and the replay child are correctly moved off the runtime and behind semaphores of 2.

**Transactions.** There is no `await` inside a transaction; the compiler would reject one. Early returns roll back, and the multi-statement writes I read are atomic, except those in RA-013. `calc_rows` writes from GET requests in autocommit, which is acceptable but surprising.

**Atomic files.** The blob store (temporary file, fsync, rename, fsync of the directory), the key and the marker files are correct. The setup-token file is not fsynced, which is harmless.

**Mutation tests.** I ran these in a scratch copy, each reverting or breaking one fix, then running its test:

| Mutation | Test | Result |
|---|---|---|
| M1 integrity reasons cleared in `evaluate` | `be002_corrupt…` | **fails** (caught) |
| M2 refused correction applied anyway | `be004_…` | **fails** |
| M3 `calculate` transaction commits on drop | `be005_…` | **fails** |
| M4 global 40/min sign-in limit re-introduced | `v33_failures…` | **fails** |
| M5 every engine treated as current | `be011_…` (both) | **fail** |
| M6 list reads the stored state column | `be009_…` | **fails** |
| M7 anchor count/head comparison removed | `v25_…` | **fails** |

The tests are meaningful for the fault each was written for. What is missing is adversarial coverage: no concurrency, no slow clients, no laundering, no tampering with cross-references. The `be007_no_unwrap…` test is a grep, which is useful but shallow.

## 4. Operations as they really are

**Backups.** `nf-backup.sh` uses SQLite's online `.backup`, runs `integrity_check`, archives and records a checksum. The key goes into the same archive on the same host, which is documented. There is no offsite copy and no restore test. A rolled-back database restore is not detectable (documented). See RA-014 for the marker problem.

**Migrations.** These are idempotent `ADD COLUMN`/`CREATE INDEX IF NOT EXISTS` statements, not run in one transaction, and a schema version is now recorded. Nothing destructive: stored results are not rewritten, and `engine` is filled in from the stored result. The surprising effects are the global REJECTED state and the operator obligations (RA-007).

**Deploy.** The unit file sets `NF_ALLOWED_HOSTS`, `NF_TRUSTED_PROXY=127.0.0.1`, `UMask=0077` and strong sandboxing. `MemoryMax=1G` holds 4 uploads × (50 MB + sealed copy). The new sandbox directives have never been run on a server (the README says so).

## 5. Cleanup note

Running the validation suite created untracked `validation/**/__pycache__/` directories in the shared tree (13:34). They are caches; I left them in place in case another agent's run is using them. Delete them if they are not wanted.

## 6. Status after the builder pass (22 November 2026, branch wt/server-sec)

Evidence: `nf-sandbox cargo test -p nf-server` gives 103 unit tests and 8 integration tests passing (baseline at f403297: 76 and 8; the 7 PDF-converter tests need `cargo build -p nf-pdf --bins` first). `cargo clippy -p nf-server --all-targets -- -D warnings` and `cargo fmt -p nf-server -- --check` are clean. The tests named below were run red against the code before their fix, except where noted: `ra007_*`, `ra009_a_setup_token_*`, `ra016_*`, `ra010_*`, `ra006_known_addresses_*`, `ra003_an_anchored_log_*` and the `ra003` startup/restart checks were written after their fixes. The two `behind_an_unconfigured_local_proxy` tests were checked red by disabling the rule (`unattributed_peer` forced to false).

| Item | Status | Where (crates/nf-server/src) | Test(s) | Behaviour old → new |
|---|---|---|---|---|
| RA-003 | partly | `main.rs` `audit` 391, `checked_chain` 437; `storage.rs` `TRIGGERS` 57; `build_app_with` | `ra003_a_truncated_audit_log_is_not_re_signed_by_the_next_event`, `ra003_audit_and_records_cannot_be_updated_or_deleted_through_the_schema`, `ra003_a_log_with_entries_but_no_anchor_stops_start_up`, `ra003_an_anchored_log_from_an_earlier_start_passes_without_the_flag` | Truncated log: next event re-signed it → refused with 503 `audit_broken`; verify intact:false. Rows updated/deleted → refused by triggers. Log with entries and no anchor: re-anchored silently → refuses start unless `NF_ACCEPT_UNANCHORED_AUDIT=1`. Left: key in the data folder unless `NF_KEY_FILE` is set (RC-07 key part, task 2.5). A writer that drops the triggers and holds the key can still re-sign; closing that needs an anchor outside the database. |
| RA-002 (+RC-04) | closed | `login` 960, `change_password` 1142 | `ra002_concurrent_wrong_sign_ins_…`, `ra002_concurrent_wrong_current_passwords_…` | 30 concurrent wrong sign-ins: 30×401 → 5×401, 25×429. The attempt is reserved (failure row) in the throttle transaction before hashing. Password-change failures store the client address. |
| RA-006 | closed for the stated threat, residual below (A11) | `login_throttle` 790; `seed_known_clients`; `admitted` column in `login_failures` | `ra006_a_guesser_cannot_lock_the_account_for_another_client`, `ra006_once_an_account_is_under_attack_…`, `ra006_behind_an_unconfigured_local_proxy_…`, `ra006_refused_attempts_are_recorded`, `ra006_known_addresses_are_seeded_…` | Old: one client's 5 failures locked the account for everyone. New: the pair (account, address) is locked alone; per-address rules skipped for unattributed clients (below); refused attempts are recorded (`admitted = 0`) and count, but do not open a slot; known addresses come from the audit `login` history once at upgrade. |
| RA-001 | partly | `BODY_IDLE` const (named, with mobile note), `guard_body`, `serve_one`, `InFlight` | `ra001_a_request_body_that_stalls_…`, `ra001_one_client_cannot_hold_more_requests_…`, `ra001_behind_an_unconfigured_local_proxy_…` | A stalled body held a request for 120 s → no chunk for 10 s gives 408. One client held any number of open requests → 32 per address (IPv6 by /64). Left: a per-TCP-connection cap, because behind Caddy every connection comes from 127.0.0.1. |
| RA-007 | partly | `recompute` 3397 (`/api/calculations/{id}/recompute`), `listen_needs_allowed_hosts` 4694 | `ra007_a_result_of_a_superseded_engine_…`, `ra007_a_non_loopback_listener_needs_the_allowed_host_list` | Recompute of a superseded result from its verified stored request; the old one stays. Non-loopback listen without a non-empty `NF_ALLOWED_HOSTS` → refused at start. Left: engine fingerprint (crates/nf-core, lane F); one-click button (web, lanes J/K). |
| RA-009 | closed | `setup` 852, `distinct_chars` | `ra009_setup_failures_are_counted_per_client`, `ra009_a_setup_token_of_one_repeated_character_is_refused` | Global `setup:` counter → per address. Token: 16+ characters and 8+ different ones. Unattributed clients share one counter (baseline behaviour). |
| RA-010 | closed (classifier test only) | `storage.rs` `StoreError`, `is_out_of_space`; `App::store_blob` | `ra010_a_full_file_store_is_classified_as_out_of_space` | Full file store 500 → 507 `storage_full`. No end-to-end full-disk test. |
| RA-011 (+RC-33) | closed | `elog!` (top of `main.rs`); `serve_one` | `ra011_a_panicking_handler_answers_500_…` | Log writes cannot panic; a panicking handler answers 500, the server keeps serving. |
| RA-012 | closed | `result_repeats_a_key`, `json_keys_are_unique`, `evaluate` | `ra012_a_stored_result_with_a_duplicate_key_is_rejected` | Duplicate key → REJECTED, DXF 409. |
| RA-013 | partly | `audit` 391 (SAVEPOINT) | `ra013_a_failed_audit_write_leaves_no_entry_behind` | One savepoint per audit entry. Left: denial entries written inside a write handler's transaction still roll back with it. |
| RA-014 | closed | `open_storage` (`has_key_fingerprint`) | `ra014_a_restored_pre_upgrade_database_…` | Restored database without installation id is accepted when its key fingerprint is present; `load_key` still checks the key. |
| RA-016 | closed | `client_key` 4055 (IPv4-mapped → IPv4; IPv6 → /64), `host_name` 4024, `allowed_host_entry` 4037 | `ra016_ipv6_clients_share_their_prefix_…`, `ra016_ipv4_mapped_clients_are_keyed_as_ipv4`, `ra016_an_unbracketed_ipv6_allowed_host_…` | Unbracketed NF_ALLOWED_HOSTS IPv6 refused with its bracketed form. |
| CLI 3.2 reason bytes | closed | acknowledge, correct, resolve | `cli32_a_reason_is_counted_in_characters_not_bytes` | `len() < 5` → `chars().count() < 5`. |
| RC-20 | partly | `storage.rs` `TRIGGERS` | as RA-003 | Triggers stop code and accidental edits; a file writer is detected by the anchor. |
| RC-21, RC-31 | left | — | — | Need `project_records` and schema v4 (task 3.4). |

### Review round 3 (rebased onto 05d81cf)

Branch rebased onto origin `dev/nexus-fabric-strategy-8guhxz` (05d81cf, engine 0.4.4). Added: `ra016_an_ipv4_client_on_a_mapped_loopback_listener_is_the_same_client` (`::ffff:127.0.0.1` is the loopback proxy, mapped before every check), `ra006_a_remote_peer_with_a_forged_forwarded_address_is_still_limited_by_its_real_address` (a remote peer's forged X-Forwarded-For does not reach the unattributed branch; this one is a regression guard and was not red before). `v34_…` now pins its anonymous-audit minute (`App::anon_minute`, test-only), so the per-minute limit no longer depends on the wall clock. The v34 assertion is unchanged.

### Unattributed clients (finding of the adversarial review)

The live server runs behind Caddy on loopback. Without `NF_TRUSTED_PROXY=127.0.0.1` every request arrives from 127.0.0.1, and the earlier build keyed them all on one address: one attacker then locked every account and capped every client. Now a request is **unattributed** when its peer is loopback, is not a configured trusted proxy, and carries `X-Forwarded-For` (which Caddy always sends). For unattributed requests the per-address rules are skipped: pair lock, per-address failure cap, per-address open-request cap, and known addresses. Only the account gate applies. Tests: `ra001_behind_an_unconfigured_local_proxy_one_client_cannot_deny_others`, `ra006_behind_an_unconfigured_local_proxy_one_client_cannot_lock_another_account`. The V-33 test is unchanged: `X-Forwarded-For` is still never believed from a peer that is not configured.

Operational consequence: behind an UNCONFIGURED proxy (no `NF_TRUSTED_PROXY`) the owner can be STARVED, not merely delayed. Every request shares one key, so any attacker who reaches the account gate takes the one slot every unattributed client shares, and the owner cannot sign in while the attack lasts. `NF_TRUSTED_PROXY=127.0.0.1` is REQUIRED for every per-address rule to function: the open-request cap (`InFlight`), the pair lock, the per-address failure cap and the known-client exemption all treat unconfigured traffic as one client. Step 2 of the live-upgrade draft in `deploy/README.md` is therefore mandatory, not optional.

### Changed expectations (none of the old assertions was weakened)

- `v25_removal_of_the_newest_audit_entries_is_detected`, `rc01_a_damaged_or_missing_closure_record_rejects_the_job`: the test drops the append-only triggers first (`attacker_drops_triggers`), to play the file writer. Assertions unchanged.
- `be011_upgrade_migration_flags_old_results_without_rewriting_them`: opens the pre-anchor log with `build_app_with(…, true)` (the explicit acceptance an operator gives). Previously `build_app`.
- `ra006_once_an_account_is_under_attack_only_known_clients_get_through_at_full_speed`: old expectation: a new address is refused (429) while the account is attacked. New: the slot is per (account, address), so a new address gets its first attempt (401 for a wrong password) and its second within 10 s is refused (429). Reason: a shared slot let one rotating attacker starve the owner's new network.
- `ra007_a_non_loopback_listener_needs_the_allowed_host_list`: the function now takes the variable's value (`Option<&str>`), not a boolean. The same cases are asserted, plus empty and blank lists are refused.

### A11 honest residual (RA-006)

- An attacker who controls many addresses (for example many /64 prefixes) can make about 5 guesses per 15 minutes per address on one account. Total guesses on one account are therefore bounded by the number of addresses the attacker controls, not by a constant. A shared slot across addresses would bound the total but let one rotating attacker starve the owner's new network; this build chose not to do that.
- Known addresses are seeded from the audit `login` history, once per database. An owner who first signs in from a new network during an attack is slowed down (one attempt per 10 s per address) until the slot is free; they are not locked out.
- Refused guesses are counted (`admitted = 0`), so a refusal does not make guessing free.
- Behind a proxy that is not configured (unattributed), only the account gate applies, and the owner can be STARVED (see the operational consequence above), because the one slot every unattributed client shares is taken by any attacker. This is a configuration failure, and the required setting is `NF_TRUSTED_PROXY=127.0.0.1`. With that setting the residual above applies instead.

### Note for the operator: Caddy timeouts (not deployed by this pass)

`deploy/Caddyfile` has no timeouts. The server's own bound is idle, not whole-body: a request body may take as long as it keeps sending chunks, and any chunk gap longer than 10 s ends the request with 408. A Caddy `read_body` is a whole-body deadline, not an idle one. Do not set it to the server's 10 s idle figure: that would cut large PDF uploads over slow mobile links. Choose Caddy's whole-body value with that in mind; `read_header` 15 s matches `NF_HEADER_TIMEOUT_SECS`. `deploy/` was not changed by this pass.

### Strings for users

The new responses carry `error` (English) and `error_ja` (Japanese): `audit_broken` (503), `storage_full` (507), the pair, gate and in-flight throttles (429), recompute-not-needed (409). `web/i18n.js` should show `error_ja` in Japanese; no key was added to it.
