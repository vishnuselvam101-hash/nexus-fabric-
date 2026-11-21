# Determinism and Approval Semantics

Version 0.1 (refinement C) · 18 November 2026 · Proposed design under ADR-010 and ADR-012 conditions · Responds to review findings W4, W5

**Status.** Proposed [P]. The round model is an application of an established pattern (bulk-synchronous / superstep execution) [E]; that it achieves replay at acceptable cost in this system is to be demonstrated (benchmark safety prerequisite S2).

## 1. Round model (deterministic replay under parallel execution)

```
state v_k ──schedule──► work set W_k ──execute in parallel──► outputs ──canonical commit──► state v_{k+1}
   ▲            (pure function of v_k)   (each activity reads v_k only)    (sorted, validated)        │
   └──────────────────── human inputs merged only at round boundaries ◄──────────────────────────────┘
```

- **DET-01** The scheduler is a pure function `W_k = schedule(v_k, configuration)`; it reads primary data only (§2). No timing, host, queue or completion-order information can influence it.
- **DET-02** Every activity in a round reads the frozen state `v_k` and nothing else. Activities in the same round cannot see each other's outputs.
- **DET-03** Each activity is itself deterministic: pinned versions and weights, fixed seeds derived from input hashes, no wall-clock or environment dependence in outputs (Spec 02 §7.5 defines the reproducibility level of numeric results).
- **DET-04** Commit: all outputs of the round are schema-validated, then appended in a canonical order (sorted by activity type, then input hash). Record identifiers are content hashes, so the order affects nothing but the log position. If two activities produce the identical record, it is stored once with both producers.
- **DET-05** Signals are derived at commit from the committed records, in canonical order. There is no other event source.
- **DET-06** Human inputs (confirmations, corrections, decisions, uploads) are external inputs, like the Snapshot. They are queued and merged at the next round boundary with a recorded sequence number. Replay consumes the same sequence.

Consequence: for a given Snapshot, configuration and human-input sequence, the sequence of states `v_0, v_1, …` is independent of worker count, thread scheduling and completion order.

**Stated limits.** (a) Replay equality is at the reproducibility level of the activities: bitwise on the same build/platform; across platforms only what Spec 02 §7.5 establishes. (b) Resource aborts (§3) are the one place where two machines can legitimately diverge; they are recorded and visible. (c) Rounds cost latency compared with free-running asynchronous execution; the cost is a benchmark item.

## 2. Primary data versus operational data (state identity)

| | Contents | In `eos_version`? |
|---|---|---|
| **Primary** | Snapshot; all records (content); Findings and their transitions (immutable records); Activity records: type, version, lane, inputs, outputs, **logical** work units, logical outcome; human-input records including their own timestamps as data; configuration hash | Yes |
| **Operational** | Wall-clock start/end, durations, host and worker ids, retry counts, queue metrics, memory peaks | No — kept in the audit log, linked to activity ids |

`eos_version = H(primary data)`. Wall-clock time is evidence about operations, not part of engineering state. (Corrects the earlier definition, which hashed budget usage measured in time.)

Findings (Issues) are immutable: a state change is a new transition record referencing its predecessor. (Corrects the earlier mutable `state/history` fields.)

## 3. Logical work budgets

- **DET-10** Each activity type declares a deterministic cost function in **work units** computed from its inputs (pixels, glyph count, entity count, sample count, constraint count). Budgets are granted, consumed and reported in work units.
- **DET-11** Budget checks happen in the scheduler, before execution, from declared costs — never by interrupting on elapsed time.
- **DET-12** Wall-clock and memory limits exist only as **safety aborts**. An abort is an operational event; the activity is retried under a bounded policy. If it still cannot complete, a primary outcome `incomplete(resource)` is committed. This is the only way physical resources enter primary state, and it always reduces what can be claimed (coverage, release), never increases it.
- Declared costs will be wrong at first; calibration of cost functions against measured time is a benchmark task. Mis-calibration affects fairness and throughput, not reproducibility.

## 4. Release closure and approval

### 4.1 Definition

For a part (or package) `P` to be released, the **release closure** `C(P)` is the smallest set of records containing P's release outputs and closed under:

1. Derivation and Manufacturing edges (everything the outputs were computed from: profiles, pieces, patterns, surfaces, parameters, constraints, dimensions with their validation records, materials, thickness, ProcessData, assumptions, the Activities and AlgorithmVersions involved);
2. Assurance edges (evidence, audit results, coverage statement for P's scope, decisions, the Residual Uncertainty Statement);
3. Interface records of P and, for each mating part, **only its interface-defining records** (the features and parameters named in the interface constraints) — not the neighbour's whole closure;
4. Sheet- and project-level declarations that P's records reference (units, surface-reference convention, projection method, general tolerance, revision identity);
5. The gate-evaluation record `E(P)` computed over items 1–4.

`release_closure_hash(P) = H(sorted record ids of C(P))`.

### 4.2 Approval

1. `E(P)` is produced by the gate-evaluation activity over the closure *without* any approval (gates G01–G15 and proposed G17–G19; not G16).
2. The engineer is shown the closure's content and `E(P)` and signs **Approval { release_closure_hash, E(P) id, approver, time }**.
3. Gate G16 is then defined as: *a valid Approval exists whose hash equals the current `release_closure_hash(P)`*.

This removes the circularity between "approval requires all gates" and "G16 is a gate", and makes Spec 02's wording ("over the content hash of exactly the evaluated records") precise.

### 4.3 Validity and invalidation

An Approval is **valid** at state `v` iff all hold:

- every record in `C(P)` is still `current` (none superseded, quarantined or retired-with-issue);
- recomputing `C(P)` at `v` by the closure rule yields the same set (nothing that *would belong* has appeared);
- no open Finding and no open mandatory Obligation has a subject in `C(P)`.

| Event | Effect on approval of P |
|---|---|
| New file, part or observation unrelated to P | None |
| Activity in another part; discretionary exploration elsewhere | None |
| Correction/supersession of a dimension, parameter, material, ProcessData, assumption in `C(P)` | Invalid → REVIEW REQUIRED |
| Change of a mating part's **interface-defining** records | Invalid |
| Change inside a mating part that leaves its interface records unchanged | None |
| Change of a referenced declaration (units, convention, revision) | Invalid |
| New Finding or mandatory Obligation targeting a record in `C(P)` | Invalid |
| Quarantine or retirement-with-issue of an AlgorithmVersion in `C(P)` | Invalid |
| New drawing revision touching P's sources | Invalid; new revision required |

- **DET-20** An invalidated approval is never restored. A new approval requires a new gate evaluation and signature over the new closure hash; the old Approval remains as history bound to its old hash.
- **DET-21** Validity is computed, not stored, and is itself audited by A1 (recompute-from-scratch).

## 5. Open questions

Round latency versus throughput on a single node; granularity of rounds for very large jobs (partitioned rounds per independent sub-project?); cost-function calibration; closure size and hashing cost for large assemblies; exact list of "interface-defining records" per joint type (with the dependency rule table); cross-platform replay level; formal model of rounds, lanes and the release state machine for model checking (research AR-05).
