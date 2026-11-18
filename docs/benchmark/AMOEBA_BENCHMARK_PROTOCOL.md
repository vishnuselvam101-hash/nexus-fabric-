# AMOEBA Benchmark Protocol

Protocol version 1.0 · 18 November 2026 · Governs ADR-009B and ADR-012 · Supersedes section B-4 of the [AMOEBA Architecture Benchmark](AMOEBA_ARCHITECTURE_BENCHMARK.md) (that document is preserved; its other sections remain in force as safety and scale tests)

## 0. Freeze status

- **Structure frozen:** arms, budgets, probe-set rule, metrics, analysis method, ablations and falsification criteria below are fixed as protocol v1.0. They were written before any priority function, discrimination table or strategy profile exists, and before any data was collected.
- **Parameters awaiting owner sign-off** (table §9): numerical values that a protocol must fix in advance but that only the owner can set (minimum effect sizes, significance level, budget levels). They are proposed here. Once signed, any change creates protocol v1.1+, and results obtained under an earlier version cannot be pooled with later ones.
- **Nothing has been run.** No system and no corpus exist. No claim of any kind follows from this document.

## 1. Question

At an identical budget of logical work units, with an identical catalogue of available probes and checks, does AMOEBA's adaptive investigation find more engineering defects — especially critical ones — earlier, without more false alarms, than conventional investigation?

Architectural identity (ADR-009A) does not depend on the answer. The effectiveness claim (ADR-009B) does.

## 2. Arms

All arms run on the same framework (same EOS, same mandatory work, same probe/check catalogue, same recognisers, same human-input simulation). They differ **only** in how discretionary budget is spent.

| Arm | Discretionary investigation | Role |
|---|---|---|
| **A0 Fixed** | None beyond mandatory work | Conventional baseline; budget floor |
| **A1 Uniform** | The full probe catalogue applied to all eligible elements in a fixed canonical order until the budget is spent | Controls for "more effort" |
| **A2 Signal-directed** | Priority from recorded signals (findings, coverage states, uncertainty, risk weight); no dependency propagation beyond mandatory | Tests adaptive prioritisation |
| **A3 Dependency-directed** | Discretionary obligations along dependency edges (lateral and common-cause beyond the mandatory hop); catalogue-order tests | Tests the dependency model |
| **A4 Combined AMOEBA** | A2 + A3 + hypothesis-driven test selection + strategy switching + stopping rule | The full proposal |

Supplementary controls (reported, not part of the primary comparison): **R** random targeting with the same catalogue (seeded, repeated); **H** static heuristic (re-probe all numeric text and all interfaces first); **O** oracle targeting from ground truth (upper bound on what targeting could achieve).

Mandatory work — baseline examination, mandatory obligations, family verification, audits — is identical in every arm, including A0. The comparison is therefore about discretionary investigation only; safety must not depend on it.

## 3. Budgets

Budgets are in logical work units ([Determinism](../architecture/amoeba/DETERMINISM_AND_APPROVAL.md) §3). For each job, `M` = work units of its mandatory work. Arms A1–A4 receive total budget `M × b` for each budget level `b` in the signed parameter table. Results are reported as curves over `b`, not at a single favourable level. Wall-clock time, memory and energy are recorded as secondary resource metrics.

## 4. Corpus

| Component | Content | Use |
|---|---|---|
| **Seeded synthetic** | Generated multi-part projects (headers with branches, reducers between pipes, transitions between frames and ducts, multi-sheet packages) with **seeded defects** of each class, at known locations, with known *secondary* defects (the consequences elsewhere that a dependency-aware system should find) | Development and mechanism studies; never sufficient for a superiority claim |
| **Authorised real drawings — clean** | Factory jobs believed correct, with approved outputs | False-alarm measurement |
| **Authorised real drawings — natural defects** | Historical jobs where an error was found (in checking, on the shop floor, by the customer), with the documented defect | The evidence that matters most; reported separately from seeded results |
| **Real drawings with seeded defects** | Authorised drawings with controlled modifications | Bridges synthetic and natural |

**Defect taxonomy** (each with severity *critical / major / minor* assigned by fabrication engineers before any run, by the consequence if released): reading error · association error · inter-view or inter-document contradiction · missing dimension · projection/mirror · surface-reference (inside/outside) · unit/scale · revision inconsistency · interface mismatch between parts · geometric invalidity · calculation fault (injected algorithm defect) · process-data out of range or missing calibration · seam/plate-division rule violation · output (DXF) fault.

**Splits.** Development set (tuning of tables, priorities, profiles, thresholds) and a **sealed held-out set** (seeds and files unavailable to developers). Stratified by defect class, input type and project size. The held-out set is run **once per frozen configuration**; the configuration hash is recorded before unsealing.

**Sample size.** Determined before the held-out run by a power calculation using variance estimated on the development set, for the signed minimum effect and significance level. If the available authorised corpus is smaller than required, results are reported as *inconclusive* — not as support.

- **No superiority claim may be made from synthetic data alone, or from a real-drawing set below the pre-computed sample size.**

## 5. Metrics

| Metric | Definition |
|---|---|
| Defect detection rate (recall) | Seeded/known defects for which a Finding identifying the correct records was raised before the stop, per class and severity |
| **Critical-defect recall** | The same, critical severity only — **primary endpoint** |
| Secondary-defect recall | Recall on the dependent consequences of a primary defect (the measure of whole-design intelligence) |
| False-positive rate | Findings with no corresponding defect, per job and per work unit, judged by adjudicating engineers blind to the arm |
| False-negative rate | 1 − recall, with every missed critical defect analysed individually |
| Explanation accuracy | Investigations closed with the correct explanation class ÷ investigations closed |
| Detection latency | Work units consumed before each defect's first correct Finding; area under the detection-vs-budget curve |
| Coverage | Coverage vector at stop (declared enumeration level) |
| Unresolved risk | Content of the Residual Uncertainty Statement at stop: open findings, open obligations, unexplained investigations, assumption-ledger entries — and, for seeded sets, whether undetected defects lie within what the statement flagged as unresolved |
| Prediction quality | Correlation and calibration between recorded expected and actual test outcomes (Investigation Model INV-03) |
| Reproducibility | Identical primary state on replay (same platform); results across worker counts |
| Resource consumption | Work units (controlled), wall-clock, peak memory, storage growth |
| Human effort | Simulated or measured reviewer actions to reach an approvable state; in reviewer studies, minutes and error rate |

## 6. Safety prerequisites (must pass before any arm's results count)

S1 safety invariants (Architecture Benchmark B-1, with the confidence test as revised in the Safety refinement S-3) · S2 deterministic replay under the round model · S3 mandatory work identical across arms and unaffected by signals · S4 no arm releases a job that A0 would block. A failing arm is disqualified, whatever its detection rate.

## 7. Analysis

- **Design:** paired — every job is run under every arm at every budget level.
- **Primary comparison:** A4 versus A1 on critical-defect recall at the signed primary budget level, held-out set, real drawings.
- **Key secondary comparisons:** A2 vs A1 (does signal direction help?); A3 vs A1 (does dependency direction help?); A4 vs max(A2, A3) (do they combine?); every arm vs H; distance to O.
- **Statistics:** paired tests on per-defect detection (McNemar-type for binary detection; paired bootstrap over jobs for rates and latency); confidence intervals reported for every difference; family-wise error controlled across the pre-declared comparisons (Holm procedure); non-inferiority tests for false-positive rate and human effort at the signed margins.
- **Reporting:** seeded and natural defects separately; by class and severity; full budget curves; every missed critical defect listed; negative and inconclusive results reported with the same prominence.
- **Blinding:** adjudicators of false positives and explanation correctness do not know the arm.

## 8. Ablations

From A4, remove one mechanism at a time, same budgets, same data:

| Ablation | Removed | Shows the contribution of |
|---|---|---|
| A4−sig | Signal-based priority (replaced by canonical order) | Adaptive prioritisation |
| A4−dep | Discretionary dependency propagation | Dependency-following beyond the mandatory hop |
| A4−hyp | Expected-benefit test selection (catalogue order instead) | Hypothesis-driven selection |
| A4−str | Strategy switching (best single profile fixed at ingestion) | In-job adaptation |
| A4−stop | Yield-based stopping (run to budget) | Stopping rule (efficiency, not recall) |
| A4−conf | Confidence as a discretionary priority input | Whether confidence helps targeting at all |

Additionally, outside the arm comparison: mandatory dependency propagation and audits A1–A5 are evaluated by seeded-fault detection (they are reliability mechanisms kept regardless of this benchmark), with their cost reported.

A mechanism whose removal does not worsen the pre-declared metrics beyond the signed minimum effect is **not shown to contribute** and is a candidate for removal on complexity grounds.

## 9. Parameters awaiting owner sign-off (proposed values)

| Parameter | Proposed | Note |
|---|---|---|
| Budget levels `b` | 1.25, 1.5, 2, 4 | Primary level: 2 |
| Significance level (family-wise) | 0.05 | |
| Minimum effect — critical-defect recall, A4 vs A1 | +5 percentage points absolute | Smaller differences are treated as "no practical advantage" |
| Alternative efficiency effect | Equal recall reached with ≥ 20 % fewer work units | |
| Non-inferiority margin — false-positive rate | ≤ +2 percentage points relative to A1 | |
| Non-inferiority margin — human effort | ≤ +5 % | |
| Power | 0.8 | For sample-size calculation |
| Minimum share of natural-defect jobs in the held-out set | To be set once the factory states how many exist | Open requirement |

These are engineering judgement, not derived quantities; the owner should set them with the factory's view of what difference would matter in practice.

## 10. Falsification criteria (fixed before testing)

The effectiveness claim (ADR-009B) is **not supported** if, on the held-out real-drawing set at the primary budget level, any of:

| ID | Criterion |
|---|---|
| F1 | A4 does not exceed A1 on critical-defect recall by the minimum effect, and does not reach equal recall with the stated work-unit saving |
| F2 | A4 exceeds the non-inferiority margin on false positives or human effort |
| F3 | A4 does not exceed the static heuristic H by the minimum effect (a fixed rule does as well) |
| F4 | Neither A2 nor A3 exceeds A1 (no component of the adaptivity helps) |
| F5 | Expected and actual test outcomes are uncorrelated (the scheduler's judgement carries no information) |
| F6 | Any safety prerequisite fails with adaptivity enabled and cannot be restored without removing the adaptive mechanism |
| F7 | The advantage appears only on synthetic or seeded data and not on natural defects |

Consequences are decided now: if F1–F5 or F7 hold, adaptive investigation is switched off by default and offered at most as an experimental option; mechanisms failing their ablation are removed; the dependency obligations, audits, coverage ledger, lanes and release closure — which do not depend on this benchmark — remain. AMOEBA remains the governing architecture (ADR-009A) in either case, with its adaptive component honestly described as unproven.

## 11. What this protocol cannot show

Field performance on drawing types outside the corpus; physical conformity of fabricated parts (benchmark tier P); effects on engineers' vigilance over months of use; benefit in other fabrication domains. Each needs its own study.

## 12. Dependencies before the protocol can run

A minimal system implementing the framework and all arms; cost functions; probe catalogue; synthetic project generator with defect seeding and secondary-defect ground truth; authorised drawings (`FR-10-1…9`), including documented historical defects (new requirement **FR-10-10**: records of past drawing or development errors and how they were found); severity classification by fabrication engineers; adjudicators.
