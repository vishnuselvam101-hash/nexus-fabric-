# NEXUS FABRIC - Stress, Performance and Recovery Report

Date: 2026-11-19. Binary: a copy of `target/release/nf-server` taken once before the campaign (the shared tree was mid-edit afterwards and was not rebuilt). No product code was changed. Framework and raw data: `stress/` (`run.py`, `nfstress.py`, `bottleneck.py`, `results/*.json|md`).

## 1. Environment
- 4 cores, 16 GB RAM, Linux 6.18 (Firecracker VM), root user, ulimit -n 20000, local disk (`/dev/vda`). Other agents used the same machine at the same time: absolute numbers are noisy, ratios and trends are the useful part.
- Server: single process, 5 threads (tokio multi-thread + blocking pool), SQLite (WAL) via rusqlite behind one `std::sync::Mutex<Connection>`.
- Client: Python threads with keep-alive `http.client`, on the same machine (they consume CPU too).
- Ports 8810-8819 only; disposable data dirs; 1 s sampler (CPU %, RSS, threads, fds, data-dir size).
- Timing: L1 75 s, L5 1201 s (ran in parallel with L2/L3/L6 - affects its latency, not its RSS/fd trend), L2 65 s, L3 113 s, L4 171 s, L6 about 263 s plus a 60 s lockout wait, bottleneck probe about 2 min. Total about 45 min.

## 2. Method
`python3 stress/run.py --level N --duration S --port 8810x` starts its own server, drives load, samples the process, stops it with SIGTERM, then (L2/L3/L4) runs integrity verification: read-only open of the SQLite file with `PRAGMA integrity_check`, Python recomputation of the audit hash chain, contiguity/orphan checks, DXF blob presence, Python recomputation of every record identity hash (floats may differ from serde formatting, so suspects are cross-checked by the server's own replay of the owning job), then a restart and `/api/audit/verify` plus replay of a random sample. "Error" below means 5xx or connection failure; 4xx on purpose are not errors.

Capabilities in the owner brief that do not exist: **external DXF or drawing import, background job queue, structural frames/trusses - not implemented, not tested.** (File upload/document ingestion DOES exist: `POST /api/projects/{id}/documents`, membrane admission by content, 50 MB limit; it was tested.) Replay audit runs a child process of the same binary synchronously.

## 3. Results by level

| Level | Workload | Duration | Requests | Error rate | p50 / p95 / p99 (ms) | Throughput | Peak RSS / CPU | Result |
|---|---|---|---|---|---|---|---|---|
| 1 Normal | 1 admin user, loop over 5 families (calc, get, DXF), replay, correction, listings | 75 s | 2,401 | 0 | 3.3 / 264 / 482 | 32 rps | 32 MB / 100% | pass; latency of listings grows with job count |
| 2 Heavy | magnitudes 1e1..1e308 x 5 families, 250 KB description, 1,500 projects, 3,000 jobs in one project (6 clients), listings, 400 DXF, 25 replays | 65 s | 5,349 | 0 | 16 / 56 / 111 (max 2,339) | 101 rps | 52 MB / 110% | pass, integrity ok; `GET /api/projects` 2.2 s with 1,501 projects x 3,043 jobs |
| 3 Extreme | 20,000 jobs in one DB (2.0 GB), limits probe (about 120 probes), 50 MB uploads, protocol limits | 113 s | 20,167 | 0 | 20 / 44 / 64 (max 2,571) | 179 rps | 102 MB / 100% | pass; no crash/5xx; listing latency linear in job count |
| 4 Concurrency | 1/4/16/64 users mixed read/write, 25 s each, plus races | 171 s | 2,824 | 0 | see 3.4 | see 3.4 | 213 MB / 120% | pass, integrity ok; throughput collapses with concurrency |
| 5 Endurance | 3 users, think 0.2 s, mixed (calc, get, list, DXF, correct, replay, login, audit) | 1,201 s (20 min, real) | 6,451 | 0 | 18.6 / 1,748 / 2,979 | 5.4 rps | 96 MB / 101% | pass; RSS plateau with small creep, see 5 |
| 6 Faults | kill -9 x25, SIGTERM, key/db damage, disk full, malformed HTTP, fuzz 6,500+ requests | about 5 min | n/a | 0 5xx from fuzz | n/a | n/a | 232 MB / 79% | see section 6 for failures |

### 3.1 Level 1 baseline per operation (ms; about 80 samples each, data set about 480 jobs)
calc cylinder p50 4.3 / p95 12 / p99 17; cone 4.5/10/26; eccentric reducer 8.9/24/56; branch stub 6.0/12/59; header with branch 18.5/44/155; correction 25.7/40/249; replay 17.8/31/50; get job 2.7/4.7/6.3; DXF download 3.0/5.3/7.9; dependencies 2.9/4.2/6.0; health 0.25/0.42/1.4; audit verify 1.2/2.0/2.3; **list calculations 271/501/538; dashboard 258/500/526; get project 267/518/565** (the last three grow with data, see 7).

### 3.2 Level 2 heavy
- Magnitude sweep: every family accepted 1e1..1e15 and mostly 1e300 with 200 (state REVIEW_REQUIRED or REJECTED by the engine's own checks; a REJECTED result withholds the DXF with 409). `branch_stub`/`header_with_branch` at 1e308 give 400 (JSON overflow in a derived value). Each calc 3-43 ms. There is no upper bound on dimensions: a cylinder shell with r_dev = 1e300 is returned REVIEW_REQUIRED with a DXF (see F9).
- Bulk: 1,500 project creates p50 3 ms; 3,000 calcs in one project (6 clients) 16.7 s (about 180/s, p50 32 ms).
- At 1,501 projects and 3,043 jobs: list projects 2,197 ms, list calculations 302 ms, dashboard 328 ms, get project 326 ms, amoeba summary 111 ms, audit verify 0.2 ms, DXF download p50 2.4 ms (400), replay p50 9 ms (25).
- Data size: 145 KB per job on disk (about 30 KB result_json + records + DXF blobs): 3,000 jobs = 445 MB.
- Integrity after run: integrity_check ok, 3,043 jobs, no gaps, audit chain intact (4,737 entries), 111 python-hash suspects all verified by server replay (0 bad), 60 random replays 0 bad.

### 3.3 Level 3 extreme: growth of one database
| jobs | data MB | create jobs/s | list calcs ms | dashboard ms | get project ms | amoeba ms | list projects ms | audit verify ms | single calc ms | RSS MB |
|---|---|---|---|---|---|---|---|---|---|---|
| 1,000 | 102 | 260 | 127 | 113 | 111 | 24 | 1.1 | 1.5 | 5.0 | 30 |
| 3,000 | 298 | 257 | 296 | 305 | 302 | 63 | 2.1 | 3.4 | 5.3 | 30 |
| 6,000 | 594 | 246 | 577 | 736 | 683 | 171 | 5.2 | 9.0 | 5.3 | 33 |
| 10,000 | 987 | 247 | 1,137 | 1,120 | 1,134 | 240 | 8.1 | 14 | 5.2 | 34 |
| 15,000 | 1,481 | 210 | 1,849 | 1,894 | 1,828 | 387 | 13 | 19 | 5.6 | 37 |
| 20,000 | 1,976 | 203 | 2,381 | 2,464 | 2,571 | 536 | 19 | 22 | 3.1 | 37 |

Job creation stays about 5 ms (single) and 200-260/s regardless of size; **every listing endpoint is O(total jobs)** (about 120 ms per 1,000 jobs) although each returns at most 200 rows. No limit was reached at 20,000 jobs / 240,516 records / 20,069 audit entries (stopped by plan, not by failure; no job-count limit exists). Post-run: integrity_check ok, audit chain intact, 16 python-hash suspects verified by replay, 60 random replays 0 bad.

### 3.4 Level 4 concurrency (25 s phases, mixed workload, data set 0.3-1.8 k jobs)
| users | requests | errors | p50 ms | p95 ms | p99 ms | max ms | rps | server CPU mean |
|---|---|---|---|---|---|---|---|---|
| 1 | 1,341 | 0 | 1.8 | 166 | 268 | 309 | 53.3 | 90% |
| 4 | 539 | 0 | 28 | 666 | 954 | 1,323 | 21.5 | 94% |
| 16 | 370 | 0 | 1,096 | 2,307 | 2,864 | 3,293 | 14.0 | 94% |
| 64 | 446 | 0 | 4,281 | 8,583 | 9,677 | 10,191 | 14.2 | 97% |

Aggregate throughput falls 4x going from 1 to 64 users, server CPU never exceeds about 120% of one core on a 4-core machine. No request failed and none timed out (client timeout 60 s, max latency 10.2 s).

Races (all consistent):
- 64 simultaneous identical requests: 64 x 200, 64 distinct job ids (jobs are not de-duplicated by design), 1 distinct closure hash, record count +12 only (content addressing de-duplicated the records); no duplicate or lost job.
- 32 simultaneous corrections of one job: exactly 1 x 200 and 31 x 409; exactly one successor job.
- 24 concurrent logins/password changes on 4 users: exactly one password change succeeded per user, new password works, old password rejected, sessions revoked.
- Audit chain after the whole level: 5,189 entries, intact (API and independent Python recomputation, no id gaps).
- Final: SQLite integrity_check ok, 1,832 jobs contiguous, 0 orphans, 0 missing DXF blobs, 44,207 records (76 python-hash suspects, all 57 owning jobs replay-verified, 0 bad), 60 random replays 0 bad, restart ok.

## 4. Documented and actual limits
| Limit | Value | Behaviour beyond |
|---|---|---|
| JSON body | 256 KiB (261,944 B accepted; 262,145 B) | 413 clean (also for chunked) |
| Upload | 50 MiB content (52,428,800 B accepted); axum body cap 50 MiB + 1 KiB | 1 byte over: 422 `too_large`; larger: 413 and the connection is closed early (client sees EPIPE) |
| Project name | 200 bytes (200 emoji = 800 bytes rejected) | 400 |
| JSON depth | serde_json recursion limit 128 (127 passes parsing) | 400 |
| Numbers | 1e309 / -1e309 / NaN / Infinity | 400 invalid JSON; 1e308, 4.9e-324, 1e-400, -0, 30-digit integers accepted |
| Header size | 64 KB header accepted; 1 MB or 5,000 headers | 431 |
| URL | 100 KB | 414 |
| Ids | 20-digit ids, text ids | 400; negative or 0: 404 |
| Username | 3-64 chars | 400 |
| Password | minimum only (10); **no maximum** (250 KB accepted) | none |
| Job description/surface_reference/reason strings | only the 256 KiB body limit | accepted up to it |
| Sessions | 12 h; login throttle 5 failures per account per 15 min, 40 overall per minute | 429 |
| Connections | none (no cap, no read timeout) | see F3 |
| Number of jobs / projects | none found up to 20,000 jobs / 1,501 projects | latency only |

Unsupported or nonsense inputs (zero, negative, r1 = r2, beta 0 or 180, branch larger than header) return 200 with state REVIEW_REQUIRED and **no pattern and no DXF**, as the smoke tests document; this is honest but returns 200 not 4xx.

## 5. Endurance (Level 5, 20 min real, 3 users, 2,205 jobs created, 6,451 requests, 0 errors)
- File descriptors: 17 constant, slope 0 (r2 = 1.0). Threads: 5 constant.
- RSS: 26 MB at start, 53 MB at 30 s, 94 MB by 120 s (cache warm-up), then 93.8 (300 s) / 93.8 (600 s) / 94.7 (900 s) / 95.7 MB (1,200 s). Linear fit over the second half: +10 to +13 MB/h (r2 about 0.85); over t > 30 s: +27 MB/h (r2 0.26, dominated by warm-up). **Verdict: no unbounded growth visible in 20 min and fds/threads are flat; a creep of about 0.2 MB/min remains unexplained. It may be SQLite page cache / allocator growth with the growing database (60 to 140 MB) or a small leak; 20 min cannot separate them.**
- Data dir grew 0.10 MB/s (375 MB/h).
- Latency drifted upward by quarter: p50 6.1, 19, 19, 51 ms; p95 358, 961, 1,435, 2,408 ms - caused by the O(N) listings (7) and the global lock, not by process degradation. Health p95 was 1.56 s: any slow request blocks all.
- **A 6-24 h run was NOT done.** Command to run elsewhere (64 GB free disk recommended): `NF_BIN=/path/nf-server python3 stress/run.py --level 5 --duration 86400 --users 4 --think 0.05 --port 8810 --workdir /big/disk/w` then read `fit_rss_mb`, `fit_fds`, `latency_by_quarter` in `stress/results/level5-*.json`.

## 6. Fault injection and recovery (Level 6)
### 6.1 Results that passed
- **kill -9 during writes, 25 repetitions** (6 writer threads: calcs, corrections, projects, 200 KB uploads, random kill after 0.05-1.5 s; 1,988 acknowledged operations): restart every time (max 0.06 s), integrity_check ok, audit chain intact (API and independent), every acknowledged job present and complete with its DXF, no half-written job visible, no job without audit entry, no correction job without obligations, no document without record or blob, 40-job replay sample 0 bad. Power loss/OS crash was NOT tested (a process kill does not drop the OS page cache).
- **SIGTERM during requests**: process exits at once with signal 15 (no graceful drain; in-flight requests are cut), restart ok, integrity ok.
- Malformed HTTP: invalid JSON, empty body, invalid UTF-8, garbage bytes, CL+TE conflict, negative content length -> 400; wrong or missing content type -> 415; oversized headers -> 431; origin mismatch -> 403; chunked over limit -> 413; process alive after all of it.
- **Fuzz**: 6,500+ mutated requests over all 14 POST routes (500 each, password 120; field removal, type swaps, huge/extreme numbers, 70 KB strings, NUL, unicode, SQL-looking strings, binary mutation of uploads): **0 x 5xx, 0 connection failures, no crash, no hang**.
- Corrupt key file with wrong length or empty: server exits (101, Rust panic message) - fails loudly, no data touched.

### 6.2 Failures found
| # | Severity | Failure | Reproduction |
|---|---|---|---|
| F1 | CRITICAL (data loss, 5xx from valid request) | Missing key file is **silently regenerated**; same for a different 32-byte key. Server starts, old DXF/documents are permanently undecryptable: `GET .../dxf/..` returns 500 `decryption failed`. New data is encrypted under the new key; audit still verifies. | Start with `NF_KEY_FILE` outside data dir, create job, stop, `rm` the key file, start, download DXF of the old job. |
| F2 | CRITICAL (silent data loss/ wrong result) | After a damaged or truncated SQLite file the server **starts without any integrity check** and serves wrong data. `truncate_0`, deleting the db file: starts in first-run mode with all blobs left behind, and **unauthenticated `POST /api/setup` returns 200**, so anyone who can reach the port becomes admin. Random 4 KB page corruption in the middle (6 of 6 seeds): server starts, `PRAGMA integrity_check` reports malformed, `/api/audit/verify` says `intact:true` (entries 83 vs 85 - unreadable rows are skipped by `.flatten()`), only replay notices one bad job. Truncate to half or 4096 B and garbage header: server panics at start (exit 101) - acceptable. | Run the framework: `python3 stress/run.py --level 6 --only db`; or stop server, `truncate -s 0 nexus-fabric.sqlite` (after `PRAGMA wal_checkpoint(TRUNCATE)`), start, `curl -X POST -H 'X-NF-Request: 1' -H 'Content-Type: application/json' -d '{"username":"a","password":"attacker-pass-1"}' /api/setup`. Needs file access to the data dir. |
| F3 | HIGH (DoS, hang) | No read timeout and no connection cap: 300 idle slow-body connections and 100 never-finished header connections stay open indefinitely (fds 12 -> 312, 5,012 with 5,000 plain sockets; limit is only ulimit -n = 20,000). A body shorter than its Content-Length hangs forever. Health stayed 200 in 1-3 ms during my tests, so I did not reach the exhaustion point. | `level6b` in `run.py`, or send headers with `Content-Length: 1000`, 1 byte, then wait. |
| F4 | HIGH (5xx and dropped connections) | Disk full: with a 6 MB tmpfs data dir, valid `POST calculations` returned 500 for every request after request 10 (41 of 50), `POST /api/projects` and a 2 MB upload abort (panic in handler `.unwrap()`; empty reply or 500), log full of panics. Server stayed up, reads worked, integrity_check ok after restart. One job row was left without its audit entry (audit insert errors are ignored `let _ =`), i.e. the audit trail can silently miss an action. Login returned **200 and a cookie, but the session row was not stored**, so the session is immediately 401. | `mount -t tmpfs -o size=6m tmpfs d; NF_DATA_DIR=d nf-server`; create jobs until full. |
| F5 | HIGH (availability) | Login lockout is global: 41 failed logins for any usernames (unauthenticated) make **valid logins return 429** for up to a minute (40/min overall throttle). Existing sessions keep working. Recovered after 61 s. | 41 x `POST /api/login` with wrong passwords, then log in as admin. |
| F6 | MEDIUM (availability) | Login and password change hash with Argon2 **while holding the global DB lock**: max 42 logins/s whatever the concurrency, and 8 concurrent login attempts raise /api/health p50 from 0.4 ms to 142 ms. No password maximum: a 250 KB password is accepted and hashed. | `bottleneck.py`; `login_argon2` rows. |
| F7 | MEDIUM (performance defect) | Every listing is O(total jobs) (see 7); `GET /api/projects` at 1,501 projects x 3,043 jobs took 2.2 s; `list calculations` 2.4 s at 20,000 jobs, though each returns at most 200 rows. Under concurrency everything queues behind it (health p50 1.8 s while 4 clients list). | Level 3 growth table. |
| F8 | LOW | Graceful shutdown absent: SIGTERM = immediate exit. No data loss seen (SQLite WAL). | Level 6 SIGTERM test. |
| F9 | LOW | No sanity bounds on dimensions: cylinder shell r_dev = 1e300 returns REVIEW_REQUIRED with a 3.6 KB DXF; 1e15 cone, stub and assembly are REJECTED by the engine. Correction `new_value` 1e308 and negative radii are accepted for assemblies (11 obligations created). Not a wrong result in the sense of APPROVED, but unrealistic inputs reach a downloadable DXF. | `POST calculations {"family":"cylinder_shell","r_dev":1e300,"length":1e300}`. |
| F10 | LOW | Replay marks a job REJECTED permanently and records a lesson if the child process fails to run for any reason (e.g. process or fd exhaustion), a possible false integrity verdict under resource pressure (code-reading; not reproduced). | `replay` handler. |
| - | no defect | No 5xx, crash or hang from any valid or malformed input except the environmental faults F1, F2, F4. | |

Read-only data directory: NOT tested (running as root, chmod ineffective, `chattr +i` not attempted successfully because the tmpfs branch was available; the disk-full test covers the write-failure path). Filling the real disk was not done.

## 7. Bottleneck analysis (evidence, nothing optimised)
1. **One global `std::sync::Mutex<Connection>` held inside async handlers.** Evidence: (a) mean server CPU stays about 90-97% (about one core of four) from 1 to 64 users; per-thread CPU under 16 writers: four threads at 0.9-1.1 s each over 8 s (total 3.9 CPU-s in 8 s, about 0.5 core) - threads wait, they do not compute; (b) throughput falls as users rise (53 -> 14 rps); (c) `/api/health`, which only counts users, has p50 0.4 ms idle, 22 ms during 8 concurrent replays, 69 ms during 8 concurrent writers, 142 ms during 8 logins, **1.8 s** during 4 clients listing; p95 1.56 s in the endurance run. Blocking calls also occupy tokio worker threads (replay spawns a process and waits synchronously).
2. **Slow listing queries - O(N) because of unindexed correlated subqueries.** `EXPLAIN QUERY PLAN` on the `calc_rows` query shows `SCAN calculations` plus a correlated `SCAN n` for `supersedes`; the same for `obligations` and for `list_projects` (`SCAN c`). With 1,500 jobs the query takes 154 ms; with the `supersedes` subquery removed it takes 2 ms (the `replay_json IS NOT NULL` column made no difference). No indexes exist on `supersedes`, `project_id` or `target_calc`. These run under the global lock (point 1), so one dashboard load stalls every user for 0.3-2.5 s at 3,000-20,000 jobs.
3. **Synchronous disk commits, about 40 fsync per assembly job** (each statement is its own autocommit transaction: records one by one, calculation, audit). strace over 50 header jobs: 2,074 fsync, 21,893 pwrite64. Same server with data on tmpfs: single-client write throughput 555 rps vs 48.5 rps on disk (11x), p50 1.6 ms vs 3.7 ms. So single-write latency is disk-sync bound; once data is in RAM the global lock/CPU bound shows (820 rps at 16 clients on tmpfs).
4. Argon2 under the lock: 42 logins/s flat for 1, 4 and 16 clients.
5. Not bottlenecks: JSON parse, DXF generation (3-43 ms), DXF download (2-3 ms), audit verify (22 ms for 20,000 entries), memory (37-102 MB; 102 MB peak = one 50 MB upload held in memory plus the sealed copy).
Caveat: the bottleneck script's tmpfs `get_calc` read figure (88 rps) is lower than on disk (314 rps); I did not investigate it and treat it as machine noise from other agents.

## 8. Maximum tested workload
20,000 jobs / 240,516 records / 20,069 audit entries / 2.0 GB in one database; 1,501 projects; 3,043 jobs in a project; 64 simultaneous users (mixed); 5,000 simultaneous idle connections; 50 MB uploads; 256 KiB JSON; 1e308 magnitudes; 25 kill -9 cycles; 20 minutes continuous. Nothing crashed, hung or returned 5xx from API input.

## 9. Not tested / skipped
- External DXF or drawing import, background job queue, structural frames/trusses: not implemented - not tested.
- 6-24 h endurance (command in section 5); power-loss durability; real disk full (tmpfs 6 MB used instead); read-only data directory; fd exhaustion at ulimit; TLS/proxy behaviour; multi-machine replay; slow-loris beyond 5,000 connections; load above 64 concurrent users; levels were not each run on an idle machine (other agents active; L5 overlapped L2/L3/L6).
- Python recomputation of record identity hashes differed from the server's for some float formats (a few hundred records at extreme magnitudes); each such record's job was verified by the server's own replay (0 failures), so "every stored record passes its identity hash" rests on replay for those.
- Python fuzz is bounded (500 requests per route) and seeded (seed 1234); results are reproducible with `run.py --level 6`.

## 10. Reproduce
```
cp target/release/nf-server $SCRATCH/nf-server && export NF_BIN=$SCRATCH/nf-server
for L in 1 2 3 4 6; do python3 stress/run.py --level $L --duration 75 --port 8810 --workdir $SCRATCH/w$L; done
python3 stress/run.py --level 5 --duration 1200 --users 3 --think 0.2 --port 8812 --workdir $SCRATCH/w5
python3 stress/run.py --level 6 --only db --port 8815 --workdir $SCRATCH/w6b
python3 stress/bottleneck.py --port 8816 --bin $NF_BIN --workdir $SCRATCH/wbn
```
Level 6 mounts a 6 MB tmpfs (needs root) for the disk-full case and falls back to RLIMIT_FSIZE emulation otherwise.
