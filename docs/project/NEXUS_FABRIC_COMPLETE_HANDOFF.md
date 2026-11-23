# NEXUS FABRIC — complete hand-off (single file)

Written 20 Nov 2026 by the cloud session's lead for the CLI session running on the owner's own machine.
Everything needed to continue the work is in this one file. Where a longer document exists, its path is given, but
you do not need to open it to start.

**Development build. Not validated. Outputs must not be used for manufacturing.**

---

## 0. How to use this file

On the owner's PC (Windows 10/11, macOS or Linux):

1. Install Git, and the CLI development tool (see its official documentation). Sign in.
2. Get the project:
   ```
   git clone https://github.com/vishnuselvam101-hash/nexus-fabric-.git
   cd nexus-fabric-
   git checkout dev/nexus-fabric-strategy-8guhxz
   ```
   The repository is private; Git asks for a GitHub sign-in.
3. Start the CLI session in that folder and type:
   ```
   Read docs/project/NEXUS_FABRIC_COMPLETE_HANDOFF.md completely. Follow sections 3 and 6 without exception.
   Do the tasks in docs/project/CLI_WORK_PLAN.md in order (it replaced section 10 on 21 Nov 2026),
   starting with the first one that is not blocked.
   ```
4. Always `git pull` before starting a task: the cloud session is still pushing the work listed in section 8.

---

## 1. What the product is

NEXUS FABRIC computes mathematically checked **flat plate patterns** and **R12 DXF files** for steel pipe and plate
fabrication. First market: Japan. An engineer enters the dimensions of a part or joint; the software develops the
surface onto the plane, checks its own result with independent formulations, records every value and step, and
writes a DXF that is marked NOT RELEASED.

- **Part families today (five):** cylindrical shell; cone frustum (concentric reducer); eccentric reducer (oblique
  cone frustum); branch stub with a saddle cut; header shell with a branch opening (and the two-part header-and-branch
  joint, optionally declared as a set-on joint).
- **AMOEBA:** the record, dependency, correction and replay layer. Every value and step is a linked, append-only
  record; a job has a dependency graph; correcting one value supersedes the old job and raises scrutiny obligations
  on everything that depended on it; lessons from corrections are matched against later jobs; a job can be replayed
  and must match.
- **No automated drawing reading.** The product contains none, and none is to be added unless the owner asks (decision of 20 Nov 2026).
  The software does disclose that its algorithms are evidence level EV-1 and not independently reviewed
  (evidence level EV-1); that disclosure stays.
- **Fully offline.** No network calls, no telemetry, no CDN, no auto-update. Proven by `tests/offline_check.sh`.
- **Interface:** English and Japanese with a switch; light and dark themes; a "drawing sheet" job view.
- **Delivery:** a server (browser clients) and a desktop mode (`nf-server --desktop`, one person, this computer
  only). Windows installer, Windows portable zip, Linux tar.gz. About 2.6 MB to download, about 9 MB installed.
- **Not modelled:** plate thickness, neutral axis, bend allowance, weld gap, kerf. Patterns are zero-thickness
  reference surfaces. Engineer approval is not implemented: no result can reach APPROVED.

---

## 2. Owner decisions in force

| Date | Decision |
|---|---|
| standing | Never invent dimensions, process parameters, standards, factory data, customers, testimonials, benchmarks, certifications, prices or validation claims. Never claim hack-proof, zero-error, certified. |
| standing | Automated drawing reading is never the final authority for geometry. APPROVED stays unreachable until independent verification, obligation closure, audits, replay and authorised engineer approval exist. Fail closed. |
| standing | No parser is merged without an isolated, network-less, resource-limited process and fuzz tests. |
| standing | Confidential factory drawings never go to an external service. Benchmark and customer data are never committed. |
| standing | Priority order: data integrity and engineering correctness, DXF precision, backend reliability, computational correctness, security, interface, performance, marketing. |
| 20 Nov | Readiness is stated in five independent classifications, never one label (section 9.6). |
| 20 Nov | Desktop security: say plainly that the program does not encrypt the database or the whole data folder; a copied data folder IS inside the threat model; database encryption with operating-system protected keys is a condition for unrestricted desktop deployment; the recovery key is held by the customer organisation's administrator. |
| 20 Nov | The DXF states the joint as declared ("set-on" or "not declared"). Units stay as documented R12 metadata (no `$INSUNITS`). |
| 20 Nov | No automated drawing reading in the software. Claims of automated intelligence and any announcement of automated drawing reading are removed everywhere. |
| 20 Nov | The software must be fully offline. |
| 20 Nov | Focus on the calculations, mathematical algorithms and their execution first. |
| 20 Nov | New feature: upload a PDF drawing and convert it to DXF (vector PDFs, scale given by the user). |
| 20 Nov | All coding tasks go to the CLI session on the owner's machine; the cloud session finishes what it has in flight and keeps the advertisement. |
| 20 Nov | Use less usage: read only what is needed, run long suites once, keep reports short. |

---

## 3. Rules that are never broken

1. A wrong pattern means wrongly cut steel. Correctness first. Refuse or report INCONCLUSIVE; never guess.
2. Never weaken, skip, delete or loosen a test or a tolerance to get a pass. A changed expectation is listed with
   the old value, the new value and the engineering reason.
3. `APPROVED` stays unreachable. Every DXF stays marked NOT RELEASED. Never write approved, certified, validated,
   verified for manufacturing, production-ready, secure, hack-proof, error-free or claims of automated intelligence in the interface,
   packages, documents or films.
4. Invent nothing (see section 2, first row). Say nothing that suggests thickness, allowances, weld gap or kerf are
   handled.
5. A new parser runs isolated, without network, with resource limits, and has fuzz tests — or it is not merged.
6. Records are append-only. Never delete or rewrite one.
7. No network calls, telemetry, CDN or auto-update in the product.
8. Work on branch `dev/nexus-fabric-strategy-8guhxz` only. Commit in logical increments and push.
   **Do not open, merge or close a pull request. Do not create tags or releases.** (Pull request #1 exists; leave it.)
9. Do not touch or probe the live server (section 14).
10. Report honestly: failures with their output; "not run" is never written as "passed"; nothing is "done" unless
    you ran it and saw it pass.
11. Only one worker edits the pattern mathematics (`crates/uem`) at a time.

---

## 4. Repository map

| Path | What it is |
|---|---|
| `crates/uem` | Pattern mathematics: `develop.rs` (developments), `profile.rs`, `verify.rs` (checks), `independent.rs` (independent verifier), `lib.rs`. Governing equation for the saddle/opening: `s(φ) = (sqrt(R² − (δ + r cosφ)²) − r sinφ cosβ) / sinβ`. |
| `crates/nf-dxf` | R12 (AC1009) writer and independent reader; `quality.rs` is the DXF quality gate. Units: `$LUNITS`, a 999 comment block and a TEXT entity (R12 has no `$INSUNITS`). Non-cutting layer `NF_NOT_RELEASED`. |
| `crates/nf-core` | Jobs, records, gates G00–G22, dependencies, corrections; `tests/regressions/` is the regression corpus (001–004…). |
| `crates/nf-server` | axum server, SQLite (rusqlite, WAL), sessions and roles (admin, engineer, viewer), audit chain, encrypted file store (AES-GCM), `desktop.rs` (desktop mode), `tests/cli.rs`. |
| `web/` | Dependency-free ES modules: `app.js`, `viewer.js` (pattern canvas), `i18n.js` (English/Japanese catalogue, 815 keys), `styles.css`, `index.html`. Embedded with `include_str!`: rebuild after any change. Strict content policy. |
| `tests/` | `e2e_smoke.sh` (API), `security_smoke.sh`, `security_r6.sh` (deeper audit), `ui_smoke.js` (browser, both languages), `desktop_smoke.js`, `offline_check.sh`. |
| `validation/` | Independent Python reference and comparison (`run_all.py`, `reference/`, `checks/`, `seeded_faults.py`, `exact_onsurface_check.py`). |
| `stress/` | `run.py` levels 1–6, results. |
| `packaging/` | `build_all.sh` (Linux tar.gz, Windows installer via NSIS, portable zip, SHA256SUMS), `verify_release.sh` (release verification, manifest). |
| `deploy/` | systemd unit, Caddy file, `install.sh`, backup timer for a Linux server. |
| `docs/project/` | `MASTER_GOAL.md` (goal, twelve release gates), `READINESS.md`, this file. |
| `docs/backend/` | Architecture, defect register (BE-, RA-, RB- items), both reviews, validation report, tolerance policy, stress report, `DXF_METADATA.md`, `REFUSAL_CODES.md` (the last two arrive with engine 0.4.1). |
| `docs/security/` | Vulnerability register (V-items), audits, `DESKTOP_KEY_ASSESSMENT.md`. |
| `docs/design/` | Design system, localisation rules, screenshots, tools (`seed_demo.js`, `i18n_check.mjs`). |
| `docs/qa/BUG_HUNT_R7.md` | 18 defects with status. |
| `docs/legal/LEGAL_READINESS.md` | Working list for a lawyer (not legal advice). |
| `docs/release/` | Release manifests, Windows hand-off. |
| `marketing/nexus-fabric/` | Films (not coding work): `flagship3` is the current advertisement. Older films carry claims of automated intelligence and must not be published. |
| `data/` | Ignored by git: a leftover test data folder on the cloud machine. |

---

## 5. Build, run, configure

Build: `cargo build --release` → `target/release/nf-server`.

```
nf-server                    start the server, configured by environment variables
nf-server --desktop [--port N] [--no-browser]   desktop mode: one person, this computer only
nf-server --help | --version                    print and exit (writes nothing, opens no port)
any other argument                              error, exit status 2
```

| Variable (server mode) | Meaning |
|---|---|
| `NF_LISTEN` | address and port (default 127.0.0.1:8787) |
| `NF_DATA_DIR` | data folder (default `data` in the working directory) |
| `NF_KEY_FILE` | storage key file (default inside the data folder; keep it outside) |
| `NF_SETUP_TOKEN` | one-time token for creating the first administrator; without it a token is written to `setup-token` in the data folder |
| `NF_ALLOWED_HOSTS` | extra host names the server answers to (a foreign Host gets 421) |
| `NF_TRUSTED_PROXY` | reverse proxies whose X-Forwarded-For is used |
| `NF_COOKIE_SECURE` | 1 behind HTTPS |
| `NF_REQUEST_TIMEOUT_SECS`, `NF_HEADER_TIMEOUT_SECS`, `NF_MAX_CONNECTIONS`, `NF_SHUTDOWN_GRACE_SECS` | limits |

Server contract worth knowing: first-run setup needs the token (header `X-NF-Setup-Token` or JSON `setup_token`);
state-changing calls need the header `X-NF-Request: 1`; a refused calculation is HTTP 422 with state `REFUSED`; an
unknown request field is 400; magnitudes are bounded (1e-6 … 1e8 mm); patterns are drawn as seen from the OUTSIDE of
the formed part (provisional convention); results of an older engine are marked for recomputation (gate G21);
a superseded job gains gate G00 = FAIL.

Desktop mode: data in the per-user folder (`%LOCALAPPDATA%\NexusFabric\data` on Windows;
`$XDG_DATA_HOME` or `~/.local/share/nexus-fabric` on Linux); loopback only; one-time setup link carried in the URL
fragment; a second copy on the same data is refused.

Packages: `bash packaging/build_all.sh` → `dist/` (never committed). Release verification:
`bash packaging/verify_release.sh [--previous <older installer>] --out <dir>` → manifest in `docs/release/`.

---

## 6. How to verify (run before every push)

| Command | Expected at commit `ed4a5fb` (engine 0.4.2) |
|---|---|
| `cargo build --release` | builds |
| `cargo test --workspace` | 263 pass (engine 0.4.2) |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |
| `bash tests/e2e_smoke.sh` | 89 pass |
| `bash tests/security_smoke.sh` | 66 pass |
| `node tests/ui_smoke.js` on a fresh server | 194 pass |
| `node tests/desktop_smoke.js target/release/nf-server 8861` | 18 pass |
| `bash tests/offline_check.sh` | all ok (the no-network part needs Linux and root) |
| `node docs/design/tools/i18n_check.mjs` | no problem |
| `python3 validation/run_all.py --spawn target/release/nf-server --quick` | 155 cases: 93 agree, 62 refused, 0 disagree (exit 0) |
| `python3 validation/run_all.py --spawn target/release/nf-server` (FULL, ~17 min) | 1392 cases: 973 agree, 419 refused, 0 disagree, 0 wrongly accepted, 0 wrongly refused; 27/27 seeded faults detected; 2 pre-existing near-tangent D4 FAILs (identical on 0.4.1) |
| `bash tests/security_r6.sh` | 35 pass, 5 fail (section 9.3) |


Engine 0.4.3 (work plan task 1.1, working tree on `011b061`, clean `nf-verify --full` 21 Nov 2026, log `/root/nf-logs/task-1.1-final-20261121T141553Z`, results in `validation/results/latest.json`): build, clippy, fmt clean; `cargo test` 269 pass; e2e 89; security 66; browser 197; desktop 18; offline (static part), i18n, notices ok; quick 177 cases: 116 agree, 61 refused, 0 disagree; FULL 1398 cases: 977 agree, 421 refused, 0 disagree, 0 wrongly accepted, 0 wrongly refused, 0 errors; 33/33 seeded faults detected (27 + 6 inside the task 1.1 regime); verdict FAIL only from the 2 pre-existing near-tangent findings (D4, task 1.2).

Fresh server for the browser suite:
`NF_SETUP_TOKEN=ui-smoke-setup-token NF_DATA_DIR=$(mktemp -d) NF_LISTEN=127.0.0.1:8793 target/release/nf-server &`
then `NF_BASE=http://127.0.0.1:8793 node tests/ui_smoke.js`.
Deeper audit: `NF_SETUP_TOKEN=<24+ chars> NF_R6_SETUP_ARGS='-H X-NF-Setup-Token:<same>' NF_TEST_PORT=8831 bash tests/security_r6.sh`.
Shell scripts need Git Bash or WSL on Windows; say which suites you could not run and why. The totals will rise when
the work in section 8 lands: use the totals stated in the newest commit message.

GitHub runs the same checks on every push in two jobs: `software` and `geometry-validation` (the quick corpus). Both
passed on `ed4a5fb` and `ccaa7f2` (20 Nov 2026). The full corpus is not yet run by CI (work plan task 1.8).

---

## 7. Done and pushed (as of commit `9fbbfe4`)

- Five part families; engine 0.4.0: set-on joint for the specification's pairing (each part on its own contact
  surface), DXF quality gate (a failing file is withheld and the job rejected), independent 3D joint checks, shared
  sampling density per joint, results of older engines marked for recomputation.
- Three wrong-part defects found by independent review and fixed: header opening frame (002), opposite marking sides
  (003), set-on joint (004, partly).
- English/Japanese interface with redesigned job view; number fields accept only a plain decimal number (a typed
  `1,5` used to become 15 — critical defect BH-001, fixed).
- Desktop mode; Windows installer, portable zip, Linux package; `--help`, `--version`, unknown options refused.
- Offline check: API, browser and desktop suites pass with the network removed.
- Stress test on engine 0.4.0, all six levels: no server errors; after 25 forced kills all acknowledged operations
  present, database consistent, audit chain intact, replays match. Weaknesses in section 9.4.
- Bug hunt: 18 defects; 15 fixed, 1 partly, 3 were owner decisions (now decided: section 2).
- Release-verification tool and first manifest (`docs/release/MANIFEST-16d97dc.md`): Linux checks 5 of 5 native;
  Windows checks not run (emulator could not start); the build is NOT bit-for-bit reproducible.
- Quality gate split into `software` and `geometry-validation`.
- Documents: readiness classifications, desktop key assessment, legal readiness list, Windows hand-off.

---

## 8. In progress in the cloud session — do not start these until their commits are on the branch

| Id | Work | Files being changed |
|---|---|---|
| A1 | LANDED 20 Nov (commit 2894cc5). Engine 0.4.1: DXF states the joint as declared; structured refusal codes (41 codes, `docs/backend/REFUSAL_CODES.md`); geometry proven unchanged against a 335-request snapshot | `crates/*`, `web/`, `tests/` |
| A2 | LANDED 20 Nov. Validation tool fix | `validation/` |
| A3 | LANDED 20 Nov. Second independent mathematical review | `docs/backend/REVIEW_B2_COMPUTATIONAL.md` |
| A4 | LANDED 20 Nov (merge b96a1de). PDF → DXF converter crate; see `docs/backend/PDF_TO_DXF.md` | `crates/nf-pdf` |
| A5 | Advertisement film, revision 3 (section 13) | `marketing/nexus-fabric/flagship3` |

The lead verifies each one (all suites) before pushing and will say when each has landed.

---

## 9. Open items

### 9.1 Mathematics (first review, `docs/backend/REVIEW_B_COMPUTATIONAL.md`; scored 6/10)

| Item | Subject | State |
|---|---|---|
| RB-001 | Set-on joint | Partly: the specification's pairing works; a set-on with the saddle on the branch outside surface needs wall thickness |
| RB-002 | Sign of the offset δ (handedness) not stated in DXF or result | Open |
| RB-003 | Rolling axis and seam edges not in the DXF; frames differ between families | Open |
| RB-004 | Sampling density | Partly: maximum chord error 0.01 mm — an implementation value with no engineering source |
| RB-005 | Reported error must be a true upper bound | Partly: checked at five points; item D4 open |
| RB-006 | Checks independent of the construction | Partly |
| RB-007 | Angular datum and direction | Open |
| RB-008 | DXF contour details (kerf statement, outer profile as one closed polyline) | Partly |
| RB-009 | Plate origins differ between families and are unstated | Open |
| RB-010 | An "unknown" surface switches off the radius checks; cone, reducer and shell have no surface attribute | Open |
| RB-011 | Validation tool | Open: see 9.2 |

### 9.2 Independent geometry validation
**Updated 20 Nov, after A2 landed:** the validation tool was corrected (scaled patterns compared as curves; exact
vertex-to-reference distance and on-surface test for near-tangent joints; coverage check; new planted faults, all
detected). Quick run on engine 0.4.0: 155 cases, 93 agree, 62 correctly refused, 0 disagree, exit 0 — confirmed by the
lead on a clean packaged build. Still open: the full corpus has not been re-run; D4 was not reproduced in the quick
run and is unmeasured on the full corpus; one scale tolerance is now 2e-9 relative (was 1e-11 pointwise) — decide
whether to tighten; two scaled cases pass at 94 % and 99.6 % of their limit, so watch the GitHub run. The second
computational review is on the branch: `docs/backend/REVIEW_B2_COMPUTATIONAL.md` (8/10 and 8/10; B2-001 high:
the radius rule accepts a development radius far smaller than the contact radius; B2-002 medium: polylines labelled
exact with zero deviation; B2-003 … B2-007 low).

### 9.3 Security
- Deeper audit, 5 failing checks: V-45 the content policy allows inline styles; V-47 a request Origin with a non-web
  scheme, and one with the wrong scheme, are not refused (2 checks); V-44 `deploy/install.sh` pipes a download into a
  shell and builds as root (2 checks).
- Open high findings: V-21 the SQLite database is not encrypted (only stored files are, with AES-GCM); V-22 key
  management (desktop mode keeps the key beside the data); V-23 no multi-factor sign-in or account recovery.
- Architecture review items RA-001 … RA-017 (`docs/backend/REVIEW_A_ARCHITECTURE.md`, scored 5/10) are mostly open,
  among them RA-001 slow-connection exhaustion, RA-002 sign-in race, RA-003 audit truncation, RA-007 no recompute
  after an engine upgrade.

### 9.4 Stress findings still open
- Connection flood: about 640 held connections stopped the health request for 10 s; recovered when they closed.
- Full disk: writes fail with a clear error and reads continue, but a restart on the still-full disk did not succeed.
- Listing endpoints and audit verification grow with the number of jobs (54 ms → 1.6 s at 20,000 jobs).
- Two malformed requests are held until the client gives up instead of being refused at once.

### 9.5 Other
- BH-014 partly: the form accepts some values the engine refuses (the limit comes from sampled geometry); A1 gives
  the refusal a code and the field.
- Leftover automated-intelligence wording (task D2). In `web/i18n.js`: `adm.planned.4` (automated-reading/model configuration planned); drawing
  reading announced as planned in `doc.accepted`, `req.hint`, `ws.planned.lead`, `ws.planned.1` … `ws.planned.5`
  (drawing intelligence, OCR, dimension and symbol recognition) and `math.planned.1`. Keep the statement
  "EV-1 (not independently reviewed)": it is a disclosure about the review status of the algorithms. Also the
  old positioning line in `docs/project/MASTER_GOAL.md`, and the older films.
- Packages do not yet ship third-party licence texts (102 libraries, all permissive: MIT, Apache-2.0, BSD, Zlib,
  Unicode, Unlicense, BSL).
- No licence terms: packages say "All rights reserved. Evaluation use only — terms to be provided by the owner."
- Japanese wording has not been reviewed by a native-speaking fabrication engineer.
- An intermittent timing fault in one browser check was seen twice in total; not explained.

### 9.6 Readiness classifications (`docs/project/READINESS.md`)

| Classification | Status |
|---|---|
| Software verified | Holds for `cd452de` |
| Security reviewed | Does not hold (9.3) |
| Geometry verified | Does not hold (9.2) |
| Fabrication reviewed | Does not hold: 0 of 56 engineering statements reviewed by a fabrication engineer |
| Manufacturing approved | Does not hold and cannot: approval is not implemented |

Desktop product: "restricted deployment" until the database is encrypted with operating-system protected keys and an
organisation-held recovery key.

---

## 10. Tasks for the CLI — replaced

Replaced on 21 Nov 2026 by `docs/project/CLI_WORK_PLAN.md` (the CLI's full work plan, with the status of every task
kept current in it). Where the old items went:

| Old item | Now |
|---|---|
| B1 (review B2 findings) | done in engine 0.4.2 (`ed4a5fb`); remainder in 1.2 and 1.6 |
| B2 (conventions in the DXF and the result) | 1.3 and 1.4 |
| B3 (validation, D4, chord limit) | 0.3, 1.2, 1.6, 1.8 |
| C1 (five failing audit checks) | 2.1 |
| C2 (desktop security wording) | 2.4 |
| C3 (database encryption design) | 2.5 |
| C4 (stress findings) | 2.3 |
| D1 (third-party notices) | done (`b368d9a`) |
| D2 (banned-word test, leftover automated-intelligence wording) | wording removed (`9169305`); the test is 3.3 |
| D3 (Windows native verification) | 4.2 (real Windows PC only) |
| D4 (release-candidate report) | 4.4 |
| E (PDF → DXF in the product) | 3.1 |
| F (execution and scale) | 3.4 |
| G (later) | phase 5 (ask the owner first) |

---

## 11. Windows verification (task D3) — on a real Windows PC only

You verify and report; you do not change product code. Write results to
`docs/release/WINDOWS_NATIVE_<short commit>.md`, screenshots under `docs/release/windows-native/`; commit only files
under `docs/release/`.

1. Record the full commit hash; Windows edition and build, CPU, RAM.
2. Build natively (Rust stable, NSIS; `packaging/build_all.sh` under Git Bash or its commands). Record tool versions,
   each command and exit code. If the build fails, stop and report the full error.
3. For each artefact: name, size, SHA-256 (`Get-FileHash <file> -Algorithm SHA256`).
4. Version agreement: installer file name, `Cargo.toml`, `nexus-fabric.exe --version`; record the engine identifier.
   `--help` prints and exits 0; an unknown option exits non-zero and starts nothing.
5. Install as a normal user. Record every Windows warning word for word (SmartScreen, antivirus). Confirm program
   folder, Start-menu shortcut, optional desktop shortcut, entry in "Installed apps".
6. First run from the Start menu: console shows a local address; the browser opens the first-run screen; create the
   administrator; sign in; switch English ↔ Japanese. Note the data folder.
7. Work test: one job of each of the five families with the example values; pattern, Gates, Checks, AMOEBA Control
   Centre; download a DXF and open it in a viewer; record a correction on the header-with-branch job; run the replay
   audit. Type `1,5` in a dimension field: it must be refused, not turned into 15.
8. Offline: disconnect every network, start again, compute and download. Everything must work.
9. Start it twice: the second copy must refuse and say why.
10. Close the console window, start again: data still there, audit check intact. Stop once with Ctrl+C.
11. Upgrade over an older installer if one exists (job survives, version changes); otherwise "not run".
12. Uninstall: program files, shortcuts and the installed-apps entry gone; data folder kept (test "also delete my
    data" too if time allows).
13. Portable zip: unpack elsewhere, `--version`, one start.

One row per step: passed / failed / not run, with exact evidence. Not covered: code signing (files are unsigned, so
Windows warns), macOS (no package), engineering correctness.

---

## 12. PDF → DXF (owner's feature)

**Meaning:** a user uploads a PDF drawing and gets a DXF of what is drawn in it. It is a drawing conversion, NOT a
flat pattern: a drawing is not necessarily to scale and dimension text is not interpreted as geometry.

**Converter (A4, in progress):** crate `crates/nf-pdf`, binary `nf-pdf2dxf`.
- Vector PDF pages only: paths (lines, rectangles, Bézier curves flattened within a chord tolerance), transforms,
  form objects, page rotation and crop box; dashed and filled paths on their own layers; text best-effort.
- The user must give the scale (millimetres per PDF point, or a drawing scale such as 1:10). No scale → refused.
- Refused with a specific code: encrypted PDF; scanned or image-only page; page out of range; malformed file; any
  limit exceeded.
- Output: R12 DXF, layers `PDF_STROKE`, `PDF_DASHED`, `PDF_FILL`, `PDF_TEXT`, and `NF_NOT_RELEASED` carrying
  "CONVERTED FROM PDF - NOT VERIFIED - NOT RELEASED"; 999 metadata (source hash, page, scale, units, tolerance,
  converter version); a JSON report.
- Hard limits (input size, decompressed size and ratio, objects, operators, segments, nesting, time), never panics,
  a mutation fuzzer of at least 200,000 cases; no automated drawing reading, no OCR, no raster tracing; deterministic.

**Integration (task E):**
- Server: an endpoint that converts an uploaded PDF document by launching `nf-pdf2dxf` as an isolated child (no
  inherited environment, temporary working directory, no network, time and memory limits; state honestly what is not
  isolated on Windows), stores the DXF and the report as documents, records the conversion (source hash, page, scale,
  converter version) and writes an audit entry.
- Interface: on Projects & Documents, a "Convert to DXF" action on a PDF asking for page and scale, showing the
  report, a preview and the download, in English and Japanese; labelled "Converted from PDF — not verified — not
  released"; never presented as a flat pattern.
- Tests: API, browser in both languages, every refusal, the limits, and the isolation itself.

Today: documents can already be uploaded on Projects & Documents (stored encrypted, never parsed).

---

## 13. The advertisement (stays with the cloud session; not coding work)

`marketing/nexus-fabric/flagship3`: 54 s, 1920×1080, English and Japanese, 120 BPM. Script in
`script/SCRIPT.md`: hook ("One wrong curve — and the steel doesn't fit."), "how do you know?", dimensions in / pattern
out, the five parts, self-checks, DXF written / read back / compared, AMOEBA demonstration, "What it cannot confirm,
it shows", "In English and Japanese. And it runs fully offline.", end card "NEXUS FABRIC — Engineering, Reimagined."
with "Engineering platform powered by AMOEBA." and "Request a demonstration".
- Voices synthesised locally (English: Kokoro `af_heart`; Japanese: VOICEVOX Nemo, on-screen credit required and
  shown). Both mixes 54.0 s, −14 LUFS. Nobody has listened to the audio.
- Revision 3 in progress: full motion graphics, Apple-launch-film craft with simplified interface pieces built from
  the product's own colours, labels and data, the actual software in two short moments only, no mention of automated intelligence anywhere.
- Before publishing: two independent critics must score it; the owner must listen to it; read the VOICEVOX Nemo
  terms and the Kokoro model terms; Japanese wording needs a native review. Older films (`film`, `launch30`,
  `notch30`, `promo30`, `flagship2`) carry claims of automated intelligence or older footage: do not publish them.

---

## 14. The live server

A Hostinger VPS (Ubuntu) serves `fabric.vsnexustechnologies.com` through Caddy from a service on `127.0.0.1:8792`.
It has its own CLI session, which updates it by pulling the branch, rebuilding the binary and restarting the service.
**Do not run `deploy/install.sh` there, do not probe it, do not push from it.** When it is next updated it needs
`NF_ALLOWED_HOSTS=fabric.vsnexustechnologies.com`. It still runs an older build.

---

## 15. Legal position (not legal advice — `docs/legal/LEGAL_READINESS.md`)

No software can be made "legally proof". What exists: truthful statements everywhere; permissive third-party
licences (notices not yet shipped — task D1). What only the owner can do: a lawyer admitted in Japan for the terms of
use, liability limits, privacy statement and the ownership status of the code; a trademark search for "NEXUS FABRIC"
and "AMOEBA"; the commercial model.

---

## 16. What only people can supply

- A fabrication engineer's review of the conventions, assumptions and a real cut part against a pattern.
- A lawyer's review of the terms.
- A run on a real Windows PC (section 11).
- Listening to the advertisement.
- A native-speaking engineer's review of the Japanese wording.
- Sample drawings, if drawing-related features are ever to be tested on real material.

---

## 17. How to report

After each task: what changed; the exact totals of every suite in section 6; every changed expectation; anything not
finished or uncertain. Update the document the task belongs to (defect register, review status, bug-hunt status,
vulnerability register, readiness). Tell the owner when you have pushed, so the cloud session can pull.
