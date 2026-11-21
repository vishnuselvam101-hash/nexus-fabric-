# NEXUS FABRIC — hand-off of the coding work to Claude Code on the owner's machine

Written 20 Nov 2026 by the cloud session's lead. Start here:

```
Read docs/project/HANDOFF_CLI.md, then docs/project/MASTER_GOAL.md and docs/project/READINESS.md.
Do the tasks in section 5 in order. Follow sections 2 to 4 without exception.
```

## 1. What this product is

Engineering software that computes flat plate patterns and R12 DXF files for steel pipe and plate fabrication (first
market: Japan). Rust workspace: `crates/uem` (pattern mathematics), `crates/nf-dxf` (DXF writer, reader, quality
gate), `crates/nf-core` (jobs, records, gates), `crates/nf-server` (axum server, SQLite, sessions; the web client in
`web/` is embedded with `include_str!`, so rebuild after any change there). Runs fully offline; desktop mode is
`nf-server --desktop`. "AMOEBA" is the record, dependency, correction and replay layer. **The product contains no AI
and none is to be added unless the owner asks.**

## 2. Rules that are never broken

1. A wrong pattern means wrongly cut steel. Correctness first; the software fails closed (refuse or report
   INCONCLUSIVE, never guess).
2. Never weaken, skip, delete or loosen a test or a tolerance to get a pass. A changed expectation is listed with old
   value, new value and the engineering reason.
3. `APPROVED` must stay unreachable. Every DXF stays marked NOT RELEASED. Never write approved, certified, validated,
   verified for manufacturing, production-ready, secure, hack-proof, error-free, or "AI-native" anywhere.
4. Never invent dimensions, process parameters, standards, tolerances from standards, customers, prices, benchmarks
   or accuracy figures. Thickness, bend allowance, weld gap and kerf are NOT modelled: say nothing that suggests
   otherwise.
5. No parser is merged unless it runs in an isolated, network-less, resource-limited process and has fuzz tests.
6. Records are append-only. Never delete or rewrite one.
7. No network calls, telemetry, CDN or auto-update in the product (`tests/offline_check.sh` enforces this).
8. Branch `claude/nexus-fabric-strategy-8guhxz` only. Commit in logical increments and push. **Do not open, merge or
   close a pull request. Do not create tags or releases.**
9. Do not touch the live server (fabric.vsnexustechnologies.com). Its own Claude updates it by pulling.
10. Confidential factory drawings never go to any external service.
11. Report honestly: a failing test is reported with its output; "not run" is never written as "passed".

## 3. How to verify (run all before every push; totals at commit `cd452de`)

| Command | Expected |
|---|---|
| `cargo build --release` | builds |
| `cargo test --workspace` | 157 pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |
| `bash tests/e2e_smoke.sh` | 89 pass |
| `bash tests/security_smoke.sh` | 66 pass |
| `node tests/ui_smoke.js` against a fresh server (first 6 lines of the file say how) | 183 pass |
| `node tests/desktop_smoke.js target/release/nf-server 8861` | 18 pass |
| `bash tests/offline_check.sh` | all ok (the no-network part needs Linux and root) |
| `python3 validation/run_all.py --spawn target/release/nf-server --quick` | 155 cases; 4 known disagreements until task B3 lands |
| `bash tests/security_r6.sh` (see its header for the setup token arguments) | 35 pass, 5 fail until task C1 lands |
| `node docs/design/tools/i18n_check.mjs` | no problem |

The shell scripts need Git Bash or WSL on Windows. If a suite cannot run on your machine, say which and why.

## 4. State on 20 Nov 2026

Done and pushed: five part families; engine 0.4.0 with the set-on joint (specification pairing), DXF quality gate and
independent joint checks; English/Japanese interface; desktop mode and Windows/Linux packages; offline check; stress
re-run; bug hunt with 15 of 18 defects fixed; release-verification tool (`packaging/verify_release.sh`, manifest in
`docs/release/`); owner decisions in `docs/project/READINESS.md`, `docs/security/DESKTOP_KEY_ASSESSMENT.md`,
`docs/legal/LEGAL_READINESS.md`.

**In progress in the cloud session — do NOT start these until their commits are on the branch** (pull first; if a
task below says "after A-n lands", check `git log`):

| Id | Work | Files it is changing |
|---|---|---|
| A1 | Engine 0.4.1: DXF states the joint as declared; structured refusal codes | crates/nf-core, nf-dxf, nf-server, uem; web/app.js, web/i18n.js; tests/ui_smoke.js |
| A2 | Validation tool fix (scale and near-tangent comparisons, planted faults) | validation/ |
| A3 | Second independent mathematical review | docs/backend/REVIEW_B2_COMPUTATIONAL.md (report only) |
| A4 | PDF → DXF converter crate (vector PDFs, isolated process, fuzz tests) | new crate crates/nf-pdf, on its own branch until merged |
| A5 | The advertisement film | marketing/nexus-fabric/flagship3 (not coding work for the CLI) |

## 5. Tasks for the CLI, in order

### B. Mathematics and engine (first priority)
- **B1. Fix every finding of `docs/backend/REVIEW_B2_COMPUTATIONAL.md`** (after A3 lands), most severe first. Each fix:
  a unit test, a regression case in `crates/nf-core/tests/regressions/`, and a check that does not reuse the
  construction it checks. Bump the engine version when results change.
- **B2. State every convention in the DXF and the result** (after A1 lands): RB-002 sign of the offset, RB-003 rolling
  axis and seam edges, RB-007 angular datum, RB-009 plate origin, RB-008 remainder, RB-010 surface attribute for cone,
  reducer and shell and no silent switch-off of radius checks. Same `KEY: value` 999 scheme as
  `docs/backend/DXF_METADATA.md`, read back by the quality gate. See `docs/backend/REVIEW_B_COMPUTATIONAL.md`.
- **B3. Geometry validation green for the right reasons** (after A2 lands): run it, explain every remaining
  disagreement, close D4 (a true error bound; RB-005), justify or replace the 0.01 mm chord limit (RB-004) — the value
  has no engineering source yet, so record it as an open question for a fabrication engineer rather than inventing one.

### C. Security
- **C1. The five failing audit checks, fixed in the product, never in the tests:** V-45 remove `'unsafe-inline'` from
  the style policy (move inline styles to classes); V-47 refuse a request Origin with a non-web scheme or the wrong
  scheme; V-44 `deploy/install.sh` must not pipe a download into a shell and must not build as root. Add tests for
  cross-origin requests, invalid and missing Origin headers, malformed installer input, insufficient privileges, an
  interrupted install and rollback.
- **C2. Desktop security wording** (after A1 lands; owner-approved text in `docs/security/DESKTOP_KEY_ASSESSMENT.md`
  section 6): install guide (`packaging/INSTALL.md`, `packaging/README.txt`) and the first-run screen in desktop mode,
  English and Japanese. Must not suggest that an operating-system account encrypts the database.
- **C3. Database encryption and key protection:** write the threat model, migration, backup/restore, key rotation and
  recovery design first (organisation-held recovery key) as `docs/security/DESKTOP_ENCRYPTION_DESIGN.md`; no code
  until the owner approves the design.
- **C4. Open stress findings** (`docs/backend/STRESS_AND_RECOVERY_REPORT.md`, addendum): connection flood (RA-001),
  restart on a full disk, malformed requests held open.

### D. Release hygiene
- **D1. Third-party notices:** generate THIRD_PARTY_NOTICES (name, version, licence, licence text) from the exact
  build and ship it in every package; add a test that fails if a package lacks it.
- **D2. Words test:** a test that fails if the interface, packages or install texts contain untrue marketing words
  (rule 3 list). Remove leftover AI wording: interface "planned" lists that announce drawing reading by AI, and the old
  positioning line quoted in `docs/project/MASTER_GOAL.md`.
- **D3. Windows native verification:** `docs/release/HANDOFF_WINDOWS_VERIFICATION.md` (only on a real Windows PC).
- **D4. Release-candidate report:** `docs/release/RC1_REPORT.md` with source commit, artefacts and SHA-256, every
  suite's result, known limitations, security findings, install instructions, and the five readiness classifications.

### E. PDF → DXF in the product (after A4 lands)
- Server: an endpoint that converts an uploaded PDF document by launching `nf-pdf2dxf` as an isolated child (no
  inherited environment, temporary working directory, no network, time and memory limits), stores the DXF and the
  conversion report as documents, records the conversion (source hash, page, scale, converter version) and writes an
  audit entry. The scale is required; a scanned PDF is refused with its reason.
- Interface: on Projects & Documents, a "Convert to DXF" action on a PDF, asking for page and scale, showing the
  report, a preview and the download; English and Japanese; the result is labelled "Converted from PDF — not verified —
  not released" and is never presented as a flat pattern.
- Tests: API, browser (both languages), refusals, limits, and the isolation itself.

### F. Execution and scale
- Listing endpoints that grow with the total number of jobs; audit verification time at 20,000 jobs; throughput under
  many simultaneous users (one database mutex today). Measure with `stress/run.py` before and after; no behaviour
  change.

### G. Later (ask the owner before starting any of these)
Thickness, bend allowance, weld gap and kerf (needs a fabrication engineer's process values); more part families;
3D view; PDF reports; a desktop window instead of the browser tab; the engineer approval workflow; DXF import;
nesting and CNC output; draft terms of use and privacy statement for the owner's lawyer.

## 6. How to report

After each task: what changed, the exact totals of every suite in section 3, every changed expectation, anything not
finished or uncertain. Update the document the task belongs to (defect register, review status, bug-hunt status,
vulnerability register). Never report something as done unless you ran it and saw it pass.
