# NEXUS FABRIC — full work plan for the CLI

Written 21 Nov 2026 by the cloud session's lead. State it starts from: branch
`claude/nexus-fabric-strategy-8guhxz`, engine 0.4.2 (commit `ed4a5fb`), docs commit `ccaa7f2`.
Read `docs/project/NEXUS_FABRIC_COMPLETE_HANDOFF.md` first; this plan replaces its section 10.

Save this file in the repository as `docs/project/CLI_WORK_PLAN.md` and keep the status column of each task current.

**Development build. Not validated. Outputs must not be used for manufacturing.**

---

## Status of every task (kept current by the CLI)

Statuses: `not started` · `in progress` · `done` (with the commit that holds it) · `blocked` (with what it waits
for) · `design — awaiting owner` · `not on this host` · `tracked` (people only). "Done" means the work was run and
seen to pass on the stated commit; "not run" is never written as passed.

| Task | Subject | Status | Commit | Note |
|---|---|---|---|---|
| 0.1 | Working on the live server made safe | in progress | | facts under the task |
| 0.2 | Readiness and registers tell the truth | not started | | |
| 0.3 | The two near-tangent FAILs (D4) | not started | | |
| 1.1 | Shallow and near-180° branch angles | blocked | | owner decision pending (asked 20 Nov) |
| 1.2 | A true error bound everywhere | not started | | |
| 1.3 | Every convention stated in the DXF and the result | not started | | engine-version bump: ask the owner |
| 1.4 | Surface attribute for every family | not started | | |
| 1.5 | Checks that do not depend on the construction | not started | | |
| 1.6 | Sampling density | not started | | |
| 1.7 | Set-on joint, remaining case — design note | not started | | design only |
| 1.8 | Validation as a permanent gate | not started | | |
| 1.9 | Third independent review | not started | | after 1.1–1.6 |
| 2.1 | The five failing audit checks | not started | | |
| 2.2 | Architecture review findings RA-001 … RA-017 | not started | | |
| 2.3 | Stress findings | not on this host | | needs a non-live machine |
| 2.4 | Desktop security wording | not started | | |
| 2.5 | Database encryption — design | not started | | design only |
| 2.6 | Sign-in hardening — design note | not started | | design only |
| 3.1 | PDF → DXF inside the product | not started | | |
| 3.2 | Forms and refusals | not started | | |
| 3.3 | Words and language | not started | | |
| 3.4 | Speed and scale | not started | | |
| 3.5 | Interface quality | not started | | |
| 4.1 | Reproducible builds | not started | | |
| 4.2 | Release verification | not started | | Windows part needs a real Windows PC |
| 4.3 | Continuous integration | not started | | |
| 4.4 | Release-candidate report | not started | | |
| 4.5 | Legal drafts | not started | | |
| 4.6 | The live server | not started | | only when the owner says so |
| 5.x | Later work | not started | | ask the owner before any of it |
| 6.x | What only people can supply | tracked | | |

---

## A. Rules for every task (never broken)

1. Correctness first. Fail closed: refuse or report INCONCLUSIVE; never guess.
2. Never weaken, skip, delete or loosen a test or a tolerance. Every changed expectation is listed with old value,
   new value and reason.
3. Nothing reaches `APPROVED`. Every DXF stays NOT RELEASED. Never write approved, certified, validated,
   production-ready, secure, hack-proof, error-free or AI-native.
4. Invent no number: no dimension, process value, clearance, K-factor, tolerance from a standard, benchmark or
   accuracy figure. If a rule needs a value nobody has, record it as a question for a fabrication engineer.
5. Thickness, bend allowance, weld gap and kerf are not modelled. Say nothing that suggests otherwise.
6. A new parser runs isolated, without network, with resource limits, and has fuzz tests.
7. Records are append-only. No AI in the product. Fully offline: no network call, telemetry, CDN or auto-update.
8. Only one change to the pattern mathematics at a time; cut geometry stays byte-identical unless the task says it
   changes, and then the engine version is bumped and older results are marked for recomputation (gate G21).
9. The independent reference in `validation/` is changed only with a derivation from the specification or first
   principles written into the diff, never from the engine's code; every planted fault must still be detected.
10. This branch only. No pull requests, tags or releases. Do not edit `marketing/`.
11. Report honestly: failures with their output; "not run" is never "passed".

## B. Before every push — run and report

`cargo build --release` · `cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings` ·
`cargo fmt --all -- --check` · `bash tests/e2e_smoke.sh` · `bash tests/security_smoke.sh` ·
`node tests/ui_smoke.js` (fresh server) · `node tests/desktop_smoke.js target/release/nf-server 8861` ·
`bash tests/offline_check.sh` · `node docs/design/tools/i18n_check.mjs` · `bash tests/notices_check.sh` ·
`python3 validation/run_all.py --spawn target/release/nf-server --quick`; the FULL corpus whenever the engine or
the reference changed. Totals at `ed4a5fb`: 263 / 89 / 66 / 194 / 18; quick 155 cases, full 1392 cases, 0 disagree.

After each task: what changed, exact totals, changed expectations, what is unfinished; update the register the
task belongs to; then push.

---

## PHASE 0 — Safety and housekeeping (do first)

### 0.1 You are working on the live server — make that safe or move
- **Why:** builds, test servers, fuzzing and security tests on the production host can slow or damage the live
  service. The rule is "do not touch the live server".
- **Do:** report to the owner (a) the user account and folder you work in, and that they are separate from the
  live service's account, data folder and key file; (b) every port the suites bind (8787–8889 are used by tests)
  and proof none is the live port 8792; (c) CPU and memory limits on your builds (`nice`, `ionice`, `systemd-run`
  with `MemoryMax`/`CPUQuota`); (d) that nothing ever targets the live address.
- **Never on this host:** `stress/run.py`, `tests/security_r6.sh` against anything but your own throw-away server,
  the root part of `tests/offline_check.sh`, or any command that touches the live service, Caddy or its data.
- **Recommend to the owner:** a separate development machine. Until then, heavy runs outside working hours.
- **Done when:** the four facts are written in `docs/project/CLI_WORK_PLAN.md` under this task.

### 0.2 Readiness and registers tell the truth
- `docs/project/READINESS.md`, row "Geometry verified": rewrite from the full-corpus run on 0.4.2 (1392 cases,
  973 agree, 419 refused, 0 disagree, 0 wrongly accepted, 0 wrongly refused, 27 planted faults detected). Keep it
  "not yet claimed" and list exactly what still blocks it.
- `docs/backend/DEFECT_REGISTER.md`: RB-011 still says the validation disagrees on four cases — update. Give
  every B2-00n finding a row with its status and test name. Add a section "Questions for a fabrication engineer"
  with: minimum K-factor; minimum seam clearance in the flat pattern; practical minimum and maximum branch angle;
  justification of the 0.01 mm chord limit; marking side, offset sign, rolling axis and seam conventions.
- `docs/project/NEXUS_FABRIC_COMPLETE_HANDOFF.md` section 6 heading still names commit `2894cc5`: correct it.

### 0.3 The two near-tangent FAILs (item D4)
- **Why:** you report 0 disagreements but 2 FAILs. If a job is accepted with an error bound that is not a true
  bound, a check does not mean what it says.
- **Do:** state the case ids, what fails, the measured error against the reported `eps_num`, the state the job
  ends in and whether a DXF is offered. If a file is offered while the bound is exceeded: make the job REJECTED
  (bound gate fails) until the bound is fixed in task 1.2. Add a regression case for each.

---

## PHASE 1 — Mathematics and engine (first priority of the project)

### 1.1 Shallow and near-180° branch angles (owner decision pending — ask before coding)
- **Proposal:** when the verifier cannot confirm the geometry because of conditioning, answer with a refusal that
  has its own code and a plain message naming `beta_deg`, instead of an accepted job that then fails G10. Do NOT
  hard-code 0.003°: derive the condition from the verifier's own tolerance. Same logic at the obtuse end; show
  `eps_num` prominently on any accepted near-180° job.
- **Tests:** boundary cases just inside and outside at both ends; corpus cases; refusal-code table updated.

### 1.2 A true error bound everywhere (RB-005, D4, B2-007)
- **Why:** `eps_num` and `eps_rep` must be upper bounds, or be called estimates and not gate anything.
- **Do:** for each family write the derivation of the bound in `docs/backend/TOLERANCE_POLICY.md`; replace
  sampled estimates by a provable bound (interval evaluation of the governing expressions or an analytic bound
  on the second derivative for chord error); where no bound can be proved, the gate returns INCONCLUSIVE.
- **Tests:** property tests over random parameters comparing measured error (high-precision reference) with the
  reported bound — never above it; the two near-tangent cases; full corpus.

### 1.3 Every convention stated in the DXF and the result (RB-002, RB-003, RB-007, RB-009, RB-008 remainder)
- **What to state, for every part:** viewing/marking side; sign of the offset δ and which way positive is; the
  rolling axis; which edges are seams and which are ends; the angular datum (where 0° is) and the direction of
  increasing angle; the plate origin and axes; that the cut lines are the theoretical line with no kerf
  compensation.
- **How:** the same scheme as `docs/backend/DXF_METADATA.md`: stable `KEY: value` lines in the 999 block AND a
  TEXT entity on layer `NF_NOT_RELEASED`; the same facts in the job result and the interface (English and
  Japanese). Make the outer profile one closed polyline where the format allows (RB-008).
- **Quality gate:** reads every statement back and compares it with the job; a missing, altered or contradictory
  statement fails the gate and the file is withheld.
- **Tests:** a statement test per family and part; tampered-file tests; cut geometry byte-identical (snapshot
  test); round trip through the independent reader. Bump the engine version.

### 1.4 Surface attribute for every family; "unknown" never switches a check off (RB-010)
- Cone, eccentric reducer and cylindrical shell get the same surface declaration as the branch families
  (`outside`, `inside`, `neutral`, `unknown`).
- With `unknown`, radius checks are reported INCONCLUSIVE with the reason, never silently skipped.
- Old requests without the field keep their exact behaviour and geometry (compat snapshot).

### 1.5 Checks that do not depend on the construction (RB-006)
- A mating test that works from the two DXF files alone: re-read both files with the independent reader, fold
  them back using only the stated conventions, and confirm the two cut edges coincide. This also proves the
  convention statements of 1.3 are sufficient to build the part.
- Keep the verifier's code free of calls into `develop.rs`; add a test that fails if it imports them.

### 1.6 Sampling density (RB-004)
- Place polyline vertices at tangent points and at parameter values where curvature is extreme, so the curve's
  key points are exact vertices. Record in the fabrication list that the 0.01 mm chord limit has no engineering
  source. No change of the limit without the owner.

### 1.7 Set-on joint, remaining case (RB-001)
- The set-on with the saddle on the branch OUTSIDE surface needs the wall thickness, which is not modelled.
  Do not approximate it. Write `docs/backend/THICKNESS_MODEL_DESIGN.md`: what inputs would be needed, what each
  part's contact surface becomes, what must come from a fabrication engineer. No code until the owner approves.

### 1.8 Validation as a permanent gate
- CI: keep the quick corpus on every push; add the FULL corpus as a scheduled (nightly) and manually triggered
  job. Decide, with a derivation, whether to tighten the scale tolerance that was relaxed to 2e-9 relative.
- Fix: `validation/run_all.py` leaves its server running if an exception escapes the planted-fault stage;
  `validation/exact_onsurface_check.py` uses a fixed port.
- Add corpus cases for every new rule of phase 1, just inside and just outside each bound.

### 1.9 Third independent review
- After 1.1–1.6: have a reviewer with no knowledge of the code derive the five developments again and test the
  build against a high-precision reference (as `docs/backend/REVIEW_B2_COMPUTATIONAL.md` did). Target: no finding
  above "low". Fix what it finds before phase 3 work on the engine.

---

## PHASE 2 — Security and reliability

### 2.1 The five failing audit checks (`tests/security_r6.sh`: 35 pass, 5 fail) — fix the product, never the test
- **V-45:** remove `'unsafe-inline'` from the style policy: move every inline `style=` and injected style into
  classes in `web/styles.css`; the canvas and SVG keep working. Browser suite must stay green in both themes.
- **V-47:** refuse a state-changing request whose `Origin` has a non-web scheme (`file://`, `data:`…) or the wrong
  scheme for the listener; keep desktop mode working (its origin is `http://127.0.0.1:port`).
- **V-44:** `deploy/install.sh` must not pipe a download into a shell and must not build as root: require an
  existing toolchain or a verified package, build as an unprivileged user, install with the least privilege.
- **Tests to add:** cross-origin requests; invalid and missing Origin; malformed installer input; insufficient
  privileges; an interrupted install; rollback leaves the previous version working.
- **Done when:** the audit reads 40 of 40 and `docs/security/VULNERABILITY_REGISTER.md` is updated.

### 2.2 Architecture review findings (`docs/backend/REVIEW_A_ARCHITECTURE.md`, RA-001 … RA-017)
- Read all seventeen; give each a status row. Fix in this order: RA-003 (the audit chain can be truncated and
  re-signed — anchor the chain so removal of the newest entries is detected), RA-002 (sign-in race), RA-001
  (connection exhaustion: a per-client connection cap, header and body read deadlines, a bound on idle
  keep-alive), RA-006 (lockout cannot be used to lock out the administrator), RA-007 (after an engine upgrade the
  interface offers a one-click recompute of affected jobs; old results stay as records).
- Each fix has a test that fails before it and passes after.

### 2.3 Stress findings (`docs/backend/STRESS_AND_RECOVERY_REPORT.md`, addendum) — on a non-live machine only
- Connection flood stalls the health check for 10 s; a restart on a still-full disk fails; two malformed
  requests are held open instead of refused. Fix, then re-run `stress/run.py` levels 4 and 6 and record the
  figures before and after.

### 2.4 Desktop security wording (owner-approved text: `docs/security/DESKTOP_KEY_ASSESSMENT.md` section 6)
- `packaging/INSTALL.md`, `packaging/README.txt` and the first-run screen in desktop mode, English and Japanese:
  the program does not encrypt the database or the whole data folder; stored files have their own encryption
  but that does not make the data set encrypted; security depends on the computer; recommend disk encryption,
  individual accounts, restricted access to the data folder and backups. Must not suggest that an
  operating-system account encrypts the database. The server needs to tell the client it is in desktop mode.

### 2.5 Database encryption and key protection — DESIGN ONLY
- Write `docs/security/DESKTOP_ENCRYPTION_DESIGN.md`: threat model (a copied data folder is in scope);
  encryption of the SQLite database; keys wrapped by the operating system (Windows DPAPI, macOS Keychain, Linux
  Secret Service, with a stated fallback); a recovery key held by the customer organisation's administrator;
  migration of existing installations that can be interrupted and resumed; backup and restore on another
  machine; key rotation; what cannot be tested without real Windows and macOS machines.
- No code until the owner approves the design.

### 2.6 Sign-in hardening — DESIGN NOTE ONLY
- V-23: multi-factor sign-in and account recovery that work fully offline (time-based codes, recovery codes).
  One page of options and costs for the owner.

---

## PHASE 3 — Product completeness (backend and frontend)

### 3.1 PDF → DXF inside the product (the owner's requested feature)
- **Read:** `docs/backend/PDF_TO_DXF.md` and `crates/nf-pdf`.
- **Server:** an endpoint that converts an uploaded PDF document by launching `nf-pdf2dxf` as a child process
  with no inherited environment, a fresh temporary working directory, no network (`unshare -n` where available),
  a kill timeout and memory limit. Inputs: document id, page, scale (millimetres per point or 1:N — required),
  tolerance, options. Store the DXF and the JSON report as documents (encrypted like other documents); record
  the conversion (source hash, page, scale, converter version) as an append-only record with an audit entry.
  Refusals keep the converter's code and message. State honestly what is not isolated on Windows, and compile
  and test the converter for Windows in the release workflow.
- **Interface:** on Projects & Documents, a "Convert to DXF" action on a PDF: page and scale form, the report
  (counts, bounding box, warnings, skipped text), a preview drawn from the DXF, download. English and Japanese.
  The result is labelled "Converted from PDF — not verified — not released" and is never shown as a flat
  pattern or attached to a calculation.
- **Tests:** API (success, every refusal code, every limit, wrong role); browser in both languages; a test that
  the child really has no network and is killed on timeout; the fuzz corpus run through the server path.
- **Close the converter's own gaps first:** the operand limit has no test; `--no-text` has no test; the 64 MiB
  input limit is only tested with a lowered value.

### 3.2 Forms and refusals
- BH-014: when the user returns to the form ("New from these values") every refused field is marked with the
  message for its refusal code, in both languages, for all 41+ codes.
- The refusal-reason minimum length counts characters, not bytes (a two-character Japanese reason passes
  today). Same for any other length rule.

### 3.3 Words and language
- A test that fails if the interface, packages or install texts contain any word of rule A.3.
- Confirm every gate G00–G22, every check name, every refusal code and every record kind has a Japanese label;
  the Japanese walk in `tests/ui_smoke.js` fails on any untranslated identifier outside the documented list.
- Keep the disclosure "EV-1 (AI-generated, not independently reviewed)".

### 3.4 Speed and scale — measure before and after, no behaviour change
- Listing endpoints grow with the total number of jobs: add pagination with indexed queries.
- Audit verification took 1.6 s at 20,000 jobs: verify incrementally from the last verified anchor, full
  verification on demand.
- One database mutex limits throughput with many users: separate read connections (WAL allows it), writes
  stay serialised.

### 3.5 Interface quality
- Find the cause of the intermittent browser-check failure seen twice (a page-wait race in the viewer-role
  check) and fix the wait, not the assertion.
- Keyboard and screen-reader pass on the new-calculation form, the job tabs and the conversion screen.
- Offline help in English and Japanese inside the program: what each part family needs, what each gate and
  refusal means, what is not modelled. Three sample projects.

---

## PHASE 4 — Release

### 4.1 Reproducible builds
- Two builds of one commit give different checksums. Remove the causes (timestamps in the zip and the
  installer, path prefixes, non-deterministic ordering) so a checksum identifies a commit; if one cause cannot
  be removed, the manifest says which.

### 4.2 Release verification
- `packaging/verify_release.sh` on the release commit: build from a clean copy, checksums, version agreement
  (installer name, `Cargo.toml`, `--version`, engine identifier), Linux checks natively. A manifest row that was
  cut short by a full disk in the rehearsal must be complete this time.
- Windows: `docs/release/HANDOFF_WINDOWS_VERIFICATION.md` on a real Windows PC (not on the VPS).

### 4.3 Continuous integration
- Both jobs green on GitHub (`software`, `geometry-validation`); the release workflow run once by manual
  trigger to prove it builds on Windows and Linux runners (artefacts only; no release, no tag).

### 4.4 Release-candidate report
- `docs/release/RC1_REPORT.md`: source commit; artefacts with sizes and SHA-256; every suite's result; the five
  readiness classifications with evidence; known limitations; open security findings; install instructions;
  what was run natively and what only under an emulator.

### 4.5 Legal drafts (for the owner's lawyer; each marked "DRAFT — not reviewed by a lawyer")
- Terms of use, privacy statement (no telemetry; data stays on the customer's machine), engineering
  responsibility notice, in English and Japanese. See `docs/legal/LEGAL_READINESS.md`.

### 4.6 The live server
- Updated only when the owner says so, by pulling the release commit, rebuilding and restarting, with
  `NF_ALLOWED_HOSTS=fabric.vsnexustechnologies.com`. Back up the data folder and key first; keep the previous
  binary for rollback; check `/api/health` and the audit verification afterwards.

---

## PHASE 5 — Later; ask the owner before starting any of these

1. Plate thickness, neutral axis, bend allowance, weld gap, kerf — needs process values from a fabrication
   engineer; design first (`THICKNESS_MODEL_DESIGN.md`).
2. More part families: segmented elbows, rectangular-to-round transitions, Y-branches, several branches on one
   header. Each: derivation written first, independent reference before the engine, regression cases, gates.
3. 3D view of the folded part and the joint.
4. PDF pattern sheet (dimensions, title block, status NOT RELEASED), cutting list, project export.
5. A desktop window instead of the browser tab.
6. Engineer approval workflow (roles, signature, obligations closed, replay, audit) — the only way a result may
   ever leave NOT RELEASED; design reviewed by the owner before any code.
7. DXF import (isolated parser, fuzz tests), then nesting on sheets and CNC output.

## PHASE 6 — What only people can supply (track, do not attempt)

- A fabrication engineer: the questions in task 0.2, and a real cut part compared with a pattern.
- A lawyer admitted in Japan: terms, liability limits, privacy, status of AI-assisted code; a trademark search
  for "NEXUS FABRIC" and "AMOEBA".
- A person at a real Windows PC and a real Mac.
- A native-speaking engineer's review of every Japanese text.

---

## Order of work

0.1 → 0.2 → 0.3 → 1.1 (after the owner decides) → 1.2 → 1.3 → 1.4 → 1.5 → 1.6 → 1.8 → 1.9 → 2.1 → 2.2 → 2.4 →
3.1 → 3.2 → 3.3 → 3.4 → 3.5 → 2.3 (non-live machine) → 4.1 → 4.2 → 4.3 → 4.4 → 4.5.
Design-only documents (1.7, 2.5, 2.6) can be written at any point while a long test run is going.
Stop and ask the owner on: any engine-version bump, any change to what the engine accepts, any design document
awaiting approval, anything that needs a fabrication engineer's value, and before touching the live service.
