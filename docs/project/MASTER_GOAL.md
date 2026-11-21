# Master project goal and release gates

Owner's goal (2026-11-19): finish NEXUS FABRIC as one integrated, verified software product. Work from the existing code, keep what works, fix what does not, test hard, and claim nothing the evidence does not support. Software only.

**Standard:** not "zero errors", which no finite test suite can show, but: zero known critical defects, zero known unresolved high defects, zero unexplained test failures, zero silent data corruption in tested workflows, independently verified engineering outputs, and failure that is safe and visible.

**Defining principle:** a drawing that looks right is not a drawing that is right. Geometry, numbers and exported files must be checked by something that does not share the construction being checked. (See Defect 002 in `docs/LEARNING_AND_STRESS_TESTING.md`: shared-construction checks passed a wrong part for weeks.)

**Order of priority:** data integrity and engineering correctness; DXF precision and geometric verification; backend reliability; computational correctness; security and fault tolerance; interface quality; performance; marketing.

## Release gates (all must hold; none holds yet)

| # | Gate | Evidence required | State on 2026-11-19 |
|---|---|---|---|
| G-R1 | Zero known critical or high defects | `docs/backend/DEFECT_REGISTER.md`, vulnerability register, UI critic report: none open | Open: audits in progress |
| G-R2 | All automated suites pass with nothing skipped | CI run of `.github/workflows/quality-gate.yml` | Suites pass locally; CI workflow not yet run |
| G-R3 | Every critical calculation independently verified | `validation/` results: 0 disagreements | In progress |
| G-R4 | Every DXF passes the DXF quality gate, read back by an independent reader | validation DXF checks; product marks failing output INVALID | Product re-reads with its own reader only; independent reader in progress |
| G-R5 | No critical security finding open | `docs/security/SECURITY_AUDIT_R6.md` | Audit in progress; 3 earlier high findings open |
| G-R6 | Recovery proven for tested interruptions | `docs/backend/STRESS_AND_RECOVERY_REPORT.md` | In progress |
| G-R7 | Documented performance limits and maximum tested workload | same report | In progress |
| G-R8 | No unexplained memory growth in endurance | same report; a 6–24 h run on a real server | Short run in progress; long run not done |
| G-R9 | Critical user journeys pass in the browser | `tests/ui_smoke.js` | 83 of 83 |
| G-R10 | Two independent reviewers per critical area, findings resolved | reviewer reports | UI and film: first round done, below bar |
| G-R11 | Documentation matches the product; limitations disclosed | feature matrix below, `docs/IMPLEMENTATION_STATUS.md` | Partly out of date |
| G-R12 | Fabrication-engineer review, authorised approval, independent mathematical review | standing validation boundary | Not started: 0 of 56 statements reviewed. Software work cannot close this gate. |

Until G-R12 holds, no output may be used for manufacturing, whatever the other gates say.

Readiness is stated per classification, never as one label: see `READINESS.md`.

## Capability matrix (what exists; evidence level EV-1 throughout)

| Capability | State | Notes |
|---|---|---|
| Flat patterns: cylinder shell, cone frustum, eccentric reducer, branch stub, header with one branch opening | Implemented, not fully verified | Independent validation in progress; header defect fixed today |
| DXF writing (R12 subset) with read-back comparison | Implemented, not fully verified | Read-back uses the product's own reader |
| Records, dependencies, obligations, corrections, replay, gates, audit chain, users and roles | Implemented, not fully verified | Under audit |
| Engineering workstation interface | Implemented; first independent review below bar | Fixes in progress |
| File upload and document storage | Implemented: documents are uploaded, stored encrypted and never parsed | Corrected 20 Nov 2026: an earlier version of this table said "not implemented" |
| DXF or drawing import (reading a drawing) | Not implemented | Standing rule: no parser is merged without an isolated, network-less, resource-limited process and fuzz tests |
| Drawing reading, 3D reconstruction, any AI component | Not implemented | No AI in the software (owner decision, 20 Nov). AI may never be the final authority for geometry |
| Plate division, seams, nesting, manufacturing-constraint evaluation, CNC or CAM hand-off | Not implemented | No CNC-readiness claim |
| Structural frames, trusses, structural analysis, code checks, STEP, IFC | Not implemented, not planned in the specifications | Would be a new product area needing its own specification and independent validation |
| Engineer approval | Not implemented | `APPROVED` stays unreachable by design |
| Multiple openings per plate, splines, blocks, 3D DXF | Not implemented | |
| Background job queue, cancellation, monitoring, alerting | Not implemented | Jobs run within the request |
| Backups | Outside the product | A daily backup with restore test runs on the owner's server |

## Work in hand and queued

In progress: backend audit; independent computational validation with benchmark corpus; stress, concurrency and fault-injection harness; security audit; interface defect fixes; launch-film revision.

Queued, in priority order: (1) one fix pass on critical and high defects from the four audits, each with a regression test; (2) DXF quality gate inside the product: an error-detection pass (open contours, duplicates, overlaps, self-intersections, wrong layer or units) and an INVALID state that blocks download; tolerance and precision specification; (3) property-based tests of geometric invariants; (4) error and integrity monitoring with an "Engineering integrity" page in the application showing real results only; (5) two independent reviewers (system architecture, computational engineering); (6) evidence-based release report and updated status documents.

Needs an owner decision before any work: DXF import; structural engineering; STEP or IFC. Each is new product scope, and DXF import is bound by standing safety rules. (No AI component: the software contains none — owner decision, 20 Nov; see above.)
