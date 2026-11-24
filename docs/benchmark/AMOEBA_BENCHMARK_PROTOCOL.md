# AMOEBA Benchmark Protocol

Protocol version 1.3 · 18 November 2026 (change log in §13) · Governs ADR-009B and ADR-012 · Supersedes section B-4 of the [AMOEBA Architecture Benchmark](AMOEBA_ARCHITECTURE_BENCHMARK.md) (that document is preserved; its other sections remain in force as safety and scale tests)

## 0. Freeze status

- **Structure frozen:** arms, budgets, probe-set rule, metrics, analysis method, ablations and falsification criteria below are fixed as protocol v1.0. They were written before any priority function, discrimination table or strategy profile exists, and before any data was collected.
- **Parameters: provisional, preregistered** (table §9), approved by the owner on 18 November 2026. They are experimental design choices, not scientific constants. They **may be revised before the benchmark is executed** (each revision is a new protocol version with a recorded reason) and **may not be revised after any held-out result has been observed**. Results under different protocol versions are never pooled.
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
| **Major-defect recall**, **minor-defect recall** | As above for each severity. The three severities are always reported **separately**; a single pooled recall figure is never the headline |
| False-negative rate | 1 − recall per severity, with every missed critical defect analysed individually |
| **Dependency-propagation effectiveness** | Of the defects that are consequences of another defect (secondary defects), the share whose first correct Finding was produced through a scrutiny obligation; and obligations that led to a confirmed Finding ÷ obligations created (yield), by direction (downstream / lateral / common-cause) |
| **Dependency-edge failures** | Secondary defects missed, or found late, **because a dependency edge or rule-table entry was missing or wrong** — established by comparing the system's dependency graph with the ground-truth dependency graph of each seeded project. Reported as: missing-edge rate, spurious-edge rate, and missed defects attributable to each |
| Spurious obligations | Obligations created along incorrect edges or discharged with no relevant content — a direct contributor to review burden |
| Explanation accuracy | Investigations closed with the correct explanation class ÷ investigations closed |
| Detection latency | Work units consumed before each defect's first correct Finding; area under the detection-vs-budget curve |
| Coverage | Coverage vector at stop (declared enumeration level) |
| Unresolved risk | Content of the Residual Uncertainty Statement at stop: open findings, open obligations, unexplained investigations, assumption-ledger entries — and, for seeded sets, whether undetected defects lie within what the statement flagged as unresolved |
| Prediction quality | Correlation and calibration between recorded expected and actual test outcomes (Investigation Model INV-03) |
| Reproducibility | Identical primary state on replay (same platform); results across worker counts |
| Resource consumption | Work units (controlled), wall-clock, peak memory, storage growth |
| **Human review burden** | Per job: number of items presented for review (findings, obligations, confirmations, exclusions), reviewer actions to reach an approvable state, share of presented items that were false alarms; in reviewer studies, minutes and reviewer error rate. Reported alongside every recall figure |
| **Additional computational cost** | Work units beyond mandatory work (controlled by design), plus wall-clock, memory and storage relative to A0 |

## 6. Safety prerequisites (must pass before any arm's results count)

S1 safety invariants (Architecture Benchmark B-1, with the confidence test as revised in the Safety refinement S-3) · S2 deterministic replay under the round model · S3 mandatory work identical across arms and unaffected by signals · S4 no arm releases a job that A0 would block. A failing arm is disqualified, whatever its detection rate.

## 7. Analysis

- **Design:** paired — every job is run under every arm at every budget level.
- **Primary comparison:** A4 versus A1 on critical-defect recall at the signed primary budget level, held-out set, real drawings.
- **Key secondary comparisons:** A2 vs A1 (does signal direction help?); A3 vs A1 (does dependency direction help?); A4 vs max(A2, A3) (do they combine?); every arm vs H; distance to O.
- **Statistics:** paired tests on per-defect detection (McNemar-type for binary detection; paired bootstrap over jobs for rates and latency); confidence intervals reported for every difference; family-wise error controlled across the pre-declared comparisons (Holm procedure); non-inferiority tests for false-positive rate and human effort at the signed margins.
- **Conclusion rule:** a statistically significant difference alone is **not** sufficient. Support for the claim requires all of: the point estimate meets the minimum practical effect; the confidence interval excludes zero after correction; the pre-computed sample size was reached on the sealed real-drawing set; non-inferiority holds for false positives and human review burden; and no safety prerequisite failed. Otherwise the result is reported as *not supported* or *inconclusive*.
- **Reporting:** critical, major and minor defects separately; seeded and natural defects separately; by class and severity; full budget curves; every missed critical defect listed; negative and inconclusive results reported with the same prominence.
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

## 9. Provisional preregistered parameters (protocol v1.2, approved 18 November 2026)

These are **experimental design thresholds chosen by the project owner — not industry standards and not scientific constants.** They may be revised (as a new protocol version, with reason) at any time **before** the held-out benchmark is executed, and never after any held-out result has been observed.

| Parameter | Value | Justification |
|---|---|---|
| Primary budget level | 2 × mandatory-work cost | A single pre-declared level prevents choosing the most favourable point of the curve afterwards |
| Additional budget levels | 1.25 ×, 1.5 ×, 4 × | Descriptive curve from "a little extra" to "several times" the mandatory cost |
| Significance level | 0.05 family-wise, Holm correction | Conventional; says nothing about practical importance |
| Statistical power | ≥ 0.80 for the primary comparison at the minimum effect | Conventional minimum; determines the required sample size |
| Minimum practically meaningful improvement | +5 percentage points critical-defect recall, A4 vs A1 | Owner's judgement of the smallest difference likely to matter; to be revisited with the factory |
| False-positive non-inferiority margin | 2 percentage points (absolute) | Owner's judgement |
| Human review burden non-inferiority margin | 10 % relative increase | Owner's judgement |
| Known critical dependency omissions reaching release | **Zero tolerated** | A critical dependency failure must not be averaged away by good performance elsewhere |
| Efficiency criterion (**secondary** success criterion) | ≥ 20 % fewer logical work units at equal critical-defect recall | Approved. Conditions in §9a. It is secondary: it cannot substitute for a failed safety prerequisite or a zero-tolerance failure |
| Non-inferiority margin for "equal" critical-defect recall in the efficiency criterion | **Open — must be set before execution** | "Equal within the preregistered margin" needs a number that has not been decided; the efficiency criterion cannot be evaluated until it is |
| Share of natural-defect jobs in the held-out set | **Deliberately not set.** To be decided only after the factory's historical data has been seen | No percentage is invented in advance |

## 9a. Statistical analysis plan

**Units.** The *job* (one engineering package) is the sampling unit. Defects within a job are not independent, so all interval estimates and tests resample or pair **by job**. Every job is run under every arm at every budget level (paired design). All confirmatory analyses use the sealed held-out set of authorised real drawings at the primary budget level; other budget levels and all synthetic results are descriptive.

**Metric definitions and denominators.**

| Metric | Numerator | Denominator |
|---|---|---|
| Recall, severity *s* (critical / major / minor — each separately) | Ground-truth defects of severity *s* for which, before the stop, a Finding was raised that references at least one ground-truth record of that defect | All ground-truth defects of severity *s* in the analysed jobs |
| False-negative rate, severity *s* | Ground-truth defects of severity *s* not detected | Same as recall |
| False-positive proportion | Findings presented for review that adjudicators (blind to arm) judge to correspond to no defect | All Findings presented for review |
| False findings per clean job | False findings | Jobs known to contain no defect |
| Human review burden | Items presented for human action (findings, obligations, confirmations, exclusions) until an approvable state | Per job; compared as ratio of arm means |
| Additional computational cost | Work units beyond mandatory work (also wall-clock, memory) | Per job |
| Detection latency | Work units consumed before the first correct Finding for a defect; undetected defects censored at the budget | Per defect; summarised as restricted mean and detection-vs-budget curve |
| Secondary-defect recall | Detected secondary (consequential) defects | All ground-truth secondary defects |
| Propagation attribution | Secondary defects whose first correct Finding arose from a scrutiny obligation | Detected secondary defects |
| Obligation yield | Obligations leading to a confirmed Finding | Obligations created, by direction |
| Reproducibility | Jobs whose replay reproduces the identical primary state | Jobs replayed |
| Residual-uncertainty capture | Undetected defects lying within items the Residual Uncertainty Statement listed as unresolved | Undetected defects |

**Dependency-edge failures — three categories, always reported separately.** Ground truth is a dependency graph prepared for each benchmark project by engineers, with each edge marked critical or non-critical.

| Category | Definition | Numerator / denominator | Treatment |
|---|---|---|---|
| **K — Known dependency omitted** | An edge in the ground-truth graph that the system's graph lacks (or whose rule-table entry is missing) | Omitted ground-truth edges ÷ ground-truth edges; for **critical** edges, each omission listed individually with whether the dependent defect was nevertheless detected and whether the job reached a releasable state | **Zero tolerance:** any known *critical* dependency omission whose dependent defect escapes detection and whose job passes all automated gates is a protocol failure (F8), irrespective of every other result |
| **U — Unknown dependency discovered** | A real dependency evidenced during testing that is in neither the ground truth nor the system's graph | Count; each described | Not scored for or against any arm; triggers a recorded revision of the ground truth and the rule table for *future* protocol versions; never used to alter the current run's results |
| **S — Incorrect (spurious) dependency** | An edge in the system's graph that is not in the ground truth, and the unnecessary obligations it created | Spurious edges ÷ system edges; spurious obligations ÷ obligations; review items caused | Counts toward human review burden and computational cost |

Because zero observed failures in a finite sample does not show a zero rate, a zero count for category K-critical is reported with its sample size and an upper confidence bound (for 0 events in *n* critical edges, the one-sided 95 % upper bound is approximately 3 ⁄ *n*).

**Estimation.** Every proportion is reported with a 95 % confidence interval (Wilson interval for single proportions; cluster bootstrap over jobs for differences, ratios and latency summaries). No result is reported as a bare point estimate.

**Confirmatory tests.**

| Test | Hypothesis | Method | Multiplicity |
|---|---|---|---|
| Primary (superiority) | Critical-defect recall, A4 − A1 > 0 | Paired, job-clustered bootstrap test; defect-level McNemar test as a sensitivity analysis | Holm family, first |
| Secondary (superiority) | Critical-defect recall: A2 − A1; A3 − A1; A4 − H; A4 − max(A2, A3). Secondary-defect recall: A3 − A1 | Same | Holm family with the primary |
| Non-inferiority: false positives | False-positive proportion A4 − A1 < 2 percentage points | Upper limit of the one-sided 95 % interval below the margin | Required conjunct; no adjustment (all conjuncts must hold) |
| Non-inferiority: review burden | Ratio of mean review burden A4 ⁄ A1 < 1.10 | Upper limit of the one-sided 95 % interval below 1.10 | Required conjunct |
| Zero tolerance | Category K-critical escapes = 0 | Count | Required conjunct |

**Efficiency criterion (secondary).** Supported only if all hold: (1) critical-defect recall of the adaptive arm at the reduced budget is non-inferior to the comparison arm at the full budget, within the preregistered recall margin (interval-based, as for the other non-inferiority tests); (2) work units are lower by at least 20 %, with the confidence interval for the saving excluding smaller values; (3) false-positive and human-review-burden margins are not exceeded; (4) the comparison is on the same jobs with the same defect-severity composition (paired design; any imbalance reported); (5) confidence intervals are reported for every quantity involved.

**Natural versus injected defects.** *Naturally occurring factory errors* (found historically in checking, on the shop floor or by a customer) answer: does the system find the kinds of mistakes that really happen, at their real frequency and subtlety? *Deliberately injected defects* (synthetic, or seeded into real drawings) answer: can the system find a defect of a known class at a known place, and how does detection vary with controlled factors? Neither substitutes for the other. They are never pooled; every metric is reported for each separately, and the confirmatory conclusion requires the natural-defect result to point the same way (F7).

Major- and minor-defect recall, and all other metrics, are reported with intervals for every arm but are not confirmatory.

**Power and sample size.** Before the held-out run, the number of jobs and critical defects needed for power ≥ 0.80 on the primary test at the +5-point minimum effect is computed from development-set estimates (baseline recall in A1, between-job variance, discordance between arms). If the sealed real-drawing set is smaller, the confirmatory analysis is not run as such and all results are labelled *inconclusive*.

**Support requires all of:** point estimate ≥ minimum effect; corrected interval for the primary difference excludes zero; required sample size reached; both non-inferiority conjuncts hold; zero K-critical escapes; safety prerequisites passed. Statistical significance alone is never sufficient.

**Handling of deviations.** Jobs failing for operational reasons are reported and analysed as failures for the arm concerned (no exclusion after the fact). Any departure from this plan is listed in the report as a deviation with its reason.

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
| F8 | **Any** known critical dependency omission whose dependent defect escapes detection and whose job reaches a releasable state (category K-critical, §9a). Not averaged with other results. Additionally reported, but not by itself falsifying: A3 finds no more secondary defects than A1 |
| F9 | Recall gains are accompanied by a human review burden above the 10 % margin or false positives above the 2-point margin (the system "succeeds" by generating excessive review work) |

Consequences are decided now: if F1–F5 or F7 hold, adaptive investigation is switched off by default and offered at most as an experimental option; mechanisms failing their ablation are removed; the dependency obligations, audits, coverage ledger, lanes and release closure — which do not depend on this benchmark — remain. AMOEBA remains the governing architecture (ADR-009A) in either case, with its adaptive component honestly described as unproven.

## 11. What this protocol cannot show

Field performance on drawing types outside the corpus; physical conformity of fabricated parts (benchmark tier P); effects on engineers' vigilance over months of use; benefit in other fabrication domains. Each needs its own study.

## 12. Dependencies before the protocol can run

A minimal system implementing the framework and all arms; cost functions; probe catalogue; synthetic project generator with defect seeding and secondary-defect ground truth; authorised drawings (`FR-10-1…9`), including documented historical defects (new requirement **FR-10-10**: records of past drawing or development errors and how they were found); severity classification by fabrication engineers; adjudicators.

## 13. Change log

| Version | Date | Change | Reason |
|---|---|---|---|
| 1.0 | 18 Nov 2026 | Initial structure | — |
| 1.1 | 18 Nov 2026 | Parameters recorded as provisional preregistered values with justification; severities reported separately; added major/minor recall, dependency-propagation effectiveness, dependency-edge failures, spurious obligations, human review burden and computational cost as explicit metrics; conclusion rule (significance alone insufficient); falsification criteria F8, F9 | Owner decision: the benchmark must not let AMOEBA appear successful while generating excessive review work or missing downstream consequences. Made before any system, data or result exists |
| 1.2 | 18 Nov 2026 | Remaining parameters set (primary budget 2×, power ≥ 0.80, non-inferiority margins 2 points and 10 %); zero tolerance for known critical dependency omissions reaching release; statistical analysis plan with denominators, intervals, multiplicity, power and conclusion rule; three separately reported dependency-edge failure categories; F8 and F9 made exact | Owner decision. Made before any system, data or result exists |
| 1.3 | 18 Nov 2026 | Efficiency criterion accepted as a secondary success criterion with five conditions; recall non-inferiority margin for it recorded as open; natural-defect share deliberately left unset until factory data is seen; natural and injected defects defined as answering different questions and never pooled | Owner decision. Made before any system, data or result exists |
