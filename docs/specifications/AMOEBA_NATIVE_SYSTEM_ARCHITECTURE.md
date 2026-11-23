# AMOEBA-Native System Architecture

Version 0.1 (draft for independent architecture review) · 18 November 2026
Relates to: [Master Plan](../MASTER_PLAN.md) · [Spec 01](01_SYSTEM_ARCHITECTURE_SPECIFICATION.md) · [Spec 02](02_MATHEMATICAL_ENGINEERING_SPECIFICATION.md) (provisional) · [Spec 03](03_DRAWING_INTELLIGENCE_AND_INGESTION_SPECIFICATION.md) (provisional)
Supporting notes: [EOS](../architecture/amoeba/ENGINEERING_ORGANISM_STATE.md) · [Mechanisms](../architecture/amoeba/MECHANISMS.md) · [Coverage Model](../architecture/amoeba/COVERAGE_MODEL.md) · [Migration](../architecture/amoeba/EXISTING_ARCHITECTURE_MIGRATION.md) · [Proposed ADRs](../architecture/amoeba/PROPOSED_ADRS.md) · [Benchmark](../benchmark/AMOEBA_ARCHITECTURE_BENCHMARK.md) · [Research](../research/AMOEBA_BIOLOGICAL_AND_COMPUTATIONAL_RESEARCH.md)

---

## 0. Basis and status of this document

1. **No prior definition of AMOEBA existed in this repository.** This document defines it from the task instruction alone (the listed lifecycle processes, mechanisms, safety boundaries and coverage requirements). "AMOEBA" is treated as a name; no expansion of it as an acronym is assumed. If an earlier AMOEBA concept exists elsewhere, it must be reconciled with this draft before review.
2. **Three kinds of statement are kept apart throughout** and tagged:
   - **[E] Established engineering principle** — widely used practice (content addressing, event logs, sandboxing, budgets, supervision trees, work scheduling).
   - **[P] Proposed architecture** — a design choice of this document, unvalidated.
   - **[H] Unverified hypothesis** — a claim that the design will have a benefit, to be tested by the [benchmark](../benchmark/AMOEBA_ARCHITECTURE_BENCHMARK.md).
3. **Biology is a source of design vocabulary and of some design ideas, not of validation.** Nothing here claims that the system models, simulates or is validated by cell biology, and no biological novelty is claimed. Where a computational mechanism deliberately *departs* from its biological namesake this is stated (the most important: biological digestion destroys what is engulfed; AMOEBA never destroys evidence).
4. **Honest scope assessment.** Most of AMOEBA's lifecycle maps onto structures already specified (trust zones, truth graph, job orchestration, verification, release gates). The genuinely new architecture is: (a) whole-project engulfment as an immutable snapshot, (b) one unified state object, (c) a **coverage ledger** with a release gate, (d) a **bounded adaptive exploration** scheduler driven by recorded signals, (e) quarantine and controlled termination as first-class operations, (f) algorithm lifecycle lanes. Sections below say which is which, so that the naming does not disguise how much is new.
5. Requirement IDs: `AMB-nn`. Existing `ARCH/MATH/DI` requirements remain in force; none is weakened.

---

## 1. Definition

**AMOEBA is the governing computational architecture of NEXUS FABRIC** [P]: every activity of the engineering lifecycle — ingestion, interpretation, reconstruction, calculation, verification, review, release, revision — is an AMOEBA *process* that

1. reads from and appends to a single versioned state, the **Engineering Organism State (EOS)**;
2. is started, budgeted, observed and terminated by the **AMOEBA runtime**;
3. is permitted only the record types and transitions its **trust zone** allows;
4. is accounted for in the **coverage ledger**.

There is no engineering computation outside an AMOEBA process and no engineering fact outside the EOS (AMB-01). AMOEBA is therefore not a module beside the pipeline of Specs 01–03: it replaces "pipeline" as the organising concept. The earlier stage diagram (Spec 01 §1.3) becomes one typical *path* through the lifecycle rather than the architecture itself.

What AMOEBA does **not** change: the trust zones Z0–Z4, the rule that automated drawing reading produces only candidates, deterministic calculation authority, independent verification, human release authority, immutability and reproducibility. These are the constraints *within which* AMOEBA operates; where an AMOEBA mechanism and one of these rules could conflict, the rule wins (AMB-02).

## 2. Structure

```
                        ┌──────────────────────── AMOEBA runtime ────────────────────────┐
  files ─► MEMBRANE ─►  │  ENGULF ─► DIGEST ─► EXPLORE ⇄ RELATE ⇄ RECONCILE ─► BUILD ─►  │ ─► RELEASE
          (Z0 gate)     │  (snapshot) (decompose) (probes)  (links)  (conflicts)  (Z2)   │   (Z3 + Z4)
                        │        ▲           signals / budgets / coverage                │
                        └────────┼───────────────────────────────────────────────────────┘
                                 ▼
                 ENGINEERING ORGANISM STATE (EOS): snapshot · records · regions · issues ·
                 coverage ledger · process ledger · signal log · heads      (append-only, versioned)
```

| Element | Role | New or existing |
|---|---|---|
| **Membrane** | The only entry point for external bytes; validation, limits, sandbox admission (Spec 03 §1.1) | Existing, renamed |
| **EOS** | The whole project as one versioned, traceable structure | Truth graph (Spec 01 §6) extended — §3 |
| **Processes** | Units of work with declared inputs, permitted outputs, zone, budget, termination condition | Job steps (Spec 01 §2), generalised |
| **Runtime** | Scheduler, budget manager, supervisor, signal dispatcher | Job orchestration (Spec 01 §2), extended |
| **Signals** | Typed events derived from EOS changes; the only inter-process coordination | New [P] |
| **Coverage ledger** | Accounting of what has and has not been examined | New [P] |
| **Lanes** | experimental / candidate / production for every algorithm, model, rule and template | Spec 01 §7 "experimental versions", formalised |

## 3. Engineering Organism State (EOS)

Full definition: [ENGINEERING_ORGANISM_STATE.md](../architecture/amoeba/ENGINEERING_ORGANISM_STATE.md). The name *Engineering Organism State* is retained as proposed. A more conventional name would be "versioned engineering project graph"; the content is the same.

The EOS is the tuple

`EOS = (Snapshot, Records, RegionTree, Issues, CoverageLedger, ProcessLedger, SignalLog, Heads)`

| Component | Contents |
|---|---|
| **Snapshot** | Immutable set of all source artifacts of the project at engulfment (hash tree), with declared roles, tenant, automated-reading policy. Later uploads create a new snapshot linked to its predecessor |
| **Records** | All truth-graph records of Spec 01 §6.1 and observations of Spec 03 §6: source documents and revisions; pages, views, regions; text, symbols, dimensions; geometric observations and reconstructed objects; constraints and relationships; materials and manufacturing information; uncertainties; calculations; evidence; decisions; approvals |
| **RegionTree** | Containment hierarchy: project → document → sheet/page → region (title block, view, table, notes) → element (text group, entity, symbol) — and, on the engineering side, project → assembly → part → surface/feature → pattern → profile |
| **Issues** | Open and closed conflicts, gaps, ambiguities, unresolved questions, quarantines — each with affected records and state |
| **CoverageLedger** | §6 |
| **ProcessLedger** | Every process instance: type, version, lane, inputs, budget, outcome, termination reason |
| **SignalLog** | Ordered log of signals (derived, replayable) |
| **Heads** | The small mutable part: current revision pointers, job status |

Properties:

- **AMB-03 Versioned.** Every EOS state has an identifier (hash over the record set and heads). Any statement about the project ("coverage is complete", "these gates pass", "this was approved") refers to an EOS version.
- **AMB-04 Append-only.** Processes add records; nothing is updated or deleted in place (ARCH-13). "Removal" is supersession, quarantine or tombstone — all records.
- **AMB-05 Decomposition preserves cross-references.** A process working on a sub-region receives a *view* of the EOS: the region's records plus **boundary references** to everything they depend on or are referenced by (a dimension whose extension lines leave the region, a section label pointing to another sheet, a constraint spanning two views). Results are written back into the one EOS. A region with unresolved boundary references is, at best, *partially examined* (§6). No process works on a detached copy.

## 4. Lifecycle

Phases are not a fixed sequence: the runtime moves between them as signals demand. Each phase lists its processes, its zone, and what it may write.

| Phase | Purpose | Zone | Writes | Biological vocabulary |
|---|---|---|---|---|
| **1 Membrane** | Admit or reject external bytes; nothing unvalidated enters | Z0 | SourceArtifact, ingestion outcome | membrane |
| **2 Engulf** | Take the *whole* project in as one immutable snapshot before interpreting any part | Z0 | Snapshot, initial RegionTree (documents, pages), coverage denominators | phagocytosis |
| **3 Digest** | Sandboxed, recursive decomposition: document → page → region → element; exact extraction first, recognition second | Z1 | Regions, origin 1–4 observations, transform chains | lysosomal digestion |
| **4 Explore** | Bounded probes into regions that are uncertain, suspicious or uncovered; recognition branches with preserved alternatives | Z1 | Candidate observations (origin 3–6), alternatives, probe outcomes | pseudopodia, chemotaxis |
| **5 Relate** | Cross-view and cross-document relationship analysis; hypothesis generation | Z1 (proposal) + Z2 logic (evaluation) | Relations, hypotheses and verdicts | signalling |
| **6 Reconcile** | Deterministic conflict detection, gap finding, ambiguity investigation | Z2 logic | Issues (conflict, gap, ambiguity) | immune response |
| **7 Review** | Human confirmation, correction, decision, sign-off | Z4 | Origin-7 records, Decisions, signed ObservationSet | — |
| **8 Build** | Reconstruction, calculation, development, fabrication planning, DXF generation | Z2 | Constraints, parameters, geometry, calculations, outputs | differentiation |
| **9 Verify** | Independent verification, coverage evaluation, gate evaluation | Z3 | Evidence, coverage statements | immune response, quorum |
| **10 Release** | Engineer approval; package | Z4 | Approval | — |
| **11 Maintain** | Revision handling, recomputation, retirement, cache reclamation, learning within lanes | all | Supersessions, tombstones, memory records | regeneration, autophagy, apoptosis, memory, homeostasis |

- **AMB-06** Whole-project first: no interpretation process starts before engulfment has fixed the snapshot and the coverage denominators. This prevents "we processed what we noticed" from being mistaken for "we processed the project".
- **AMB-07** Phases 4–6 iterate until a **fixed point** (no process has new work) or until budgets are exhausted. Reaching a fixed point is recorded; stopping on budget is recorded as *incomplete* and is visible in coverage.

## 5. Mechanisms

Each of the thirteen biologically named mechanisms has a defined computational purpose, inputs, outputs, lifecycle, limitations and safety boundaries in [MECHANISMS.md](../architecture/amoeba/MECHANISMS.md). Summary:

| Mechanism | Computational equivalent | Tag | Key boundary |
|---|---|---|---|
| Phagocytosis | Whole-project engulfment into an immutable snapshot | [E] content-addressed snapshot / [P] as mandatory first step | Engulfing ≠ trusting; nothing is destroyed |
| Pseudopodia | Bounded exploratory probes into regions | [P] | Probes only append candidates; budgeted; terminable |
| Lysosomal digestion | Sandboxed decomposition of untrusted input into typed records | [E] sandboxing | **Departure from biology:** the source is preserved intact |
| Cellular signalling | Typed events derived from EOS changes | [E] event log | Signals carry references, never values; replayable |
| Chemotaxis | Priority-driven scheduling toward uncertainty, conflict and low coverage | [P]; benefit [H] | Deterministic priority; cannot skip mandatory work |
| Immune response | Invariant monitors, conflict detection, quarantine | [E] invariants / [P] quarantine | Quarantine isolates, never deletes; cannot be lifted by a model |
| Autophagy | Reclamation of recomputable derived artefacts | [E] cache eviction | Never evidence, never records |
| Cellular memory | Versioned, tenant-scoped knowledge (templates, vocabularies, calibrations, regression cases) | [P] | Memory proposes; it never supplies authoritative values |
| Differentiation | Deterministic specialisation of generic processes into typed ones | [E] dispatch | Based on accepted records, not on model preference |
| Regeneration | Rebuilding derived state from snapshot and records; incremental recomputation after change | [E] replay | Regenerated results must match within declared reproducibility level |
| Apoptosis | Controlled termination of a process, branch or algorithm version, leaving a tombstone | [E] supervision | Termination never removes evidence; retired algorithms stay executable |
| Homeostasis | Budgets, back-pressure, health invariants | [E] | Resource pressure may slow or stop work, never lower a gate |
| Quorum sensing | Thresholds over **independent** evidence before a state transition | [P] | Counts independence classes, not votes; never replaces verification or approval |

## 6. Whole-design inspection coverage

Full definition: [COVERAGE_MODEL.md](../architecture/amoeba/COVERAGE_MODEL.md).

**Principle (AMB-10).** Coverage is measured against denominators fixed at engulfment from the *source itself* — every file, page, vector object, CAD entity and ink component — not against the list of things a detector happened to find. Every such element must end in exactly one state.

| State | Meaning |
|---|---|
| **Fully examined** | Element is explained by accepted records (or excluded by a recorded human decision), all applicable processes completed, no open issue, no unresolved boundary reference |
| **Partially examined** | Some processes completed; others pending, budget-stopped, or boundary references unresolved |
| **Unexamined** | No interpreting process has completed on it |
| **Unsupported** | Recognised as content the system cannot interpret (entity type, symbol class, format feature) |
| **Ambiguous** | Examined, with more than one surviving interpretation or an open conflict |

Coverage domains: source documents and pages · annotations (recognised / unrecognised text-like regions, symbols) · geometric regions and elements · engineering objects and constraints · relationships and dependencies · unsupported or unresolved elements. A parent's state is the worst state among its children; a coverage statement is always a vector of counts per state per domain, never a single percentage.

- **AMB-11** The phrase "fully inspected" (or any equivalent in UI, report or certificate) may be produced only when every element in every domain is *fully examined*, for a stated EOS version. Otherwise the statement lists what is not.
- **AMB-12** Coverage is a claim about examination, **not correctness**, and is bounded by what the denominators can see (e.g. marks too faint to register as ink). Both limits are printed with every coverage statement.
- **AMB-13 (proposed gate G17 `coverage_complete`)**: release requires that every element intersecting the released parts' scope is *fully examined*; elements outside scope must be accounted for (examined, or excluded by decision). Proposed amendment to Spec 02 §10 — not applied.

## 7. Safety boundaries

Enforced structurally (schema permissions, zone rules, append-only store), not by convention. Each is a tested invariant in the benchmark.

| ID | AMOEBA must never… | Enforcement |
|---|---|---|
| AMB-S1 | Silently change an authoritative dimension | Records immutable; class-H values change only via origin-7 correction records with actor and reason (DI-50); silent-change count is a zero-tolerance metric |
| AMB-S2 | Delete original engineering evidence | Snapshot and records append-only; autophagy restricted to a declared class of recomputable artefacts; deletion under retention policy is a separate, audited, human-authorised administrative operation outside AMOEBA processes |
| AMB-S3 | Modify an approved design without a new revision | Approval binds to an EOS version hash; any upstream supersession reverts state to REVIEW REQUIRED (ARCH-15) and requires a new revision and approval |
| AMB-S4 | Treat OCR confidence as engineering validation | Confidence is not an input to corroboration, quorum, gate or priority-to-skip logic (DI-34, MATH-50); it may only add work |
| AMB-S5 | Automatically promote newly generated algorithms into production | Lanes: anything generated or tuned (templates, rules, models, shape-module candidates) enters *experimental*; promotion to *candidate* and *production* requires the qualification procedure (Spec 02 §12) and a signed human decision; experimental and candidate outputs cannot produce releasable evidence (gate G14) |
| AMB-S6 | Claim complete inspection while unsupported or unexplored regions remain | AMB-11; coverage ledger computed from source denominators |
| AMB-S7 | Bypass deterministic calculation, independent verification or engineer approval | Zone rules (ARCH-01…03); gates G10 and G16 unreachable by any AMOEBA mechanism; MATH-51 |

**Adaptive exploration is bounded, sandboxed, observable and recoverable (AMB-20):**

- *Bounded* — every probe has a budget (time, compute, depth, count) from a per-job allowance; the set of probe types is a fixed, versioned catalogue; a probe cannot spawn outside its allowance.
- *Sandboxed* — probes run in Z1 workers with no network and no write access except appending candidate records through the schema gate.
- *Observable* — every probe is in the ProcessLedger with target, reason (the signals that caused it), method, budget used and outcome.
- *Recoverable* — probes are idempotent and killable at any time; killing leaves a tombstone; the EOS is consistent after any crash.
- *Deterministic* — scheduling order is a pure function of the EOS version (priority function + tie-break by hash); replaying a job reproduces the same probes. Where true parallel timing could affect order, results must be order-independent (set-union of records) — a property to be benchmarked [H].

## 8. Drawing intelligence as native AMOEBA processes

Detailed mapping of every Spec 03 component: [EXISTING_ARCHITECTURE_MIGRATION.md](../architecture/amoeba/EXISTING_ARCHITECTURE_MIGRATION.md) §3. Overview:

| Required capability | AMOEBA process(es) | Phase | Spec 03 source |
|---|---|---|---|
| Drawing ingestion and membrane validation | `membrane.admit` | 1 | §1.1 |
| Whole-project engulfment and immutable snapshot | `engulf.snapshot`, `engulf.enumerate` (denominators) | 2 | §1.1, §8.1 — extended |
| Recursive decomposition of documents, pages, views, annotations, regions | `digest.document → digest.page → digest.region → digest.element` | 3 | §1.2–1.3, §2, §4 |
| Adaptive exploration of uncertain or suspicious regions | `explore.probe.*` (re-recognition at higher resolution, alternative binarisation, second recogniser, render cross-check, glyph-outline recognition, local re-segmentation) | 4 | §2.2, §3.5, DI-03 — generalised |
| OCR and recognition branches with preserved alternatives | `explore.recognise` producing n-best alternatives; branches are records, never collapsed | 4 | §3.2, §3.6 |
| Observation creation and provenance tracking | Schema gate on every append; origin × authority × reading state | all | §6 |
| Cross-view and cross-document relationship analysis | `relate.views`, `relate.documents`, `relate.hypotheses` | 5 | §5, §8 |
| Conflict detection and ambiguity investigation | `reconcile.rules` (RC-01…15), `reconcile.investigate` (targets probes at a conflict's region) | 6 | §7 |
| Evidence aggregation and coverage tracking | `coverage.update` after every process; `evidence.aggregate` | all | new |

What AMOEBA adds to Spec 03, concretely: the unsigned stage order becomes signal-driven iteration (a conflict found in phase 6 can direct a probe in phase 4 at the exact glyph in question); the denominators make "unrecognised" and "unexamined" content visible; and the ObservationSet hand-over (Spec 03 §10) becomes a *signed view of an EOS version* rather than a separate object.

## 9. Coherence of Specifications 01–03 under AMOEBA

Assessment (detail and proposed amendments in the migration note):

- **Spec 01** — coherent. Trust zones, truth graph, job orchestration, lanes and release statuses are either unchanged or generalised. Proposed amendments: EOS supersedes "truth graph" as the top-level term; orchestration becomes the AMOEBA runtime; coverage ledger and signal log added to the data architecture.
- **Spec 02** — coherent and essentially untouched: the UEM remains a pure library called from phase-8/9 processes. AMOEBA must not introduce adaptivity *inside* a calculation (AMB-21: exploration chooses *what* to compute, never alters *how* a qualified algorithm computes). Proposed amendment: gate G17.
- **Spec 03** — coherent; stages map one-to-one onto processes. Proposed amendments include the six review findings (correlated recogniser failures, glyph-level layout preservation, drawing-specific interpretation of parenthesised values, orthogonal provenance axes, per-dependency evaluation records, coverage accounting).

No contradiction was found that prevents operation under AMOEBA. The risks are in §10.

## 10. Architectural gaps, risks and unresolved decisions

| # | Item | Kind |
|---|---|---|
| 1 | AMOEBA had no prior written definition; this draft may differ from the intended concept | Gap — needs owner confirmation |
| 2 | Deterministic, replayable scheduling under real parallelism | Unresolved; benchmark |
| 3 | Coverage denominators for raster input (ink components) and their blind spots; cost of ink accounting on large scans | Unresolved; benchmark |
| 4 | Priority function for exploration: signals, weights, and proof that mandatory work cannot be starved | Unresolved design |
| 5 | Whether adaptive exploration improves outcomes over a fixed pipeline with the same budget | Hypothesis [H] |
| 6 | Quorum rules: defining and measuring independence of evidence sources (correlated recognisers) | Unresolved; depends on benchmark |
| 7 | Cellular memory: what may be learned per tenant, permission model, and how memory is prevented from biasing authoritative readings | Unresolved; legal input |
| 8 | Scope of "newly generated algorithms": which artefacts can be machine-generated at all (templates and rules — plausible; mathematical algorithms — not proposed) | Decision needed |
| 9 | EOS size and query performance for large packages; view extraction with boundary references | Benchmark |
| 10 | Metaphor risk: biological names can obscure simple mechanisms and invite unjustified expectations; every mechanism therefore has a plain engineering name in the mechanism note, which should be the term used in code | Mitigated by convention; review |
| 11 | Retention/deletion versus append-only EOS (inherited from Spec 01) | Unresolved; legal |
| 12 | Engineering-side RegionTree (assembly → part → surface → pattern) is only outlined; to be completed in Spec 04 | Deferred |

## 11. Next steps

1. Owner confirmation of §0.1 and §1 (is this the intended AMOEBA?).
2. Independent architecture review of this document and the notes.
3. Decision on the [proposed ADRs](../architecture/amoeba/PROPOSED_ADRS.md) and on the amendment list in the migration note.
4. Only then: Specification 04, written natively against the EOS and the process model.
