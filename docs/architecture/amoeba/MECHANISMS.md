# AMOEBA Mechanisms

Version 0.1 · 18 November 2026 · Supporting note to the [AMOEBA architecture](../../specifications/AMOEBA_NATIVE_SYSTEM_ARCHITECTURE.md) §5.

Each mechanism: plain engineering name (to be used in code), biological inspiration in one line (details and caveats in the [research note](../../research/AMOEBA_BIOLOGICAL_AND_COMPUTATIONAL_RESEARCH.md)), then **Purpose · Inputs · Outputs · Lifecycle · Limitations · Safety boundaries**. Tags: [E] established practice, [P] proposed, [H] hypothesis.

---

## 1. Phagocytosis → **Project engulfment** [E/P]

*Inspiration:* a cell takes a particle whole into an internal compartment before breaking it down.

- **Purpose.** Bring the complete project inside the system boundary as one immutable unit, so that every later claim is relative to a known whole.
- **Inputs.** Files admitted by the membrane; declared roles; tenant policy.
- **Outputs.** Snapshot (hash tree), enumeration of countable source elements (coverage denominators), initial RegionTree down to pages.
- **Lifecycle.** Once per snapshot; later uploads create a successor snapshot and a diff.
- **Limitations.** Enumeration is structural only; the project may be incomplete in ways engulfment cannot know (missing sheets appear later as gap Issues).
- **Safety.** Admission is not trust: content remains Z0/Z1 material. Nothing is consumed or altered — a deliberate departure from the biological process.

## 2. Pseudopodia → **Exploration probes** [P]

*Inspiration:* temporary extensions a cell puts out to move and to surround material.

- **Purpose.** Spend additional, targeted effort on a specific region where the baseline pass left uncertainty: re-recognise at native resolution, try an alternative binarisation, run a second recogniser, recognise glyph outlines, re-segment locally, test an alternative association.
- **Inputs.** Target region view; triggering signals; a probe type from the versioned catalogue; a budget.
- **Outputs.** Additional candidate records and alternatives; a probe outcome (`informative | no_change | failed | budget_stopped`).
- **Lifecycle.** Spawned by the scheduler; runs in a Z1 sandbox; ends by completion, budget or termination; always leaves a ProcessLedger entry.
- **Limitations.** More readings are not more truth: probes sharing preprocessing or models produce correlated results. Diminishing returns; each probe type's usefulness must be shown by benchmark [H].
- **Safety.** Append-only candidates; cannot change reading states beyond `unvalidated`; cannot remove alternatives; bounded per job; kill-safe.

## 3. Lysosomal digestion → **Sandboxed decomposition** [E]

*Inspiration:* engulfed material is broken down inside an enclosed compartment whose contents are kept away from the rest of the cell.

- **Purpose.** Convert untrusted input into typed, validated records inside an isolated environment, recursively: document → page → region → element.
- **Inputs.** A source artifact or region view.
- **Outputs.** Regions, exact-extraction observations, transform chains, unsupported-content items; all schema-validated before leaving the sandbox.
- **Lifecycle.** One short-lived isolated process per unit; crash = recorded outcome.
- **Limitations.** Decomposition quality bounds everything downstream; containment hierarchy can cut through relationships (hence boundary references).
- **Safety.** **The source is never broken down or altered** — only described. No network, no ambient filesystem, resource caps. Output that fails validation is rejected whole.

## 4. Cellular signalling → **State-change events** [E]

*Inspiration:* cells coordinate through molecules released and received.

- **Purpose.** Let processes react to what others found without calling each other: `RecordAppended`, `ConflictRaised`, `GapFound`, `AmbiguityFound`, `CoverageChanged`, `RecordSuperseded`, `QuarantineRaised`, `BudgetLow`, `FixedPointReached`.
- **Inputs.** EOS changes.
- **Outputs.** Ordered signal log; subscriptions determine which process types are offered work.
- **Lifecycle.** Emitted on every append; consumed by the scheduler; derivable by replay.
- **Limitations.** Feedback loops (A triggers B triggers A) need damping — handled by idempotence (no new record ⇒ no new signal) and budgets.
- **Safety.** Signals carry references, never values or instructions; a signal cannot grant permissions; all coordination is visible in the log.

## 5. Chemotaxis → **Signal-directed prioritisation** [P]; benefit [H]

*Inspiration:* movement along a concentration gradient toward or away from a substance.

- **Purpose.** Direct limited exploration effort to where it matters most.
- **Inputs.** Open Issues, coverage states, uncertainty, sensitivity of outputs to the element (Spec 02 ALG-11 where available), risk flags.
- **Outputs.** A deterministic priority ordering of candidate work items.
- **Lifecycle.** Recomputed at each scheduling step from the EOS version.
- **Limitations.** A priority function encodes judgement and can starve unglamorous work; it could chase noise. Whether it outperforms fixed ordering at equal budget is unproven.
- **Safety.** Mandatory baseline work (every element examined once by every applicable process) is scheduled independently of priority and cannot be starved; priority can only *add* effort. Recogniser confidence may raise priority but can never be a reason to skip. Deterministic: same EOS version ⇒ same ordering.

## 6. Immune response → **Invariant monitoring, conflict detection and quarantine** [E/P]

*Inspiration:* recognition of and response to what does not belong.

- **Purpose.** Detect and contain what is wrong: hostile or malformed input; schema or zone violations; EOS invariant breaks; contradictions between records; disagreement between calculation and verification; anomalous process behaviour.
- **Inputs.** Records, EOS invariants, reconciliation rules (RC-01…15), verification comparisons (ALG-13), runtime telemetry.
- **Outputs.** Issues; **Quarantine** records that mark a file, record set, process version or algorithm version as isolated: nothing downstream of a quarantined item can contribute to release.
- **Lifecycle.** Continuous. A quarantine is lifted only by a signed human decision referencing the resolution; fixed rules ("innate") are versioned; each resolved incident adds a regression case ("acquired" — see memory).
- **Limitations.** Detects only what a rule or invariant expresses; false positives cost review time; no claim of resemblance to biological immunity.
- **Safety.** Quarantine isolates, never deletes. No automatic repair. A model cannot raise *or* lift a quarantine on its own authority — it may only produce a candidate flag.

## 7. Autophagy → **Reclamation of derived artefacts** [E]

*Inspiration:* a cell breaks down and recycles its own components.

- **Purpose.** Recover storage and compute by discarding recomputable by-products: working images, render caches, tessellations, intermediate arrays, memoised calculations.
- **Inputs.** A declared class of *reclaimable* artefacts, each with the recipe (inputs + process version) to regenerate it.
- **Outputs.** Freed resources; a reclamation log.
- **Lifecycle.** Triggered by homeostasis thresholds or age.
- **Limitations.** Regeneration cost later; recipes must remain executable (pinned versions).
- **Safety.** The reclaimable class is defined by type, not by judgement: Snapshot, records, issues, ledgers, eliminated hypotheses and probe outcomes are **never** reclaimable. A reclaimed artefact must be regenerable within the declared reproducibility level, verified by test.

## 8. Cellular memory → **Versioned tenant knowledge** [P]

*Inspiration:* persistent changes in a cell's state resulting from past exposure.

- **Purpose.** Reuse what was established: per-customer title-block templates, sheet-furniture exclusions, vocabulary extensions, line-style mappings, calibrated ProcessData, resolved-issue patterns, regression cases.
- **Inputs.** Accepted human corrections and decisions; qualification results; calibration evidence.
- **Outputs.** Versioned memory records, each with provenance, scope (tenant, customer, machine), validity range and lane.
- **Lifecycle.** Proposed from experience → reviewed → promoted through lanes; can be retired; every use is recorded in the consuming process.
- **Limitations.** Memory can entrench an early mistake or bias reading toward what was seen before; it does not transfer across tenants; permission and ownership questions (drawings of the tenant's customers) are open.
- **Safety.** Memory **proposes and prioritises**; it never supplies an authoritative value, never auto-confirms a reading, and never crosses tenant boundaries. Learning from customer data requires recorded permission. New memory items start in the experimental lane (AMB-S5).

## 9. Differentiation → **Specialisation by typed dispatch** [E]

*Inspiration:* cells of one origin taking on distinct roles.

- **Purpose.** Hand each region or object to the process specialised for it: page type → pipeline; region type → title-block / view / table processor; accepted shape identification → shape module; surface type → development method.
- **Inputs.** Accepted type records (page classification, region type, shape module identification).
- **Outputs.** A specialised process instance with a declared contract.
- **Lifecycle.** At dispatch; if the type is later superseded, the specialised results are invalidated and the element re-dispatched.
- **Limitations.** A wrong type yields confident nonsense downstream; hence types used for dispatch in Z2 must be *accepted* records, and every specialised module checks its own applicability (`OutOfRange`).
- **Safety.** Dispatch is a deterministic function of accepted records, not of a model's preference. Uncertain types dispatch to *all* candidates, with results kept as alternatives.

## 10. Regeneration → **Recomputation from source and records** [E]

*Inspiration:* regrowth of lost or damaged parts.

- **Purpose.** (a) Rebuild derived state after a crash or cache loss; (b) recompute exactly what a change invalidates when a revision, correction or new algorithm version arrives; (c) re-create an archived release for audit.
- **Inputs.** Snapshot, records, pinned process versions; the invalidated sub-graph (ARCH-15).
- **Outputs.** New derived records; comparison with previous results.
- **Lifecycle.** On failure, on supersession, on demand.
- **Limitations.** Depends on old versions remaining executable; bitwise identity is not guaranteed across platforms (Spec 02 §7.5).
- **Safety.** Regenerated results must match the originals at the declared reproducibility level, otherwise an Issue is raised. Regeneration after a change never restores an approval: affected parts return to REVIEW REQUIRED (AMB-S3).

## 11. Apoptosis → **Controlled termination and retirement** [E]

*Inspiration:* orderly, programmed cell death.

- **Purpose.** End what should not continue: a probe out of budget; a branch whose hypothesis was eliminated; a process violating an invariant; an algorithm, model or template version withdrawn from use.
- **Inputs.** Budget state, hypothesis verdicts, invariant monitors, human retirement decisions.
- **Outputs.** Tombstone record with reason; for versions, a retirement record and the list of results that depended on them.
- **Lifecycle.** Requested by the supervisor or a person; the process stops at a safe point; its partial outputs are either complete records or absent (atomic appends).
- **Limitations.** Stopping early can leave regions partially examined — which coverage reports.
- **Safety.** Termination removes no evidence: eliminated hypotheses and failed probes stay in the EOS as records of what was considered. Retired algorithm versions remain executable for reproduction and are barred only from new work. Retirement of a version used in approved releases raises an Issue on each.

## 12. Homeostasis → **Budgets, back-pressure and health invariants** [E]

*Inspiration:* maintenance of stable internal conditions.

- **Purpose.** Keep the system within operating limits: queue lengths, memory, storage, per-tenant fairness, exploration budgets, review-queue size.
- **Inputs.** Telemetry and configured limits.
- **Outputs.** Throttling, deferral, reclamation triggers, `BudgetLow` signals, operator alerts.
- **Lifecycle.** Continuous control loop.
- **Limitations.** Control loops can oscillate; limits are configuration that must come from measurement.
- **Safety.** Pressure may slow, defer or stop work — reported as incomplete coverage — but may **never** lower a threshold, skip a gate, or mark anything examined. Review-queue pressure in particular must not relax confirmation requirements.

## 13. Quorum sensing → **Independent-evidence thresholds** [P]

*Inspiration:* a population changing behaviour once a signal produced by many members passes a threshold.

- **Purpose.** Require a minimum of *independent* supporting evidence before certain transitions: `unvalidated → corroborated` for a reading; acceptance of a hypothesis as sole survivor; lane promotion.
- **Inputs.** Evidence items with declared independence class (different formulation / implementation / library / physical / human) and a record of what they share (preprocessing, model family, training data, inputs).
- **Outputs.** Threshold verdict with the list of evidence counted and discounted.
- **Lifecycle.** Evaluated deterministically whenever evidence changes.
- **Limitations.** Agreement among dependent sources is not corroboration: two recognisers with shared training data or preprocessing fail together. Independence cannot be assumed from the fact that components have different names; it must be estimated from **measured correlated-failure rates** (benchmark) and is otherwise treated as absent.
- **Safety.** Counts independence classes, not votes; items sharing a declared dependency count once. A quorum can reach at most `corroborated`; it never substitutes for human confirmation where required (DI-33), for verification (G10) or for approval (G16). Confidence scores are not evidence items.
