# Implementation Status

Updated 18 November 2026 · implementation session 2 · engine `nf-core 0.1.0 / uem 0.1.0`

Four different things are tracked separately and must not be confused:

| Column | Meaning |
|---|---|
| **Code** | NOT STARTED · IN DEVELOPMENT · IMPLEMENTED · TESTED (automated tests exist and pass) · INTEGRATED (reachable through API and UI) · BLOCKED |
| **Engineering validation** | Whether a qualified engineer has reviewed it / it has been checked against factory evidence ([evidence levels](architecture/EVIDENCE_LEVELS.md)) |
| **Release ready** | Fit for use on real work |

**Nothing in this repository is RELEASE READY. Nothing has engineering validation (everything is EV-1). No output may be used for manufacturing.**

## Summary

A launchable single-binary application exists: sign in, create projects, upload documents (stored encrypted, not interpreted), enter geometry parameters for four part families, obtain a flat pattern with independent verification checks, a DXF, a gate evaluation and a replay audit — every result ends in REVIEW REQUIRED or REJECTED. `APPROVED` cannot be reached.

## Added in session 2

- **Assembly job:** header shell (with the branch opening as an inner profile) plus set-on branch, as two parts with one joint.
- **Interface verification:** the branch cut and the header opening, each folded back to 3D from its own pattern, must be the same curve.
- **Typed dependency edges** per parameter (derivation, interface, common cause) and **propagation** in three directions.
- **Corrections:** an engineer's correction never edits a job; it is an attestation that creates a new job, supersedes the old one and raises **scrutiny obligations**. Downstream and lateral obligations are discharged only by passing evidence in the recomputed job; common-cause obligations stay open until an engineer records a decision with a reason.
- **Gates G18 and G19** are evaluated when a result is read; superseded and rejected results have their DXF withheld.
- **Replay audit** now runs in a separate operating-system process that receives only the request.

The dependency **rule table is provisional** (EV-1; 0 of 56 statements reviewed). The mechanism is tested; whether the rules are the right ones is an engineering question that remains open.

## Mathematical engine (`crates/uem`)

| Feature | Code | Engineering validation | Notes |
|---|---|---|---|
| Cylindrical shell development | INTEGRATED | None (EV-1) | Reference-surface radius entered directly |
| Mitre cut height | TESTED | None | Library function only; not in UI |
| Right cone frustum (concentric reducer) | INTEGRATED | None | Near-cylindrical cones refused (apex-free method missing) |
| Oblique cone frustum (eccentric reducer) | INTEGRATED | None | Numerical quadrature, estimated error; equal radii with offset refused |
| Cylinder-on-cylinder branch stub | INTEGRATED | None | Tangent / equal-diameter / partial intersections refused; angle limit is an implementation choice |
| Press-brake bend allowance / deduction | TESTED | None; no calibrated parameter exists | Library only; no default K-factor |
| Profile validation (closure, self-intersection, orientation) | INTEGRATED | — | binary64, not exact predicates |
| Independent verification checks per family | INTEGRATED | None | Different formulation, same crate (class I-F only) |
| Seeded-fault tests for verifiers | TESTED | — | Wrong radius reference, wrong offset, inside/outside header, scale |
| Header shell with branch opening (inner profile) | INTEGRATED | None | Seam fixed opposite the branch; no clearance rule |
| Interface check: branch cut ≡ header opening | INTEGRATED | None | Seeded faults: inside/outside header, angle mismatch |
| Branch on cone/sphere; several branches | NOT STARTED | | |
| Rectangular-to-round, irregular transitions | NOT STARTED | | |
| Thickness / neutral surface, roll and weld allowances, kerf | NOT STARTED | | BLOCKED on calibrated process data for real use |
| Constraint solving, frames/view projection | NOT STARTED | | |
| Interval-certified bounds, exact predicates, multi-precision | NOT STARTED | | Error figures are estimates and labelled so |
| Uncertainty and tolerance analysis | NOT STARTED | | |
| Plate division and seam optimisation | NOT STARTED | | BLOCKED on factory rules for real use |

## AMOEBA core (`crates/nf-core`)

| Mechanism | Code | Notes |
|---|---|---|
| Immutable content-addressed records, seven families | INTEGRATED | Identity = SHA-256 of canonical JSON; exact float round-trip enforced by test |
| Typed dependency edges | INTEGRATED | Derivation, interface and common-cause edges for the header/branch assembly; single-part jobs have one request claim |
| Project snapshot with predecessor (engulfment) | INTEGRATED | Enumeration of source content for coverage NOT STARTED |
| Membrane admission (content-based type, size limit) | INTEGRATED | No sandboxed parsing yet — nothing is parsed |
| Activities with logical work units | INTEGRATED | Costs are fixed declarations, not calibrated |
| Release closure and closure hash | INTEGRATED | Unrelated additions do not change the hash (tested) |
| Gate evaluation and release state | INTEGRATED | G01, G02 (assemblies), G05, G06, G08, G10, G11, G14, G15, G16, G18, G19; G18/G19 evaluated at read time; unevaluable gates never PASS |
| Replay audit | INTEGRATED (separate process, same machine and build) | Separate build or machine NOT STARTED |
| Round scheduler, signals, budgets | NOT STARTED | Jobs run synchronously in one pass |
| Exploration probes, hypothesis-driven investigation | NOT STARTED | |
| Dependency propagation, scrutiny obligations (G18) | INTEGRATED for parameter corrections | **Rule table provisional, unreviewed (0/56)**. Not yet raised by drawing revisions, verification failures or algorithm faults |
| Corrections with supersession; obligation resolution by recorded decision | INTEGRATED | Reason mandatory; resolved obligations are immutable |
| Coverage ledger (G17 deferred) | NOT STARTED | |
| Lanes / overlays, quarantine, retirement | NOT STARTED | All algorithms marked `not_production_qualified` |
| Self-consistency audits A1–A4 | NOT STARTED | |
| Strategy profiles, stopping rules, residual-uncertainty statement | NOT STARTED | |
| Engineer approval (G16) | NOT STARTED | Deliberately absent: no path to APPROVED |

## Drawing intelligence

| Feature | Code | Notes |
|---|---|---|
| Upload, hashing, encrypted storage, type detection (PDF, PNG, JPEG, TIFF, DXF, STEP) | INTEGRATED | |
| Everything in Specification 03 beyond admission (page classification, OCR, dimensions, symbols, views, projection, reconciliation, confirmation workflow) | NOT STARTED | Technology unselected; all candidate dependencies unevaluated |
| DWG, JWW, IGES adapters | NOT STARTED | Licensing unresolved |

## CAD and manufacturing output (`crates/nf-dxf`)

| Feature | Code | Notes |
|---|---|---|
| DXF writer (R12 subset: LINE, ARC, POLYLINE, layers) | INTEGRATED | Units in a comment only; **no CAM system has been tested** |
| Independent DXF reader and re-import comparison | INTEGRATED | |
| Entity-to-source traceability | IN DEVELOPMENT | Pattern → DXF record relation exists; per-entity map NOT STARTED |
| 3D visualisation | NOT STARTED | 2D pattern preview only |
| Bend lines, marking layers, inner profiles, kerf | NOT STARTED | |
| Plate division, seam planning, nesting | NOT STARTED | |
| STEP export | NOT STARTED | No kernel selected |

## Backend (`crates/nf-server`)

| Feature | Code | Notes |
|---|---|---|
| Authentication (Argon2id), sessions, logout | INTEGRATED | No rate limiting, MFA or password change |
| Roles: admin / engineer / viewer | INTEGRATED | Enforced server-side (tested) |
| Projects, documents, calculations, DXF export API | INTEGRATED | |
| Audit log | INTEGRATED | Stored in the database, not hash-chained |
| Blob encryption at rest (AES-256-GCM) | INTEGRATED | Interim single local key file |
| Database encryption at rest | NOT STARTED | SQLite file is plaintext — see Release Readiness |
| TLS | NOT STARTED | Must run behind a TLS-terminating proxy |
| Job orchestration / queue, notifications | NOT STARTED | |
| Tenant isolation, per-project permissions | NOT STARTED | Single tenant |
| Backup / restore, retention | NOT STARTED | |
| Sandboxed native workers | NOT STARTED | No native geometry or parsing code is run yet |

## Frontend (`web/`)

| Area | Code | Notes |
|---|---|---|
| Sign-in and first-run setup | INTEGRATED | |
| Dashboard | INTEGRATED | Live counts and recent jobs |
| Projects & Documents | INTEGRATED | Upload, hashes, snapshot id |
| Mathematical Engineering | INTEGRATED | Four families; results page with pattern, checks, gates, records |
| Verification & Quality | INTEGRATED | Job states and gate tables; approval absent |
| Manufacturing | INTEGRATED (export list only) | Planning features labelled not implemented |
| AMOEBA Control Centre | INTEGRATED | Live record and activity counts; lists implemented / missing mechanisms |
| Administration | INTEGRATED | Users, audit log |
| Engineering Workspace | NOT STARTED | Page states what is missing |
| Light / dark themes, tablet layout | INTEGRATED | Browser-tested at 1360 px and 820 px |
| Japanese localisation | NOT STARTED | |
| Framework / component system | — | Dependency-free ES modules for now; decision open |

## Tests (all passing on 18 November 2026)

| Suite | Count | Covers |
|---|---|---|
| `cargo test -p uem` | 17 | Reference cases; limits; profile checks; header opening; interface check; verifier seeded faults |
| `cargo test -p nf-dxf` | 2 | Round trip; corruption, truncation, foreign entity, layer change |
| `cargo test -p nf-core` | 12 | Record identity and storage round trip; closure; release-state rules; job determinism; assembly structure; propagation reach, locality and determinism; discharge rules |
| `cargo test -p nf-server` | 3 | Encryption, password hashing, membrane |
| `tests/e2e_smoke.sh` | 61 checks | Fresh server: setup, auth, roles, upload admission and encryption, five job families, reference values through the API, DXF export, separate-process replay, correction → supersession → obligations → resolution, withheld files, audit |
| `tests/ui_smoke.js` | 1 flow | Real browser: setup, single-part job, assembly job, correction, resolving an obligation, superseded job, all areas, no script errors, no overflow at tablet width |

Not yet present: performance tests, security tests beyond the above, failure-recovery tests, frontend unit tests, CAM acceptance tests.

## Defects found and fixed this session

1. Stored records failed their identity check after a JSON round trip (float parsing not exact) — fixed by exact float round-tripping; regression test added.
2. A slow earlier navigation could overwrite a newer page in the web client — fixed with render sequencing.
