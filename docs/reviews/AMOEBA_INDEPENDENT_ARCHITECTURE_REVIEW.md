# AMOEBA — Independent Architecture Review

Version 1.0 · 18 November 2026
Reviewed: [AMOEBA architecture](../specifications/AMOEBA_NATIVE_SYSTEM_ARCHITECTURE.md) v0.1 and its notes ([EOS](../architecture/amoeba/ENGINEERING_ORGANISM_STATE.md), [Mechanisms](../architecture/amoeba/MECHANISMS.md), [Coverage](../architecture/amoeba/COVERAGE_MODEL.md), [Migration](../architecture/amoeba/EXISTING_ARCHITECTURE_MIGRATION.md), [Proposed ADRs](../architecture/amoeba/PROPOSED_ADRS.md)), the [benchmark](../benchmark/AMOEBA_ARCHITECTURE_BENCHMARK.md), the [research note](../research/AMOEBA_BIOLOGICAL_AND_COMPUTATIONAL_RESEARCH.md), the Master Plan, Specs 01–03 and ADR-001…008.

Short references used below: **ARCH** = AMOEBA architecture document; **EOS**, **MECH**, **COV**, **MIG**, **PADR**, **BENCH**, **RES** = the notes above; **S01/S02/S03** = Specifications 01–03; **MP** = Master Plan.

---

## 0. Scope, method and limits of this review

- This is a **document review carried out by an AI reviewer**. No software exists; nothing was executed or measured. I had no access to factory drawings, factory staff, customer data or any running system.
- I did not write the reviewed documents. I read the AMOEBA documents and Spec 01 in full, and ADR-006 in full; Spec 03, the Master Plan and Spec 02 (§0, §1.2–1.3, §9–§10, §12–§14) by section, concentrating on requirements, contracts and gates. Spec 02 §7–§8 and §11 and ADR-001…005, 007 and 008 were not read beyond headings or keyword matches, so findings that depend on them are provisional. Findings about Spec 02 mathematics are out of scope.
- Every weakness below cites the text it rests on. Where I infer a consequence, I say so.
- Statements are tagged where it matters: **[E]** established principle, **[P]** proposed design, **[H]** unverified hypothesis. Nothing in this review is a claim of novelty; no prior-art search has been done, and several of my own proposals are well-known techniques.
- External research was limited to seven web lookups (§9). Six searches returned result summaries; one direct page fetch failed. I did not read any source in full, and the verdicts in §9 are at that level of confidence only.
- **This review is not a substitute for review by human software architects, fabrication engineers and counsel.** It should be used to prepare that review.

---

## 1. Verdict summary

The AMOEBA documents are unusually honest about their own status: they tag hypotheses, admit that the design "is recognisably a blackboard system" (RES §2) and that "most of AMOEBA's lifecycle maps onto structures already specified" (ARCH §0.4). That honesty is the main reason the design can be reviewed at all. Tested against the owner's requirement, however, the draft does not yet deliver it. What is specified is a typed, append-only blackboard with an agenda scheduler, a coverage ledger and lifecycle controls, described in detail for **drawing intelligence only**. The engineering side (reconstruction, calculation, development, verification) is one lifecycle row and a deferred outline. The "adaptive" part is reactive re-scheduling of a fixed probe catalogue by a priority function that is not defined. Defect discovery — the owner's stated purpose — has no model of how defects propagate between components and no metric in the benchmark. The safety boundaries are well intentioned and mostly structural, but I found six concrete contradictions or loopholes (W4–W9), two of which would make the determinism claim and the coverage gate unworkable as written. The non-adaptive parts (EOS, source-denominated coverage, lanes, quarantine) are worth keeping after correction; the adaptive part needs real design work before any ADR on it can be accepted.

| # | Question | Rating | One-line answer |
|---|---|---|---|
| 1 | Governs the whole lifecycle? | **Not shown** | Drawing intelligence is mapped process by process; everything from Build onward is asserted (ARCH §4 row 8, §10 item 12) |
| 2 | Engulfment and recursive exploration achievable? | **Partly** | Snapshot and vector enumeration are achievable; raster denominators, views with boundary references, termination and deterministic scheduling are under-specified |
| 3 | EOS represents complete projects and dependencies? | **Inadequate** | Source side is reasonable; engineering side is an outline; no typed dependency model, so no defect propagation between components |
| 4 | Credible path to showing advantage? | **Weak** | B-4 has a confounded baseline, two of four hypotheses do not test adaptivity, no statistics, no defect-discovery metric |
| 5 | Mechanisms substantive? | **Mixed** | Of the thirteen: none fully substantive as specified, 6 partly, 7 renamings; the two clearly substantive ideas (coverage ledger, lanes/quarantine) are not among the thirteen (§6) |
| 6 | Scales beyond pipe and plate? | **Unproven** | Source side is generic; engineering side, coverage analogues and probe catalogue are domain-specific and no domain contract exists |
| 7 | Safety boundaries intact? | **Mostly, with loopholes** | No intent to weaken them; six contradictions/loopholes need closing before acceptance |
| 8 | Unnecessary complexity or duplication? | **Yes** | "Ambiguous" and "unsupported" each exist in five places; three provenance ledgers; derived data inside the state hash |

---

## 2. Architectural strengths

1. **Source-denominated coverage (AMB-10, COV §2)** [P]. Counting against what is in the file rather than what a detector found is the most valuable idea in the set. It directly answers "we processed what we noticed" (AMB-06). It does not depend on any adaptivity.
2. **Four orthogonal provenance axes with computed `usable_as_H` (EOS §4)** [P]. Separating origin, authority, reading state and lifecycle, and computing usability instead of storing it, is a sound way to keep assumptions and facts apart at record level.
3. **Safety rules are structural where possible** (schema permissions per zone, append-only store, lanes; ARCH §7). AMB-02 states that existing rules win over any mechanism.
4. **Explicit departures from the metaphor** (digestion preserves the source; quorum counts independence, not votes — MECH §3, §13; RES §1). The authors did not let the biology decide the design.
5. **The dependence problem is recognised** (MECH §2, §13; MIG A-03-1; BENCH B-6): agreement among correlated recognisers is treated as worth nothing until measured.
6. **A fallback is declared**: if H1–H4 fail, reduce ADR-009 to the non-adaptive parts (BENCH B-4). This is the right instinct, although the decision rule is not yet precise (W10).
7. **No existing specification was edited**; amendments are listed, not applied (MIG header).

---

## 3. Critical weaknesses

Severity: **Critical** = blocks acceptance of the related ADR; **High** = must be resolved before Specification 04; **Medium** = must be resolved before implementation.

### W1 — AMOEBA governs drawing intelligence in detail and the rest by assertion — *Critical*
**Evidence.** ARCH §8 and MIG §3 map every Spec 03 component to a named process, phase, mechanism and coverage domain. The equivalent for engineering is one lifecycle row ("8 Build … differentiation", ARCH §4), three lines in MIG §4, and "Engineering-side RegionTree … only outlined; to be completed in Spec 04" (ARCH §10 item 12; EOS §2). The probe catalogue is entirely recognition probes (ARCH §8; MECH §2). Engineering coverage (D7) is one sentence (COV §3). The diagram shows `RECONCILE ─► BUILD ─► RELEASE` as one-way (ARCH §2). The Master Plan does not mention AMOEBA at all and names verification as "the heart of the company" (MP §21).
**Consequence.** The claim "every activity of the engineering lifecycle … is an AMOEBA process" (ARCH §1, AMB-01) is true only in the trivial sense that any job step can be called a process. The owner's requirement — detect geometric problems, mathematical errors and manufacturing risks adaptively — lives in exactly the part that is not designed. Verification disagreement (S02 MATH-41), `Contradictory`, `UnderDetermined` and `OutOfRange` outcomes are the richest suspicion signals in the system and none is wired back into exploration.
**Recommendation.** Before ADR-009: a process catalogue for Build and Verify at the same level as MIG §3; engineering-side signals (verifier disagreement, conditioning, budget margin, out-of-range) as scheduler inputs; at least three engineering probe types (e.g. second verifier formulation, tightened interval enclosure, alternative solver start set per ALG-02); feedback edges from phases 8–9 to 4–6.

### W2 — No dependency model, therefore no defect propagation between components — *Critical*
**Evidence.** Edges are `derived_from` plus untyped "cross-reference edges" (EOS §2 RegionTree). `Issue` lists "records involved, affected regions" (EOS §2) with no propagation rule. The priority inputs are "open Issues, coverage states, uncertainty, sensitivity, risk flags" (MECH §5) — not the neighbourhood of a finding. ARCH-15 gives forward invalidation of derived records only. There is no record type for a relationship *between parts* (mating diameters, shared seams, a branch sitting on a header, plate-thickness continuity, item mark to parts list).
**Consequence.** A misread header diameter invalidates what was derived from it, but nothing directs scrutiny to the branch saddle cut that must fit it, to other dimensions read by the same recogniser from the same degraded region, or to other parts built from the same template. That sideways and backward follow-up is what "following dependencies across components" means; a per-part pipeline with ARCH-15 already does the rest.
**Recommendation.** Mechanism M1 (§5). Add typed dependency edges and an `InterfaceConstraint` record family to the EOS.

### W3 — The adaptive core is undefined — *Critical*
**Evidence.** The priority function is "unresolved design" (ARCH §10 item 4; RES AR-04). Probe usefulness is [H] (MECH §2). Budget size, units and split between baseline and exploration are not stated. The fixed point of AMB-07 is asserted; MECH §4 relies on "idempotence (no new record ⇒ no new signal)", which does not stop a loop in which each probe produces a *new, different* candidate. "Adaptive" means only: a signal raises a priority; the next catalogue probe runs. Nothing estimates what a probe is expected to achieve, nothing records whether it did, and nothing changes strategy.
**Consequence.** ADR-012 asks for acceptance of a mechanism whose central function does not exist. As specified, termination is guaranteed only by the budget, so "fixed point reached" will in practice mean "catalogue exhausted".
**Recommendation.** Mechanisms M2, M4, M5 (§5). Define budgets in deterministic work units. State a monotonicity condition for termination (finite catalogue × finite targets × each (probe type, target, input-version) runs at most once).

### W4 — The determinism model is unsound as written — *Critical*
**Evidence.** (a) "Scheduling order is a pure function of the EOS version" (ARCH §7 AMB-20). With parallel workers, the EOS version seen when a worker becomes free depends on which other workers finished first. (b) "Results must be order-independent (set-union of records)" — set-union makes *merging* order-independent, not *content*: a process that starts at version v and one that starts at v′ can legitimately write different records. (c) `eos_version` hashes `process_root` (EOS §1), and `ProcessInstance` contains "budget granted/used" (EOS §2); budgets include "time" (ARCH §7). Time used differs between runs, so the hash differs. (d) Heads hold "job state" and are hashed. (e) BENCH B-2 requires an identical EOS version "across thread counts and worker counts". (f) Model-based recognisers are not shown to be bit-reproducible; S01 §3.3 already lists cross-platform reproducibility as Open.
**Consequence.** B-2 cannot pass as specified; H5 fails by construction. The replay guarantee underpinning auditability of *why* the system looked where it looked is not available.
**Recommendation.** Adopt a round-based (bulk-synchronous) schedule [E]: all processes of a round read the same EOS version; outputs are merged in canonical order; the next round is planned from the merged state. Budgets in counted work units (probes, pixels, iterations), never wall-clock; wall-clock limits become a separately recorded *fault*, not an engineering outcome. Exclude resource telemetry and job status from the engineering state hash. For non-reproducible recognisers, treat the recorded output — not the re-execution — as the replay input, and say so.

### W5 — Approval is bound to a hash that changes for unrelated reasons — *Critical*
**Evidence.** "Approval binds to an EOS version hash" (ARCH §7 AMB-S3; EOS invariant 5). The EOS version covers the whole project including process ledger, coverage ledger and heads (EOS §1). S02 G16 binds approval to "the content hash of exactly the evaluated records". Invariant 5 also requires that "all required gates evaluate PASS" at the referenced version, while G16 *is* the approval, and the Approval record itself is an append that produces a new version.
**Consequence.** Either an approval of part P is invalidated by any later append anywhere in the project (another part's probe, a periodic regeneration test under ARCH-16, a memory record), or "current release state" must be computed by some unstated scoping rule. The two documents disagree on what was approved. For a release-authority record this is a safety-relevant ambiguity.
**Recommendation.** Keep S02 G16. Define a **release closure**: the transitive `derived_from` closure of the released outputs plus the gate evaluations and coverage statement for that scope; the approval signs its hash. Use `eos_version` only as a consistency marker. Reword invariant 5 to "gates G01–G15 and G17".

### W6 — Confidence isolation contradicts confidence-driven priority — *High*
**Evidence.** AMB-S4: confidence "is not an input to corroboration, quorum, gate or priority-to-skip logic … it may only add work". MECH §5: "Recogniser confidence may raise priority". BENCH B-1 *Confidence isolation*: injecting extreme confidence must cause "no change in any reading state, threshold verdict, gate result or mandatory work performed". S03 §3.6 makes `corroborated` depend on "all recognisers agree".
**Consequence.** Causal chain: confidence → priority → an extra recogniser probe runs on element A instead of element B (the budget is fixed) → the set of recognisers that "all agree" differs → reading state and quorum verdicts differ. Under a fixed budget, "only add work" here is "remove work" there. Either the B-1 test fails or confidence cannot influence scheduling at all.
**Recommendation.** Decide explicitly. My recommendation: confidence may order work *within* a discretionary budget, and the invariant is restated as "no reading state, verdict or gate result may be *more favourable* than it would be with all confidences set to their least favourable value". Corroboration rules must be defined over a **fixed, declared recogniser set** per element type, not over "whichever recognisers happened to run".

### W7 — The coverage gate cannot pass for typical scans, and "explained" is defined two ways — *High*
**Evidence.** (a) COV §5: G17 is "INCONCLUSIVE if enumeration itself was limited (e.g. tile-level D4 on an over-size scan)" and "not resolvable by Decision except through explicit per-element exclusion". COV §2 uses tiles "for very large components"; COV §6 notes that touching content forms one component. In a line drawing, a view with its dimension and extension lines is commonly one connected component (my inference from how such drawings are drawn — to be confirmed on real scans). (b) ARCH §6: fully examined means "explained by **accepted** records"; COV §3: "linked to at least one **current** record that accounts for it". (c) "Excluded by Decision" is folded into `fully_examined` (COV §3). (d) Condition (b) of `fully_examined`, "every process applicable to its type has completed", depends on a type assigned by classification and on an applicability table that does not exist; whether exploration probes count as "applicable" is not stated.
**Consequence.** (a) Read literally, scanned drawings with large components can never reach APPROVED. (b) Under the COV wording a Z1 candidate can make an element "fully examined", so a non-authoritative process influences a release gate; under the ARCH wording every sheet-furniture and geometry-role record needs acceptance, with unknown reviewer cost. (c) A certificate saying "fully inspected" (AMB-11) may cover content a person chose not to look at. (d) A misclassified element gets the wrong obligations and reads as complete.
**Recommendation.** Add a sixth state `excluded_by_decision`, reported separately and never worded as "inspected". Define "explained" once, with the accepting authority per record type. Publish the applicability table as a versioned artefact; make probes non-mandatory by definition. Define what resolves INCONCLUSIVE. Do not allow the phrase "fully inspected" on external documents until B-5 shows it matches what engineers mean; prefer "all enumerated source content accounted for (see limits)".

### W8 — Lane isolation is defined on the wrong closure, and experimental work can steer production — *High*
**Evidence.** EOS invariant 7: "no record produced by a non-production process appears in the evidence of an Approval". Experimental and candidate processes write into the same EOS and emit the same signals (ARCH §2; MECH §4); memory items in the experimental lane "propose and prioritise" (MECH §8).
**Consequence.** "The evidence of an Approval" is not defined as a transitive closure, so an experimental candidate that a person confirms, or an experimental template that causes an exclusion, may or may not be tainted. Separately, experimental records change signals, hence the schedule, hence which production probes run: production results depend on what experimental code was installed, and a replay after retiring it differs.
**Recommendation.** Experimental and candidate lanes write to an **overlay** that production scheduling cannot read. Define taint over the release closure (W5) with human confirmation against the source crop as the only clearing step, and record that it happened.

### W9 — The runtime is an unzoned, privileged component — *High*
**Evidence.** Processes are zoned (ARCH §1.3); the runtime (scheduler, budget manager, supervisor, signal dispatcher) is not placed in any zone (ARCH §2). `coverage.update` runs "after every process" (ARCH §8) with no zone; coverage statements are Evidence records (EOS §2), which S01 reserves for Z3. `reconcile.investigate` is "Z2 logic" but directs Z1 probes (MIG §3).
**Consequence.** The component that decides what gets examined, with how much effort, and what the coverage ledger says is outside the trust model. In a fixed pipeline this component is trivial; in AMOEBA it carries engineering judgement (the priority function) and is therefore part of the safety case.
**Recommendation.** Declare the runtime, schema gate and coverage evaluator a trusted computing base with its own requirements, qualification (S02 §12-style) and lane discipline. The priority function and applicability table are versioned, qualified artefacts.

### W10 — The benchmark cannot establish the advantage it is meant to test — *High*
See Q4 in §1 and the detailed analysis in §5.0. In brief: the baseline is under-resourced, H3 and H4 are not adaptivity, there is no defect-discovery metric, and no statistical design.

### W11 — "Sole surviving hypothesis" on an incomplete search is an assumption treated as a fact — *Medium*
**Evidence.** S03 §5.5: one surviving hypothesis → "proceed", while "hypothesis enumeration may be incomplete" (`search_complete: bool`). MECH §13 lists "acceptance of a hypothesis as sole survivor" as a quorum transition. S02 G02 passes when no ambiguity record is open.
**Consequence.** The interpretation on which all geometry rests can pass G02 because the alternatives were never generated. This is exactly the confusion of assumption with fact the owner wants excluded.
**Recommendation.** A sole survivor with `search_complete = false` carries authority `assumed`, appears in an assumptions list at release, and requires an engineer Decision.

---

## 4. Where the design is still a conventional blackboard / event-sourced architecture

| AMOEBA element | Conventional counterpart [E] | What, if anything, is added |
|---|---|---|
| EOS Records + RegionTree | Blackboard with levels; Spec 01 truth graph | Strong typing, zone permissions — already in Spec 01 |
| Processes | Knowledge sources / job steps | Declared budget and termination |
| Runtime + priority function | Agenda scheduler; the blackboard "control problem" (RES AR-02) | Nothing yet — the function is undefined |
| Signals | Events derived from an append-only log | "References, never values" rule |
| Fixed-point iteration (AMB-07) | Run until agenda empty | — |
| Snapshot, append-only, replay | Merkle DAG + event sourcing (ADR-006) | — |
| Regeneration, autophagy, apoptosis, homeostasis | Incremental rebuild, cache eviction, supervision, back-pressure (all in S01 §7) | Names; tombstones |
| Lanes | Release channels; S01 §7 "experimental versions"; G14 | Three levels, records |
| Coverage ledger | — | **New relative to Specs 01–03** |
| Quarantine as a record with downstream block | Partly new | **New relative to Specs 01–03** |

The design becomes more than this only where the system (a) holds an explicit model of *what could be wrong and why*, (b) chooses work by expected effect and checks the expectation afterwards, (c) follows dependencies from a finding, and (d) audits its own conclusions. None of the four is specified today.

---

## 5. Missing mechanisms and proposed improvements

### 5.0 First: what the benchmark must be able to show
Problems in BENCH B-4:

- **Confounded baseline.** "Fixed pipeline (Spec 03 stage order, no signal-driven probes)" at "equal compute budget". If the baseline has no probes, it has nothing to spend the budget on; the comparison is "more recognition effort versus less". Required baselines: (B0) fixed pipeline, no probes; (B1) *uniform* — the same probe catalogue applied to every element in fixed order until the budget is gone; (B2) *random* targeting; (B3) *static heuristic* (e.g. always re-probe all numeric text); (B4) *oracle* targeting from ground truth, as an upper bound. Adaptivity is supported only if it beats B1–B3.
- **H3 and H4 do not test adaptive exploration.** Priority-ordered review is already in S03 §9.2 (triage "ordered by impact"); tenant knowledge is MP module 13. Both can be attached to a fixed pipeline. H5 is a non-inferiority condition. Only H1 and H2 bear on ADR-012.
- **No defect-discovery endpoint.** The owner's purpose is finding inconsistencies, geometric problems, mathematical errors, missing information and manufacturing risks. B-4 measures OCR false accepts and conflict explanation. Add: recall and precision per seeded-defect class, defects found per unit budget, and engineer minutes to an approvable state.
- **DI-33 limits H1's effect.** Every class-H value is human-confirmed by default (S03 DI-33), so the released false-accept rate is dominated by reviewers. H1 should be measured as reviewer error rate and time with and without probe-generated alternatives.
- **No statistical design.** Missing: primary endpoint, paired design on identical documents, minimum effect size, sample size, confidence intervals, correction for multiple hypotheses, a held-out corpus not used to tune the priority function, and pre-registration of the decision rule. "If H1–H4 are not supported" does not say whether one failure or all four triggers the fallback.
- **Seeded versus natural defects.** H2 uses seeded conflicts; results must be reported separately for seeded and naturally occurring defects from authorised drawings.
- **Cost side missing.** Added engineering and qualification cost of the adaptive runtime (W9) should be weighed against the measured benefit.

### 5.1 Proposed mechanisms
All are **[P]** with benefit **[H]**. None is claimed to be new; M2, M3 and M5 rest on established ideas (value-of-information reasoning, truth maintenance, capture–recapture estimation in inspection).

**M1 — Dependency-following suspicion propagation.**
*What:* Typed dependency edges (`measures`, `mates_with`, `shares_seam`, `same_feature_as`, `read_by_same_method_in_same_region`, `instantiates_template`, `computed_by_algorithm_version`) and `InterfaceConstraint` records between parts. When an Issue is confirmed, a deterministic rule table maps (issue kind, edge type) to *scrutiny obligations* on neighbours: forward (dependents), sideways (parts that must fit), and common-cause (other outputs of the same recogniser on the same degraded region; other parts using the same module version). Propagation depth and fan-out are bounded and recorded on the Issue.
*Why a fixed pipeline cannot match it:* a fixed pipeline can do forward invalidation and can run cheap cross-checks on everything. It cannot spend *expensive* checks (human attention, high-resolution re-recognition, kernel cross-checks, Monte Carlo) selectively on the neighbourhood of a finding, because it does not know the finding when its order is fixed. If all checks were cheap, M1 would add nothing — that is the test.
*Deterministic/bounded/auditable:* rule table versioned and qualified; integer depth limits; every obligation records its originating Issue and edge path.
*Measured by:* corpora with seeded common-cause and interface defects; recall of the *secondary* defects per unit budget against B1–B3.

**M2 — Hypothesis-driven investigation with explicit accounting.**
*What:* Each open Issue carries an explicit set of candidate explanations (S03 RC-01 already lists them: misread, wrong association, not to scale, real drawing error). Each probe type declares which explanations it can discriminate and its cost in work units. The scheduler selects the probe with the highest declared *expected risk reduction per cost*, where risk weight = sensitivity of released geometry to the element (S02 ALG-11) relative to tolerance margin, and discrimination rates come from a versioned table measured on the benchmark. Before running, the predicted outcome distribution is recorded; afterwards the realised outcome is recorded beside it.
*Why a fixed pipeline cannot match it:* it has no notion of which explanation is being tested, so it cannot stop when a conflict is explained nor prefer the discriminating probe.
*Deterministic/bounded/auditable:* tables are fixed data in the production lane; rational arithmetic; ties broken by hash; no learning inside a job. The prediction/outcome pairs make the scheduler's judgement *checkable* — a property the current priority function lacks.
*Measured by:* conflicts correctly explained per budget versus B1–B3; calibration of predicted versus realised probe outcomes; share of budget spent on probes with outcome `no_change`.

**M3 — Self-consistency audit of the system's own analysis.**
*What:* (a) *Justification records:* every derived conclusion (reading state, hypothesis verdict, coverage state, gate result) stores the records that justify it; an auditor process recomputes each conclusion from scratch and compares with the incrementally maintained value. (b) *Metamorphic self-checks:* re-run analysis of a sample of regions under transformations that must not change conclusions (page rotation, alternative render resolution, permuted sheet order) and raise an Issue on any difference. (c) *Assumption ledger:* every conclusion resting on an incomplete search, an uncalibrated model or a default is listed as an assumption at release (W11). (d) *Detection-power check:* seeded synthetic defects in a shadow overlay of the job measure whether the current configuration would detect them on this document type; the result accompanies the coverage statement.
*Why a fixed pipeline cannot match it:* (a)–(c) could be added to a fixed pipeline and should be adopted regardless of the AMOEBA decision. What is specific to an adaptive runtime is that audit discrepancies become Issues that redirect investigation (via M1/M2).
*Deterministic/bounded/auditable:* sampling by hash of element id; the shadow overlay never touches release records (same rule as W8).
*Measured by:* seeded internal faults (stale conclusion, dropped invalidation, order-dependent result) — detection rate; overhead as a share of the budget.

**M4 — Strategy adaptation with recorded, bounded strategy changes.**
*What:* A small, versioned set of strategy profiles (budget split between probe families, depth limits, review ordering) qualified through the lanes. In-job statistics (probe yield by type, conflict density, share of unexplained content) are evaluated against deterministic switching rules at round boundaries. Each switch is a `StrategyChange` record with the triggering statistics; the number of switches per job is capped; a switch can only reallocate discretionary budget and can never alter the mandatory baseline, a threshold or a gate.
*Why a fixed pipeline cannot match it:* by definition it has one profile.
*Deterministic/bounded/auditable:* finite profile set; rules are data; no profile is generated at run time (consistent with AMB-S5).
*Measured by:* outcome versus the best single static profile chosen in hindsight per document class; if adaptation does not beat the best static profile per input class (selectable at ingestion), M4 is not needed.

**M5 — Stopping rule and residual-risk statement.**
*What:* Track the defect-discovery curve (new confirmed Issues per unit budget). Where two detection paths with measured dependence exist, estimate the number of undetected defects by capture–recapture. Stop discretionary exploration when marginal yield falls below a declared threshold; print the estimate and its limits beside the coverage statement.
*Why a fixed pipeline cannot match it:* it has no concept of "enough"; it stops when the stages end.
*Deterministic/bounded/auditable:* pure function of recorded Issues. The estimate is advisory and is not an input to any gate.
*Measured by:* estimated versus actual remaining seeded defects. **Explicitly an unverified hypothesis**: the independence assumptions of capture–recapture may not hold here (cf. B-6).

### 5.2 Ideas rejected as cosmetic or unsafe
- Further biological names for existing functions (e.g. "metabolism" for budgets, "mitosis" for forking a branch): no behaviour added.
- A single "organism health" or "confidence" score: contradicts COV §4 (no single percentage) and S02 MATH-50.
- Run-time generation of new probes, rules or algorithms by a model, or self-tuning of the priority function within or across jobs without the lane procedure: violates AMB-S5 and the owner's prohibition on uncontrolled self-modification.
- Free-form multi-agent negotiation between processes: not replayable; already rejected in PADR ADR-012.

---

## 6. Mechanism-by-mechanism classification

| # | Mechanism | Class | Reason |
|---|---|---|---|
| 1 | Phagocytosis / engulfment | **Partly substantive** | Snapshot is S01 SourceArtifact + hash tree; the *enumeration of denominators before interpretation* (AMB-06) is real and new to the specs |
| 2 | Pseudopodia / probes | **Partly substantive** | A bounded probe catalogue is a real construct; no engineering probes, no expected effect, usefulness [H] |
| 3 | Lysosomal digestion | **Renaming** | Sandboxed parsing, S01 §8 and S03 §1.1; the analogy is inverted (source preserved) |
| 4 | Cellular signalling | **Renaming** | Derived events on an append-only log |
| 5 | Chemotaxis | **Partly substantive (undefined)** | Agenda prioritisation is the heart of adaptivity, but the function is not designed (W3) |
| 6 | Immune response | **Partly substantive** | Invariants and RC rules exist in Specs 01–03; quarantine-as-record with downstream block is an addition |
| 7 | Autophagy | **Renaming** | S01 §7 geometry cache; adds a type-based rule |
| 8 | Cellular memory | **Partly substantive** | MP module 13 plus propose-only and lane rules; the weakest analogy by the authors' own account (RES §1) |
| 9 | Differentiation | **Renaming** | Typed dispatch; "uncertain types dispatch to all candidates" is already S03 §1.3 behaviour |
| 10 | Regeneration | **Renaming** | S01 §7 incremental recomputation and ARCH-16 |
| 11 | Apoptosis | **Renaming** | Supervised termination; tombstones are a small addition |
| 12 | Homeostasis | **Renaming** (borderline) | S01 §7 resource management; the rule "pressure never lowers a threshold" is valuable but is a requirement, not a mechanism |
| 13 | Quorum sensing | **Partly substantive** | Generalises S03 §3.6 and S02 §9.1 independence classes; credit rule undefined (AR-07); analogy inverted |
| — | Coverage ledger (not one of the 13) | **Substantive** | See §2 |
| — | Lanes + quarantine (not one of the 13) | **Substantive** | See §2 |

Result for the thirteen: 0 fully substantive as specified, 6 partly substantive, 7 renamings (counting homeostasis). The two clearly substantive ideas in the documents are not among the thirteen biological mechanisms.

---

## 7. Performance and scalability risks

1. **Raster denominators.** An A0 sheet at 400 dpi is roughly 250 megapixels (my arithmetic). Noisy scans may yield very large numbers of ink components; each is a ledger element with state, examining processes, explaining records and issues (EOS §2). "Hundreds of sheets" (BENCH B-7) multiplies this. No estimate exists (ARCH §10 item 3).
2. **Vector denominators.** "Every painted path" and "every entity incl. those inside blocks" (COV §2): CAD-exported PDFs with hatching and outlined text, and DXF block instances, can be very large; whether block definitions or instances are counted is unstated.
3. **`coverage.update` after every process** and parent recomputation (ARCH §8; COV §4) on such ledgers; priority "recomputed at each scheduling step from the EOS version" (MECH §5).
4. **Views with boundary references** (EOS §3). General notes, general tolerances, unit declarations, scale and title-block fields relate to everything on the sheet or in the project; the boundary set of any region then approaches the whole EOS, and every region stays "partially examined" until project-level obligations are resolved. A separate treatment of *global* dependencies is needed.
5. **One isolated process per unit down to element** (MECH §3) implies process-spawn cost per element.
6. **Single-node on-premises baseline** (ADR-008) with local models: exploration competes with the baseline for modest hardware.
7. **Relational storage for graph traversal** (ADR-006 risk) now carries views, coverage and propagation queries as well.
8. **Engulf-then-interpret (AMB-06)** conflicts with how packages arrive (incrementally, with revisions). Each new Snapshot needs a defined rule for reuse of records and coverage from its predecessor; only "the difference drives revision handling" is said (EOS §2).

Domain scalability (Q6): the source side generalises. The engineering hierarchy "assembly → part → surface/feature/joint → pattern → profile" and the D7 definitions ("a profile when validated and compared after re-import") are plate-development concepts. There is no **domain contract** stating what a new domain must supply: record types, region types, applicability table, reconcile rules, probe types, dependency edge types, coverage definitions, gates. Without it, each new domain is a redesign of the engineering side.

---

## 8. Security and reliability risks

1. **Input-steered scrutiny (specific to adaptivity).** Under a fixed budget, content chooses where effort goes. A hostile or merely messy region can absorb discretionary budget and draw scrutiny away from a critical dimension. BENCH B-3 tests starvation of the *baseline* under "signal floods", not displacement *within* exploration. Mitigation: per-region and per-issue budget caps; reserve a share of discretionary budget for sensitivity-ranked elements regardless of signals.
2. **Amplification.** Inputs crafted to maximise probes, alternatives and signals (cost attack on a single-node deployment).
3. **Memory as an exclusion channel.** "Sheet-furniture exclusions" (MECH §8) reduce reviewer workload (COV §6) by proposing that content be excluded from coverage. A wrong or malicious template hides content from inspection with a human click. Exclusions proposed by memory need the same anti-rubber-stamping controls as class-H confirmations (S03 §9.2), and must be reported separately (W7).
4. **Text in drawings as instructions to models.** Z1 language models read drawing text. Signals carry references only (good), but a model can emit candidate flags that raise priorities and create review items. S03 DI-91 forbids machine interpretation of free text downstream; an explicit test for instruction-like text in drawings is missing from B-1.
5. **Quarantine as a denial-of-service lever.** Invariant violation ⇒ quarantine (EOS §5); lifting needs a signed human decision (MECH §6). An input that reliably triggers a process fault could quarantine a process *version* for the whole tenant. Scope rules are needed (file-level first; version-level only by human decision).
6. **Mass state changes.** "Retirement of a version used in approved releases raises an Issue on each" (MECH §11), and regeneration mismatch raises Issues (MECH §10); a hardware or driver change affecting a recogniser could reopen many approved releases at once.
7. **Retention deletion** is "outside AMOEBA processes" (AMB-S2) while AMB-01 says nothing engineering happens outside them, and hash-tree verification after deletion is unspecified (also ADR-006).
8. **Unzoned runtime** — W9.
9. **`ProcessInstance.trigger (signals)`** refers to a SignalLog that is "not hashed" and may not be persisted (EOS §1, §6): the audit trail of *why* a probe ran can dangle. Signal identifiers must be deterministic and derivable, or the log must be persisted.

---

## 9. Biological accuracy concerns

General: the thirteen concepts come from at least four kinds of organism — solitary amoebae (phagocytosis, pseudopodia), the multicellular stage of social amoebae (differentiation, sentinel cells, developmental death), bacteria (quorum sensing) and animals (immune system, apoptosis) — plus a plasmodial slime mould for "memory". RES §0 and §1 state this. Two analogies are inverted relative to the biology (digestion, quorum). The biology therefore supplies vocabulary, not design guidance; external material should not suggest otherwise, as RES §4 already warns.

Spot-check (seven lookups; verdicts are based on search-result summaries — **I did not open and read the sources in full**; the URLs are those returned by the search tool):

| # | Claim in RES | Verdict | Source located (title — URL) |
|---|---|---|---|
| 1 | Immune-like phagocytic "sentinel" cells reported in *Dictyostelium* multicellular stages (Chen, Zhuchenko, Kuspa, Science 2007) | **Supported** (summary level) | "Immune-like Phagocyte Activity in the Social Amoeba" — https://typeset.io/pdf/immune-like-phagocyte-activity-in-the-social-amoeba-1mw27q9lt7.pdf ; press release — https://www.eurekalert.org/pub_releases/2007-08/bcom-iis073007.php |
| 2 | *Dictyostelium* stalk-cell death is developmental and generally described as non-apoptotic (vacuolar/autophagic) | **Supported** (summary level); summaries add that a non-vacuolar death path appears when autophagy is disabled | "Autophagy gene disruption reveals a non-vacuolar cell death pathway in Dictyostelium" — https://pubmed.ncbi.nlm.nih.gov/15358773 ; Cell Death and Differentiation (2009) 16, 21–30 — https://tavernarakislab.gr/publications/CDDAut.pdf |
| 3 | Density sensing through a secreted factor ("conditioned medium factor") in *Dictyostelium* | **Supported** (summary level) | "The cell density factor CMF regulates the…" — https://rupress.org/jcb/article/134/6/1543/15376/The-cell-density-factor-CMF-regulates-the ; https://pmc.ncbi.nlm.nih.gov/articles/PMC2120463 |
| 4 | Habituation-like behaviour reported in slime moulds; interpretation debated; *Physarum* is a plasmodial slime mould | **Partly**: the report exists (Boisseau, Vogel, Dussutour 2016, "Habituation in non-neural organisms: evidence from slime moulds"). The statement that interpretation is debated, and the taxonomy remark, were **not verified** | https://www.sciencedaily.com/releases/2016/04/160427081533.htm ; https://www.nature.com/articles/533010d ; direct fetch of https://pubmed.ncbi.nlm.nih.gov/27122563/ **failed** (page returned no content) |
| 5 | Hearsay-II is the classic blackboard system; scheduling ("the control problem") is a known hard part | **Supported** (summary level) | Hayes-Roth, "A blackboard architecture for control" (catalogue entry) — https://www.bohrium.com/paper-details/a-blackboard-architecture-for-control/812416871067287553-2508 ; Hearsay-III paper — https://cdn.aaai.org/AAAI/1980/AAAI80-032.pdf |
| 6 | Knight & Leveson 1986: independently developed versions fail together more often than independence predicts | **Supported** (summary level): 27 versions, coincident failures well above the independence prediction. The experiment used student programmers and was contested at the time; that should be said when it is cited | "An Experimental Evaluation of the Assumption of Independence in Multi-Version Programming" — https://libraopen.lib.virginia.edu/public_view/jd472w463 ; https://www.kth.se/social/files/564df871f2765419e306178d/KnightLeveson.pdf |

**Not checked** (remain as the authors left them, unverified): phagosome/lysosome statements, cAMP and folate chemotaxis, contractile vacuole, *Stentor* regeneration, autophagy summary, the Amoeba operating-system name collision. RES AR-01 remains open for these and for full-text confirmation of the six rows above.

---

## 10. Complexity and duplication findings

| Finding | Evidence | Consolidation proposal |
|---|---|---|
| **"Ambiguous" in five places** | ARCH-05 stage outcome; S02 `EngineeringOutcome::Ambiguous`; Issue kind `ambiguity`; coverage state `ambiguous`; hypothesis verdict `alternative`; gate G02 | One source of truth: the **Issue**. `EngineeringOutcome` values and RC results *create* Issues; coverage state and gate results are *computed* from open Issues, never stored independently |
| **"Unsupported" in five places** | ARCH-05; `EngineeringOutcome::Unsupported`; Issue kind; coverage state; `ObservationSet.unsupported` | Same rule |
| **Quarantine in three forms** | Issue kind `quarantine`; Lifecycle record `Quarantine`; lifecycle status `quarantined` (EOS §2, §4) | Keep the record; derive the status; drop the Issue kind or make it a pointer |
| **ObservationSet vs EOS view** | ARCH §8 calls it "a signed view of an EOS version", but S03 §10 gives it its own `conflicts`, `gaps`, `hypotheses`, `unsupported`, `review` lists | Define the ObservationSet as a signed *selector* (record ids + Issue ids + coverage statement id); remove embedded copies. Also state what invalidates the signature (W5) |
| **Three provenance ledgers** | Record `method {component, version, weights hash}` (S03 §6.1); `Calculation` record (S01 §6.1); `ProcessInstance` (EOS §2); plus the job graph and the audit log | ProcessInstance is the single execution record; `Calculation` becomes its typed payload for UEM calls; records carry only the process id |
| **Signals vs job orchestration** | Durable job graph (S01 §2), derived SignalLog, job state in Heads | One mechanism: rounds planned from the EOS (W4). Signals are an implementation detail of the planner, not an architectural component or an ADR of their own |
| **Derived data inside the state hash** | `coverage_root` and `process_root` in `eos_version` (EOS §1) although coverage is computable from records and issues | Hash primary data only; coverage ledger is a materialised view verified by recomputation (M3a) |
| **Issue state is mutable in an append-only state** | `state: open | resolved | accepted_risk`, "history" (EOS §2) vs AMB-04; whether `accepted_risk` counts as "open" for `fully_examined` is unstated | Issue = immutable opening record + resolution records; state derived |
| **Lanes vs G14 vs qualification record vs "experimental versions"** | ARCH §7 AMB-S5; S02 §10, §12; S01 §7 | One definition (lanes); G14 references it (MIG A-02-2 already proposes this) |
| **Quorum vs corroboration vs independence classes** | MECH §13; S03 §3.6; S02 §9.1 | One "independent-evidence rule" artefact with per-rule tables |
| **Term collisions** | "Coverage" = ledger (COV), "explanatory coverage" (S03 §5.5), "coverage interval" (S02 ALG-11, metrology usage), "alternative coverage" (BENCH H1). "Hypothesis" = interpretation hypothesis (S03) and benefit hypothesis [H] | Rename the ledger concept (e.g. "examination accounting"); reserve "coverage interval" for uncertainty |
| **Concept count** | 11 phases, 5 zones, 13 mechanisms, 8 EOS components, 8 domains × 5 states, 17 gates, 3 lanes, 4 axes | Drop the 13 biological names from normative text; keep plain names (the authors already propose this for code, ARCH §10 item 10) |
| **Housekeeping inconsistencies** | PADR header and MIG §7 say "ADR-009…014" while the table lists 009–016; specification numbering differs between MP §20 (04 = Drawing Intelligence, 06 = Verification), S01 ("Verification Specification (05)", "Specification 07" for security) and S03 ("reconstruction engine (Spec 04)") | Fix in the next revision; state one numbering |

---

## 11. ADR-by-ADR challenge

| ADR | Recommendation | Reasons and conditions |
|---|---|---|
| **009 AMOEBA as governing architecture** | **Accept with conditions — split** | The stated evidence (B-1…B-4) needs a running system, while Spec 04 is on hold until the ADR is decided (MIG §7): a circular dependency. Split into **009a** (one state, uniform process contract, examination accounting, lanes — acceptable as the governing *framework* now, subject to W5, W7, W8, W9) and **009b** (adaptive investigation as the primary mode — provisional until the pre-registered benchmark of §5.0). Conditions for 009a: engineering-side process catalogue and feedback edges (W1); dependency model (W2); plain names in normative text. Without W1 and W2 resolved, "governing" is a label |
| **010 Engineering Organism State** | **Accept with conditions** | Conditions: release closure hash replaces whole-EOS binding (W5); only primary data hashed; Issues immutable with derived state; typed dependency edges and interface constraints (M1); engineering-side tree specified for the first domain *and* a written domain contract; global-dependency rule for views (§7.4); rule for record reuse across Snapshots |
| **011 Source-denominated coverage and gate G17** | **Ledger: accept with conditions. Gate G17: defer** | The ledger is the strongest idea in the set. Conditions: sixth state for exclusions; single definition of "explained"; versioned applicability table; rename to avoid "coverage" collision. G17 is deferred until the INCONCLUSIVE rule is workable for scans (W7), enumeration cost and stability are measured (B-5), and reviewer workload for exclusions is known. Until then, report the statement and require engineer acknowledgement of every non-`fully_examined` count in scope |
| **012 Bounded, deterministic adaptive exploration** | **Defer** | The priority function does not exist (W3); the determinism model is unsound (W4); the confidence rule is contradictory (W6); the baseline in B-4 cannot show benefit (W10). Bring back with: round-based scheduling, work-unit budgets, M2-style declared expected effect, at least three engineering probes, per-region budget caps (§8.1), and the benchmark design of §5.0 |
| **013 Lanes, quarantine and retirement** | **Accept with conditions** | Conditions: overlay isolation so non-production output cannot influence production scheduling (W8); taint defined over the release closure; quarantine scope rules (§8.5); a plan for mass re-opening after version retirement (§8.6); statement of how old model-based versions remain executable for the retention period |
| **014 Signals as derived events** | **Reject as a separate ADR; fold into 012** | Signals are a mechanism of the scheduler, not an architectural decision with its own alternatives. The stated evidence ("order-independence under parallel execution") is addressed by the scheduling model, not by signals. Resolve persisted-versus-derived and deterministic signal identity (§8.9) inside ADR-012 |
| **015 Independent-evidence thresholds** | **Accept the principle; defer the rule** | Accept now: declared shared dependencies count once; the maximum outcome is `corroborated`; no credit without measurement. Defer the threshold rule itself: no method yet converts measured correlation into credit (RES AR-07), and benchmark-measured correlation may not transfer to a given customer's drawings. With all credit at zero the ADR has no operational effect until B-6. Also fix the recogniser set per element type (W6) |
| **016 Tenant knowledge governance** | **Defer** | Legal basis is open (RES AR-08). Accept only the interim rule: propose-only, tenant-scoped, no learning from customer data without recorded permission (already S01 §8). Before acceptance: controls on exclusion templates (§8.3); a rule that memory may never pre-fill a class-H value in the confirmation screen (anchoring; BENCH B-8); retirement behaviour |

---

## 12. Proposed amendments (not applied)

| # | Document, section | Amendment |
|---|---|---|
| 1 | ARCH §1, §4, §8; MIG §4 | Add Build/Verify process catalogue, engineering signals and feedback edges from phases 8–9 to 4–6 (W1) |
| 2 | EOS §2 | Add typed dependency edges and `InterfaceConstraint` records; define propagation rule table (W2, M1) |
| 3 | ARCH §7 AMB-20; MECH §5 | Replace "pure function of EOS version" by round-based scheduling; budgets in work units; state termination condition (W3, W4) |
| 4 | EOS §1 | `eos_version` hashes primary data only; remove resource usage and job status from hashed content (W4, §10) |
| 5 | ARCH §7 AMB-S3; EOS §5 inv. 5 | Approval signs the release-closure hash, consistent with S02 G16; invariant 5 reworded to G01–G15 (+G17) (W5) |
| 6 | ARCH §7 AMB-S4; MECH §5; BENCH B-1 | Resolve the confidence contradiction; restate the invariant as "never more favourable"; fix recogniser sets for corroboration (W6) |
| 7 | COV §3, §5; ARCH §6 | Sixth state `excluded_by_decision`; single definition of "explained"; applicability table; workable INCONCLUSIVE rule; restrict the phrase "fully inspected" (W7) |
| 8 | EOS §5 inv. 7; MECH §8 | Overlay isolation for non-production lanes; taint over release closure (W8) |
| 9 | ARCH §2 | Declare runtime, schema gate and coverage evaluator a trusted computing base with qualification requirements (W9) |
| 10 | BENCH B-4 | Baselines B0–B4; defect-discovery endpoints; statistical design; pre-registered decision rule; H3/H4 moved out of the adaptivity test (W10) |
| 11 | S03 §5.5; S02 G02 (proposal only) | Sole survivor with incomplete search is `assumed` and requires a Decision; assumption list at release (W11) |
| 12 | EOS §2 Issues; S03 §10; S01 §6.1 | Consolidations of §10: Issue as single source for ambiguity/unsupported; ObservationSet as selector; ProcessInstance as single execution record |
| 13 | ARCH §5; MECH | Normative text uses plain engineering names; biological names confined to an informative annex |
| 14 | COV, S02 ALG-11, BENCH | Resolve "coverage" terminology collision |
| 15 | PADR; MIG §7; MP §20; S01; S03 | Correct ADR range (009–016) and specification numbering |
| 16 | BENCH B-1, B-3 | Add tests: budget displacement by hostile regions; instruction-like text in drawings; exclusion-template misuse (§8) |
| 17 | RES §1 | Record the spot-check results of §9; cite Knight & Leveson with its limits; complete AR-01 with full-text reading |
| 18 | New note | Domain contract: what a new engineering domain must supply to run under AMOEBA (§7) |
| 19 | MECH (new) | Mechanisms M1–M5 as proposed designs with hypothesis tags, if the owner accepts them |

---

## 13. Prioritised roadmap (implementation-independent)

**Before Specification 04 — decisions on paper**

1. **Owner decision on scope of "primary".** Confirm the split 009a/009b: the framework governs now; adaptive investigation is primary *subject to* a pre-registered test. Record what result would reduce it.
2. **Dependency model and engineering-side process catalogue** (W1, W2). Spec 04 cannot be "native" without them; this is the first design task, done on two or three worked paper examples (e.g. header with branch, eccentric reducer with flanges, rectangular-to-round transition) tracing a seeded defect through the dependency edges by hand.
3. **Determinism and approval semantics** (W4, W5): round model, work-unit budgets, release closure. A small formal model of rounds, lanes and the release state machine (RES AR-05) is the appropriate evidence.
4. **Examination accounting corrections** (W7) and the applicability table for the drawing side.
5. **Safety loophole closures** (W6, W8, W9, W11) written as requirements with tests.
6. **Consolidation** (§10) — before more record types are added by Spec 04.
7. **Benchmark design** (§5.0), including the primary endpoint and decision rule, agreed and frozen *before* any priority function is tuned.

**Proven by experiment, in this order, once a minimal system exists**

8. B-1/B-2 on the non-adaptive framework (safety invariants, replay under the round model).
9. Enumeration cost and stability (B-5) on synthetic and then authorised scans — decides G17.
10. Recogniser and verifier dependence (B-6) — decides whether ADR-015 has any operational content.
11. Adaptivity versus B1–B3 (H1, H2 and the defect-discovery endpoints), then M1, M2, M4, M5 individually, each switched on alone so that its contribution can be separated.

**Evidence that would falsify the claim that AMOEBA provides a real engineering advantage**

- At equal work-unit budget, signal-directed investigation does not find more seeded and natural defects, or reach an approvable state with fewer engineer minutes, than the *uniform* (B1) and *static-heuristic* (B3) baselines, beyond the pre-registered minimum effect.
- The best *static* profile chosen per input class at ingestion performs as well as in-job adaptation (M4 unnecessary).
- Dependency-following (M1) finds no more secondary defects per unit budget than running all cross-checks on everything, i.e. the checks are cheap enough that selectivity has no value.
- Predicted and realised probe effects (M2) are uncorrelated — the scheduler's judgement carries no information.
- Deterministic replay cannot be achieved under parallel execution at acceptable cost, or the adaptive runtime's qualification cost exceeds the review time it saves.
- Engineers in B-5 do not regard "all enumerated content accounted for" as corresponding to their notion of an inspected drawing, or exclusion workload exceeds the review time saved.

If these outcomes occur, the defensible architecture is Specs 01–03 plus the EOS corrections, examination accounting, lanes and quarantine — a strictly conventional design with one good new ledger — and the documents should say so plainly.
