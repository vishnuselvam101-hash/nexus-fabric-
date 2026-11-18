# AMOEBA Architecture Benchmark

Version 0.1 · 18 November 2026 · Companion to the [AMOEBA architecture](../specifications/AMOEBA_NATIVE_SYSTEM_ARCHITECTURE.md); extends the [Engineering Benchmark Plan](ENGINEERING_BENCHMARK_PLAN.md) and the [Drawing Intelligence Test Plan](DRAWING_INTELLIGENCE_TEST_PLAN.md)

No system exists to measure and no data has been collected. This plan defines what must be shown before the proposed ADRs can be accepted, and which claims are hypotheses. Thresholds are set in Specification 10.

## B-1 Safety invariants (must hold; any violation is a defect)

| Test | Method | Pass condition |
|---|---|---|
| Silent change | Diff every class-H value in outputs against source truth across the whole corpus, with adversarial probes and memory enabled | Zero changes without an origin-7 record |
| Evidence preservation | Run reclamation, termination, quarantine, revision and crash scenarios; compare Snapshot and Records before/after | Nothing lost; reclaimed artefacts regenerate within the declared reproducibility level |
| Approved-design immutability | Supersede an upstream record of an approved part | State returns to REVIEW REQUIRED; old approval remains bound to the old EOS version |
| Confidence isolation | Inject extreme confidence values into recogniser outputs | No change in any reading state, threshold verdict, gate result or mandatory work performed |
| Lane isolation | Attempt release with evidence from experimental/candidate processes; attempt automatic promotion | Blocked; promotion impossible without signed qualification record |
| Zone permissions | Fuzz process outputs with record types outside their permission | Rejected at the schema gate; quarantine raised |
| Coverage honesty | Corpus with planted unrecognisable text, unsupported symbols, masked content, extra ink, missing sheets | Never a full-coverage statement; planted items appear in the correct state |
| Gate integrity | Search for any process sequence reaching APPROVED without G10 and G16 | None (also by model checking of the state machine — research item) |

## B-2 Determinism and recoverability

Replay of identical Snapshot + configuration ⇒ identical EOS version (same build/platform), across thread counts and worker counts · kill any process at random points ⇒ consistent EOS, no duplicate or partial records, work resumes · scheduling order reproducible from EOS version · results independent of parallel completion order (set-union property).

## B-3 Bounded exploration

Budgets never exceeded · mandatory baseline completes regardless of priority signals (starvation test with adversarial signal floods) · feedback-loop termination (fixed point or budget) · every probe present in the ProcessLedger with trigger and outcome · cost overhead of exploration versus baseline.

## B-4 Does adaptivity help? (hypotheses)

Compared at **equal compute budget** against a fixed pipeline (Spec 03 stage order, no signal-driven probes) on the same corpus:

| Hypothesis | Metric |
|---|---|
| H1 Signal-directed probes reduce numeric false accepts | Numeric false-accept rate; alternative coverage |
| H2 Conflict-directed investigation raises the share of conflicts resolved to a correct explanation before human review | Correctly explained conflicts / all seeded conflicts |
| H3 Priority-ordered review reduces engineer time to an approvable observation set | Time, corrections |
| H4 Tenant memory (templates, exclusions) reduces review workload without increasing false accepts | Workload vs safety metrics, with and without memory |
| H5 Exploration does not degrade determinism or safety | B-1, B-2 with exploration on |

If H1–H4 are not supported, ADR-009 should be reduced to the non-adaptive parts (EOS, coverage, lanes, quarantine), which do not depend on them.

## B-5 Coverage model

Stability of denominators under re-runs · enumeration cost versus page size · agreement between coverage states and annotator ground truth on real drawings (does "fully examined" correspond to regions the annotators consider explained?) · blind-spot study: content annotators see that enumeration misses (faint ink, content merged into large components) · reviewer workload from exclusion decisions.

## B-6 Independence of evidence (quorum)

For each pair of recognisers and each pair of primary/verifier methods: individual error rates, joint error rate, conditional error given agreement, by input class and glyph class; with shared and with separate preprocessing. Output: the independence credit each pairing may receive in threshold rules — zero until measured.

## B-7 Scale

EOS size, view extraction and impact-analysis latency for large packages (hundreds of sheets); signal log growth; regeneration time after a single-dimension change.

## B-8 Memory and bias

Does remembered knowledge shift readings toward previously seen values (seeded cases where the new drawing differs from the remembered pattern)? Cross-tenant isolation tests. Behaviour when a memory item is retired.

## Data needs

Tier S synthetic corpus (can start once a generator exists) for B-1…B-3, B-5 (partly), B-6 (partly), B-7. Authorised factory drawings for B-4, B-5, B-6, B-8. Reviewer participation for H3/H4.
