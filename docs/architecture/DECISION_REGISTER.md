# Decision Register

Version 1.2 · 18 November 2026 · The single place where the status of every architectural decision, proposal and open question is kept visible. Specifications 01–03 are **not** edited to reflect these decisions until amendments are formally applied as new versions with change logs.

Status vocabulary: **Accepted** · **Accepted in principle** (identity decided, detailed design under review) · **Accepted with conditions** · **Provisional** (awaiting evidence) · **Deferred** · **Proposed** (no decision yet) · **Withdrawn/Merged**.

## 1. Owner decisions (18 November 2026)

| ID | Subject | Status | Record |
|---|---|---|---|
| D-01 | AMOEBA is the governing architecture of the whole platform; not conditional on benchmarks | Accepted in principle | [ADR-009A](ADR-009A-amoeba-governing-architecture.md) |
| D-02 | Claim that adaptive investigation outperforms conventional pipelines | Provisional — experimental demonstration required | [ADR-009B](ADR-009B-adaptive-investigation-effectiveness.md) |
| D-03 | Engineering Organism State | Accepted with conditions | [ADR-010](ADR-010-engineering-organism-state.md) |
| D-04 | Coverage ledger | Accepted with conditions | [ADR-011](ADR-011-coverage-ledger.md) |
| D-05 | Coverage release gate (G17) | Deferred | ADR-011 |
| D-06 | Adaptive exploration | Provisional — benchmark required | [ADR-012](ADR-012-adaptive-exploration.md) |
| D-07 | Signals as a separate decision | Merged into ADR-012 | [ADR-014](ADR-014-signals.md) |
| D-08 | Lanes, quarantine, retirement | Accepted with conditions | [ADR-013](ADR-013-lanes-quarantine-retirement.md) |
| D-09 | Independent-evidence principle | Accepted | [ADR-015](ADR-015-independent-evidence.md) |
| D-10 | Numerical independence thresholds | Deferred | ADR-015 |
| D-11 | Tenant knowledge ("memory") | Deferred pending legal and security review | [ADR-016](ADR-016-tenant-knowledge.md) |
| D-12 | Approval binds to a release-closure hash covering exactly the approved records and their dependencies; unrelated additions do not invalidate, relevant dependency changes do | Accepted (requirement); design proposed | [Determinism and Approval](amoeba/DETERMINISM_AND_APPROVAL.md) §4 |
| D-13 | Essential requirement: dependency-following investigation | Accepted (requirement); design proposed | [Dependency Model](amoeba/DEPENDENCY_MODEL.md) |
| D-14 | Essential requirement: hypothesis-driven investigation with recorded expectations and outcomes | Accepted (requirement); design proposed | [Investigation Model](amoeba/INVESTIGATION_MODEL.md) §1 |
| D-15 | Essential requirement: self-consistency auditing as a general reliability mechanism | Accepted (requirement); design proposed | Investigation Model §2 |
| D-16 | Essential requirement: bounded adaptation through pre-qualified strategies and recorded decisions only | Accepted (requirement); design proposed | Investigation Model §3 |
| D-17 | Essential requirement: engineering stopping rules; low yield never overrides mandatory checks | Accepted (requirement); design proposed | Investigation Model §4 |
| D-18 | AMOEBA is a proper name, not an acronym | Accepted | — |
| D-19 | Four-class development scheme (Spec 02 §14) | Remains proposed; not adopted in the master plan pending engineering review | Spec 02 §14 |
| D-20 | Specification 02 | Provisional until independently reviewed | Roadmap |
| D-21 | Specification 03 | Provisional; approved for further architecture work, not for implementation | Roadmap |
| D-22 | No platform implementation before Specs 04–06 are defined; first milestone is one narrow vertical slice | Accepted | Roadmap |

### Second round (18 November 2026)

| ID | Subject | Status | Record |
|---|---|---|---|
| D-23 | Benchmark minimum practically meaningful improvement: +5 percentage points critical-defect recall | Accepted as a **provisional preregistered** parameter; revisable before execution only | [Protocol v1.1](../benchmark/AMOEBA_BENCHMARK_PROTOCOL.md) §9 |
| D-24 | Budget levels 1.25×, 1.5×, 2×, 4× mandatory-work cost | Accepted (provisional preregistered) | Protocol §9 |
| D-25 | Significance 0.05 with multiple-comparison correction; power analysis and confidence intervals required; significance alone insufficient; no superiority claim from synthetic data alone | Accepted | Protocol §7, §9 |
| D-26 | Benchmark reports critical, major and minor defects separately, plus false positives/negatives, human review burden, computational cost, dependency-propagation effectiveness, dependency-edge failures, reproducibility, residual uncertainty | Accepted | Protocol §5 |
| D-27 | Distinct EXCLUDED coverage state with mandatory exclusion record | Accepted | [Coverage refinement](amoeba/COVERAGE_AND_SAFETY_REFINEMENT.md) D2 |
| D-28 | Release Gate A — obligation closure (G18) | **Adopted, mandatory** | [Release gates addendum](amoeba/RELEASE_GATES_ADDENDUM.md) |
| D-29 | Release Gate B — self-consistency (G19) | **Adopted, mandatory** | Release gates addendum |
| D-30 | Deterministic replay audit on an independent execution instance before **every** manufacturing release; does not replace independent engineering verification | Accepted | Release gates addendum |
| D-31 | Targeted fabrication-engineering review of the dependency model and interface rules is required before Specification 04 becomes an implementation baseline; paper examples must not be represented as validated cases | Accepted | [Review package](../reviews/FABRICATION_ENGINEERING_REVIEW_PACKAGE.md) |
| D-32 | Next milestone is validated engineering knowledge, not further architecture documents | Accepted | Roadmap |

### Third round (18 November 2026)

| ID | Subject | Status | Record |
|---|---|---|---|
| D-33 | Primary budget 2× mandatory-work cost; additional levels 1.25×, 1.5×, 4× | Accepted (provisional preregistered) | [Protocol v1.2](../benchmark/AMOEBA_BENCHMARK_PROTOCOL.md) §9 |
| D-34 | Power ≥ 0.80; significance 0.05 with correction | Accepted (provisional preregistered) | Protocol §9, §9a |
| D-35 | Non-inferiority margins: false positives 2 percentage points; human review burden 10 % relative | Accepted (provisional preregistered) | Protocol §9, §9a |
| D-36 | Zero tolerance for known critical dependency omissions that escape detection and reach release; three dependency-failure categories reported separately | Accepted | Protocol §9a, F8 |
| D-37 | Reviewer requirements for the fabrication-engineering review; review conducted on the original 56 statements; reviewer independent of the author of the rules where possible | Accepted | [Review package](../reviews/FABRICATION_ENGINEERING_REVIEW_PACKAGE.md) §1 |
| D-38 | A Japanese-language version must be translated and independently checked by a Japanese-speaking fabrication engineer before use at the factory | Accepted — **not yet produced** | Review package §0 |
| D-39 | Five evidence levels, kept as separate kinds of evidence, not interchangeable approval labels | Accepted | [Evidence levels](EVIDENCE_LEVELS.md) |
| D-40 | Specification 04 is not to be started; no further expansion of the architecture while its engineering assumptions are untested | Accepted | Roadmap |

Earlier ADR-001…008 remain **Proposed**; no owner decision has been recorded on them.

## 2. Designs produced in this refinement (all Proposed — awaiting review)

| ID | Design | Document | Addresses |
|---|---|---|---|
| P-01 | Typed dependency edges, Interface records, three-direction propagation, scrutiny obligations, rule table | [DEPENDENCY_MODEL.md](amoeba/DEPENDENCY_MODEL.md) | Review W1, W2; D-13 |
| P-02 | Investigation record; explanation catalogue; `UNKNOWN_CAUSE`; decisive-test rule; expected-vs-actual recording | [INVESTIGATION_MODEL.md](amoeba/INVESTIGATION_MODEL.md) §1 | W3; D-14 |
| P-03 | Audits A1–A5 and assumption ledger | Investigation Model §2 | D-15 |
| P-04 | Strategy profiles and `StrategyChange` records | Investigation Model §3 | D-16 |
| P-05 | Three-layer stopping and Residual Uncertainty Statement | Investigation Model §4 | D-17 |
| P-06 | Round model; primary vs operational data; logical work units | [DETERMINISM_AND_APPROVAL.md](amoeba/DETERMINISM_AND_APPROVAL.md) §1–3 | W4 |
| P-07 | Release closure, two-step approval, invalidation table | same §4 | W5; D-12 |
| P-08 | Coverage: three kinds of statement; raster levels R0–R3; attribution masks; single definition of "explained" | [COVERAGE_AND_SAFETY_REFINEMENT.md](amoeba/COVERAGE_AND_SAFETY_REFINEMENT.md) Part D | W7 |
| P-09 | Lane overlays and taint rule | same S-1 | W8 |
| P-10 | Runtime decomposition and zone ZR | same S-2 | W9 |
| P-11 | Single confidence rule (discretionary priority only, monotone) | same S-3 | W6 |
| P-12 | `provisional_sole` for incomplete searches | same S-4 | W11 |
| P-13 | What may be machine-proposed; promotion rules | same S-5 | AMB-S5 |
| P-14 | Seven record families; five axes; derived predicates | [CONSOLIDATED_RECORD_MODEL.md](amoeba/CONSOLIDATED_RECORD_MODEL.md) | Review §10; ADR-010 condition 4 |
| P-15 | Engineering-side process catalogue and feedback paths | [ENGINEERING_PROCESS_CATALOGUE.md](amoeba/ENGINEERING_PROCESS_CATALOGUE.md) | W1 |
| P-16 | Benchmark protocol v1.0 (structure frozen; parameters awaiting sign-off) | [AMOEBA_BENCHMARK_PROTOCOL.md](../benchmark/AMOEBA_BENCHMARK_PROTOCOL.md) | W10; D-02 |

## 3. Proposed amendments to existing documents (none applied)

| Target | Amendments | Source |
|---|---|---|
| Spec 01 | A-01…A-07 | [Migration note](amoeba/EXISTING_ARCHITECTURE_MIGRATION.md) §2 |
| Spec 02 | A-02-1…3; new gates G18 `obligations_discharged`, G19 `self_consistency`; G16 redefined via release closure; MATH-12 with `provisional_sole` | Migration note §4; this refinement |
| Spec 03 | A-03-1…6; §5.5 `provisional_sole`; ObservationSet as signed view | Migration note §3; this refinement |
| AMOEBA architecture v0.1 | Withdraw "fully inspected" wording; EOS identity; immutable Findings; confidence rule; zone ZR; overlays | This refinement |
| Original Coverage Model | Raster levels; "explained" definition; no INCONCLUSIVE from enumeration level | Coverage refinement |
| Architecture Benchmark | B-4 superseded by Protocol v1.0; confidence-isolation test restated | Protocol |
| Review amendments (19) | Listed in the review §12; most are covered by P-01…P-16 — a one-to-one reconciliation table is still to be made | [Review](../reviews/AMOEBA_INDEPENDENT_ARCHITECTURE_REVIEW.md) |
| Master plan | Describe AMOEBA as governing architecture in §4 | Migration note §6 |

## 4. Validation boundary (must remain stated until evidence changes it)

| Statement | Status on 18 November 2026 |
|---|---|
| Dependency statements independently reviewed | **0 of 56** |
| Factory dependency rules validated | **None** |
| Spec 02 mathematical models independently reviewed | **None** |
| Process allowances calibrated | **None** |
| Factory drawings or factory data received | **None** |
| Standards identified and verified | **None** |
| Japanese review package and vocabulary checked by a native-speaking engineer | **No** |
| Evidence level of every engineering relationship in the repository | **EV-1 (AI-generated proposal)** |
| Specification 04 | **Not started; not implementation-ready** |

This table is updated only when a named reviewer's returned evidence, a factory record or an experimental result is filed in the repository.

## 4a. Blockers for Specification 04

| # | Blocker | Who can remove it |
|---|---|---|
| 1 | Fabrication-engineering review of the 56 statements (at least the 18 header-and-branch statements and the 10 rules) returned with reviewer identity and reasoning | Qualified fabrication engineer |
| 2 | Answers to the five factory checklist questions (component types, formats, historical jobs, projection convention, inside/outside dimensioning) | Factory |
| 3 | Independent engineering review of the Spec 02 models for reducers, branches and transitions | Mechanical/manufacturing engineer |
| 4 | Decision on development-class terminology (Spec 02 §14) | Owner, after 3 |
| 5 | Incorporation of review results into the dependency model and rule table, with attribution and evidence level | Project team, after 1 |
| 6 | Authorised representative drawings (at least for the first target geometry) | Factory / owner |

Not blockers for drafting, but blockers for *implementation*: process-parameter calibration; licensing evaluation records; standards review; benchmark corpus.

## 4b. Decisions still needed from the owner

Who performs the fabrication-engineering review and by when · who translates and who independently checks the Japanese version · efficiency-alternative parameter and natural-defect share in the protocol · when to apply accepted amendments to Specs 01–03 · ADR-001…008 status.

## 5. Unresolved engineering questions

**Dependency model**
1. Which interface constraints exist per joint type in the factory's work, and which checks each kind of defect should trigger (rule-table authorship by fabrication engineers).
2. How interfaces are known when only detail drawings are supplied.
3. Tolerance stack-up across chains of interfaces.
4. Handling of algorithm faults that reach already-released jobs.
5. Obligation volume on large assemblies.

**Investigation**
6. Explanation catalogues and decisive tests per finding kind.
7. Discrimination tables, yield thresholds, cost functions — all require data.
8. Presentation of investigation dossiers so that engineers weigh alternatives.

**Determinism and approval**
9. Round latency on a single on-premises node; partitioned rounds.
10. Exact "interface-defining records" per joint type.
11. Cross-platform replay level with an external kernel in the loop.
12. Formal model of the round, lane and release state machines.

**Coverage**
13. Ink allowance ε per scan-quality class; stability and cost of skeleton-segment enumeration.
14. Reviewer workload for unexplained and excluded content.

**Mathematics and fabrication (inherited, still open)**
15. Independent review of Spec 02 family models against actual fabrication geometry.
16. Development classification terminology and boundaries.
17. Neutral-surface position on oblique cones and transitions; calibration of all process parameters.
18. Standards: none identified or verified (drawing practice, tolerances, welding symbols, pipe sizes, uncertainty guides).

**Factory information (nothing received yet)**
19. Most frequent component types; incoming formats; availability of historical jobs; projection convention; inside/outside/centre dimensioning — the five checklist questions.
20. `FR-02-1…10`, `FR-03-1…12`, `FR-10-1…10`, including documented historical defects for the benchmark.

**Licensing and legal**
21. Every dependency is *unevaluated* until a Dependency Evaluation Record exists; kernel, CGAL, DWG, PDF, model weights.
22. Tenant-knowledge permissions; retention versus append-only state; name and prior-art search for "AMOEBA".
