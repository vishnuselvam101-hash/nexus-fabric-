# Specification Roadmap

Version 0.1 · 18 November 2026

Order of development for the remaining specifications, with dependencies and review gates. Numbering follows the development sequence below (it differs from the list in master plan §20, which was unordered).

## Status (18 November 2026)

| Spec | Status | Outstanding before approval |
|---|---|---|
| 01 System Architecture | Draft 0.1 — awaiting review | Proposed amendments pending decision (none applied): twelve-subsystem table (Spec 02 §1.1); development-class labels (Spec 02 §14); eight origin classes and reading states in the Observation record (Spec 03 §14) |
| 02 Mathematical Engineering | Draft 0.1 — **provisional until independently reviewed** | Second-person derivation check; verification of eccentric reducer, branch and rectangular-to-round models against actual fabrication geometry; independent recomputation of [reference cases](../benchmark/MATHEMATICAL_REFERENCE_CASES.md); factory inputs FR-02-1…10; standards review. The four-class development scheme remains a **proposal**, not adopted in the master plan |
| 03 Drawing Intelligence and Ingestion | Draft 0.1 complete — awaiting review | Validation against authorised drawings (FR-03-1…12); native-speaker review of the vocabulary; standards N-01…N-08 and benchmarks T-01…T-21 in the [research register](../research/DRAWING_INTELLIGENCE_RESEARCH_REGISTER.md); Spec 05 decision on automatic acceptance of readings (DI-33) |
| 04–08 | Not started | — |
| 09 Research | Licensing review started | Quotations, counsel, patent search |
| 10 Benchmark | Plan drafted; mathematical cases and drawing-intelligence test plan drafted | Factory data and authorisation |

## Dependency order

```
01 Architecture ──┬─► 02 Mathematics ──┬─► 04 Geometry & Fabrication ─┬─► 05 Verification ─► 06 DXF/CAD Exchange
                  │                    │                              │
                  ├─► 03 Drawing Intelligence ─────────────────────────┘
                  ├─► 07 Security & Deployment
                  └─► 08 Product Requirements & UX   (consumes 02–07)

09 Competitor, Licensing & Patent Research   — runs in parallel from now; gates dependency adoption
10 Engineering Benchmark & Validation        — runs in parallel from now; finalised after 05
```

**Review gate (every document):** (1) author self-review against the master plan and Spec 01 requirement IDs; (2) independent technical review; (3) factory-engineer review where the document makes fabrication claims; (4) open items listed, each with an owner. A document is *Approved for implementation* only when every requirement in it has a named verification method. Implementation of a module starts only after its governing specifications pass the gate.

**Standing rule:** missing factory information is recorded as an open requirement (prefix `FR-`), never filled with an assumption.

---

## 02 — Mathematical Engineering Specification

- **Purpose:** define every equation, algorithm and numeric regime the UEM implements, with applicability limits.
- **Scope:** coordinate transforms and view projection; constraint formulation and solving; surface classification and development (cylinder, cone, oblique/eccentric cone, planar, piecewise/triangulated transitions); intersection curves (cylinder–cylinder, cylinder–cone, oblique cases); thickness/neutral-surface treatment; bend models; uncertainty propagation; 2D profile operations; error-bound derivations.
- **Dependencies:** 01.
- **Required research:** closed-form versus numeric development for eccentric and oblique reducers; interval methods for transcendental functions; conditioning of multi-view constraint systems; JIS conventions for dimensioning and tolerancing; published reference solutions for cross-checking.
- **Deliverables:** per-algorithm sheets (formulation, regime, bounds, validity range, failure outcomes, reference cases); notation and unit system; a definition separating *numerical error*, *measurement uncertainty* and *manufacturing tolerance*.
- **Acceptance:** each algorithm has ≥ 2 independent derivations or references; every formula dimensionally checked; reviewed by a qualified mathematician/engineer; no algorithm lacks a stated validity range.
- **Open requirements:** FR-02-1 which part families the factory actually produces, with frequency; FR-02-2 whether drawings dimension to inside, outside or mid-thickness; FR-02-3 customary tolerances; FR-02-4…10 as listed in Spec 02 §13.

## 03 — Drawing Intelligence and Ingestion Specification

- **Purpose:** define how files become typed observations.
- **Scope:** the four pipelines; observation schema; provenance classes; reconciliation rules (written vs measured vs scale); view detection; dimension-to-entity association; JA/EN vocabulary; AI-policy modes; sandboxing.
- **Dependencies:** 01; observation consumers defined in 02.
- **Required research:** PDF library evaluation; OCR/vision model evaluation on Japanese engineering drawings; vectorisation methods and their error behaviour; JIS drawing conventions (projection method, symbols, weld symbols); handwriting/stamp handling.
- **Deliverables:** schema; pipeline stage contracts; uncertainty model for scanned measurements; list of unsupported input conditions with required system behaviour.
- **Acceptance:** schema exercised against a sample of authorised drawings; every stage has metrics defined in Spec 10; every ambiguity class has a defined escalation path.
- **Open requirements:** FR-03-1 sample drawings (authorised); FR-03-2 proportion of vector PDF / scan / photo / DWG / DXF inputs; FR-03-3 typical scan quality; FR-03-4 first- or third-angle projection and house drawing standards; FR-03-5 customer restrictions on external AI; FR-03-6…12 as listed in Spec 03 §15.

## 04 — Geometry Reconstruction and Fabrication Specification

- **Purpose:** define reconstruction from observations and the shape-module contract, then the first shape modules.
- **Scope:** constraint-system construction; candidate interpretation handling; shape-module contract; initial modules (cylinder, concentric/eccentric reducer, branch/saddle, plate; rectangular-to-round as approximate class); plate division, seam placement, allowances; strategy optimisation and cost terms; kernel-port contract.
- **Dependencies:** 01, 02, 03; kernel evaluation results (ADR-003).
- **Required research:** factory fabrication practice (rolling, press-braking of cones, segment counts, seam conventions); stock sizes; weld preparation rules; kerf by process; existing manual methods used by the factory's engineers.
- **Deliverables:** module specifications with applicability ranges and reference examples; ProcessData schema and calibration procedure; optimisation formulation with configurable weights.
- **Acceptance:** each module reproduces factory-approved historical jobs within agreed criteria (requires FR data); each process parameter is either calibrated with evidence or flagged *assumed*; reviewed by a factory engineer.
- **Open requirements:** FR-04-1 machines and processes (laser/plasma/gas, rolls, press brake) with capabilities; FR-04-2 materials and thickness range; FR-04-3 seam and division rules; FR-04-4 stock plate sizes; FR-04-5 historical jobs with approved developments.

## 05 — Verification, Accuracy and Uncertainty Specification

- **Purpose:** define what evidence is required for release and how it is produced independently.
- **Scope:** release gates as executable definitions; independence classes; per-quantity verification methods; tolerance budgets; uncertainty sources and combination; decision rules for conformity under uncertainty (ISO 14253-1 approach, GUM framework — standards to be obtained and read, not paraphrased from memory); override policy; evidence records.
- **Dependencies:** 01, 02, 04 (and 03 for input uncertainty).
- **Required research:** GUM / JCGM 100, 101, 106; ISO 14253-1; applicable JIS general tolerances for fabricated/welded structures; common-mode failure analysis for our Z2/Z3 pairings.
- **Deliverables:** gate catalogue; verification method per shape module; tolerance-budget template; status decision table (Approved / Review Required / Rejected).
- **Acceptance:** every critical requirement in 02–04 maps to a gate and method; seeded-fault tests demonstrate that each gate detects the fault class it exists for; passing tests is explicitly not treated as proof of reliability — evidence classes and their limits are stated.
- **Open requirements:** FR-05-1 customer tolerance specifications; FR-05-2 inspection equipment and its uncertainty; FR-05-3 who may approve and whether preparer/approver separation is required.

## 06 — DXF and CAD Exchange Specification

- **Purpose:** define outputs and exchange formats precisely.
- **Scope:** DXF version(s), header variables and units, permitted entities, layer scheme and per-customer mapping, arc/polyline/spline policy, numeric formatting, validation and re-import comparison, STEP export, DWG/DXF import coverage, package manifest.
- **Dependencies:** 01, 04, 05; licensing review (DWG path).
- **Required research:** Autodesk DXF reference; behaviour of the factory's CAM/nesting software on entity types and layers; independent parser candidates.
- **Deliverables:** writer specification; validator specification; profile format; conformance test files.
- **Acceptance:** generated files load unmodified in the target CAM systems (requires access); round-trip comparison within defined bounds; every dropped/approximated entity on import is reported.
- **Open requirements:** FR-06-1 CAM/nesting software names and versions; FR-06-2 current layer conventions; FR-06-3 required DXF version; FR-06-4 whether splines/ellipses are accepted by the cutting machines.

## 07 — Security and Deployment Specification

- **Purpose:** define security controls and deployment topologies.
- **Scope:** threat model; tenant isolation; identity, roles, MFA; key management; audit log; file sandboxing; AI-policy enforcement; single-node / cluster / cloud topologies; offline updates; backup, recovery, retention and deletion.
- **Dependencies:** 01 (ADR-006, ADR-008).
- **Required research:** customer security requirements in Japanese manufacturing supply chains; applicable Japanese data-protection and export-control considerations (legal input); sandboxing technologies per OS.
- **Deliverables:** threat model; control catalogue mapped to tests; deployment guides outline; retention policy options.
- **Acceptance:** every threat has a control and a verification; deployment verified with no outbound network; external review planned before pilot.
- **Open requirements:** FR-07-1 factory IT environment (OS, hardware, network policy, GPU); FR-07-2 customer NDAs and data-handling clauses; FR-07-3 retention periods.

## 08 — Product Requirements and User Experience Specification

- **Purpose:** define functionality and workflows for the six workspaces, traceable to the engineering specifications.
- **Scope:** user roles and journeys; workspace requirements; review and approval UX (making wrong extractions visible); JA/EN localisation including terminology; offline behaviour; mobile/tablet scope; first-release feature cut.
- **Dependencies:** 01–07.
- **Required research:** observation of the factory engineers' current workflow; terminology glossary (JA/EN); usability testing of the review workspace.
- **Deliverables:** requirement list with IDs, each linked to a test; workflow diagrams; glossary; first-release scope.
- **Acceptance:** each requirement traceable to a master-plan objective and a verification method; reviewed with factory engineers.
- **Open requirements:** FR-08-1 current workflow and time per job type; FR-08-2 roles in the engineering office; FR-08-3 devices in use.

## 09 — Competitor, Licensing and Patent Research

- **Purpose:** evidence base for positioning, dependency adoption and IP strategy.
- **Scope:** hands-on competitor evaluation (Unfoldo, CUTL, FastSHAPES/FastCAM, Japanese CAD/CAM systems) on the benchmark jobs; completion of the [licensing review](../research/DEPENDENCY_LICENSING_REVIEW.md); prior art and patent landscape for the candidate innovations; FTO questions for counsel.
- **Dependencies:** none to start; benchmark jobs (10) for the hands-on comparison.
- **Required research:** as scope; commercial quotations (GeometryFactory, commercial kernels, ODA).
- **Deliverables:** comparison report with reproducible test records; licence decision inputs; prior-art list; questions for patent counsel.
- **Acceptance:** every competitor claim sourced and dated or reproduced by test; no novelty or FTO conclusion stated without counsel.
- **Open requirements:** FR-09-1 budget for commercial licences and trials; FR-09-2 legal counsel engagement; FR-09-3 which Japanese systems the factory uses or has evaluated.

## 10 — Engineering Benchmark and Validation Specification

- **Purpose:** turn the [benchmark plan](../benchmark/ENGINEERING_BENCHMARK_PLAN.md) into a binding specification with thresholds.
- **Scope:** dataset governance; job schema; metrics; acceptance thresholds; shadow-pilot protocol; physical trial protocol.
- **Dependencies:** plan exists now; thresholds need 05 and factory tolerances.
- **Required research:** measurement methods for fabricated parts; statistical sample sizes.
- **Deliverables:** benchmark specification; harness requirements; reporting templates.
- **Acceptance:** thresholds agreed with the factory; ground-truth procedure documented; held-out set defined.
- **Open requirements:** FR-10-1 written authorisation to use drawings; FR-10-2 historical jobs with approved outputs; FR-10-3 capacity for physical trials.

---

## Recommended immediate sequence

1. Independent review of 02 (provisional) and collection of factory answers — in progress, owner: project lead.
2. **04 Geometry Reconstruction and Fabrication Specification** (next), then 05 and 06.
3. 07 alongside 04–06; 08 last.
4. No platform implementation before 04–06 are defined. First software milestone: one narrow vertical slice — one fabrication geometry, one documented input workflow, independently verified development, and a DXF accepted by the factory's CAM system.
