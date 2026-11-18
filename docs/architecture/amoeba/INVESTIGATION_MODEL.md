# Investigation Model

Hypothesis-driven investigation · self-consistency auditing · bounded adaptation · stopping rules

Version 0.1 (refinement B) · 18 November 2026 · Proposed design under ADR-012 (provisional) · Responds to review findings W3, W11 and essential requirements 2–5

**Status.** Proposed [P]. The *record-keeping* parts (investigation records, audits, assumption ledger, stopping statements) are engineering reliability mechanisms that apply whether or not adaptive selection is ever shown to help. The *selection* parts (choosing which test to run next by expected benefit; switching strategy) are provisional and governed by the benchmark (ADR-009B).

---

## 1. Investigation record

An investigation is opened for every confirmed-suspicious situation: a conflict or discrepancy Finding, an open obligation, a verification inconclusive/disagreement, an audit discrepancy, an anomaly flag.

```
Investigation {
  id, opened_by (finding | obligation | audit), subject: [record ids], scope (region / part / interface)
  hypotheses: [ Hypothesis ]            # always includes UNKNOWN_CAUSE
  tests:      [ Test ]
  search:     { catalogue_version, classes_enumerated: [..], complete: bool }
  remaining_uncertainty: { open_hypotheses, sensitivity: effect on released geometry vs tolerance,
                           statement }
  decision:   { outcome: explained(h) | unexplained | escalated | accepted_risk,
                basis: decisive_rule(id) | human(decision id), provenance }
  cost:       { planned_work_units, used_work_units }
}

Hypothesis {
  id, class (from the explanation catalogue), parameters (e.g. alternative reading "3"→"8"),
  supporting: [evidence ids], contradicting: [evidence ids],
  status: open | supported | refuted | undetermined
}

Test {
  id, probe_type, version, discriminates: [hypothesis ids],
  expected: { per hypothesis: predicted observable outcome },      # written before execution
  cost_estimate (work units), selected_by: rule(id) | strategy(id) | human,
  actual: { outcome, evidence ids, cost },                          # written after execution
  verdict_per_hypothesis: supports | contradicts | uninformative
}
```

Rules:

- **INV-01** Hypotheses are instantiated from a versioned, human-authored **explanation catalogue** per finding kind (e.g. for *written vs measured*: misread digit; wrong association; view not to scale; radius/diameter confusion; inside/outside; scan anisotropy; genuine drawing error). A model may *propose* an additional hypothesis; it enters as a candidate and must be accepted by a person to be tested.
- **INV-02** `UNKNOWN_CAUSE` is always present and can never be refuted by a test. Therefore an investigation can be closed as `explained` **by rule** only where the catalogue marks a test as *decisive* for that hypothesis (the evidence entails it, e.g. an exact chain-closure identity). In all other cases closure requires a human decision, with the investigation record as the dossier.
- **INV-03** Expected outcomes are recorded **before** a test runs. Comparing expected with actual outcomes over many investigations measures whether the system's judgement carries information (benchmark metric); a selection rule whose predictions do not correlate with outcomes is withdrawn.
- **INV-04** An explanation never changes an authoritative value. `explained(misread)` produces a proposed correction for an engineer; `explained(drawing error)` produces a question for the customer.
- **INV-05** "Evidence contradicts" means positive evidence against; absence of support is `undetermined`, not `refuted`.
- **INV-06** If `search.complete = false` (not all catalogue classes could be enumerated or tested within budget), any single surviving hypothesis has status `supported`, never `established`; downstream it is treated as ambiguous (Safety refinement S-4).

### Test selection

Each probe type declares which hypothesis classes it can discriminate and a deterministic cost function in work units. Two selection modes:

| Mode | Rule | Status |
|---|---|---|
| **Catalogue order** | Tests run in the fixed order given by the catalogue for that finding kind | Baseline; always available |
| **Expected risk reduction per cost** | Choose the test maximising (discrimination rate from a measured table × risk weight of the subject) ÷ cost. Risk weight = sensitivity of release-critical outputs to the subject relative to tolerance margin (Spec 02 ALG-11), or a fixed class weight where sensitivity is not yet computable | **Provisional**; tables come from the benchmark development set; zero-knowledge default is catalogue order |

Discrimination tables are production-lane data fixed for the duration of a job; nothing is learned within a job. Arithmetic is exact (rationals), ties broken by hash, so selection is reproducible.

## 2. Self-consistency auditing (mandatory, not optional)

A general reliability mechanism independent of the adaptive question.

| Audit | What | When | On discrepancy |
|---|---|---|---|
| **A1 Recompute-from-scratch** | Every derived conclusion (reading-state transitions by rule, hypothesis verdicts, coverage states, obligation sets, gate results, release closure) is recomputed from primary records by an auditor implementation that shares no incremental state with the runtime | All release-critical conclusions at gate evaluation; sampled (by hash of id) otherwise | Finding `audit_discrepancy` → quarantine of the affected conclusion → investigation |
| **A2 Invariance tests** | Re-run analysis under transformations that must not change conclusions: page rotation, alternative render resolution within the valid band, permuted document order, permuted round-internal order, rigid motion / unit change of the part (Spec 02 metamorphic relations) | Per job on a hash-selected sample; fully for release-critical calculations where the cost is declared acceptable | Finding → investigation; order-dependence is a runtime defect |
| **A3 Verifier cross-examination** | When primary and verifier disagree, both are suspects: a third formulation or a higher-precision evaluation is used where available; the verifier's own seeded-fault record is consulted | On every verification disagreement | Quarantine whichever is refuted; if undetermined, both |
| **A4 Assumption ledger** | Every conclusion resting on: an incomplete search, an uncalibrated score, an *assumed* process parameter, an unmeasured independence credit, a budget-stopped process, an exclusion decision | Continuous | Entries in a release closure must each be cleared by rule or Decision; all are printed in the residual-uncertainty statement |
| **A5 Runtime replay** | An independent instance replays the job from the Snapshot and human inputs and must reach the same primary state ([Determinism](DETERMINISM_AND_APPROVAL.md)) | Before release (policy: always, or sampled — Spec 05) | Release blocked |

- **INV-10** Audit processes run in the verification zone (Z3) and may not share implementation with what they audit beyond audited primitives (ARCH-03).
- **INV-11** A1 and A5 on the release closure are mandatory for every release (proposed gate **G19 `self_consistency`**; proposed amendment to Spec 02 §10 — not applied).

## 3. Bounded adaptation

AMOEBA may change how it investigates; it may not change what it is.

| May adapt (recorded) | May never adapt |
|---|---|
| Which discretionary tests run, and in what order | Any qualified algorithm's computation (AMB-21) |
| Allocation of discretionary budget between probe families, parts and regions | Mandatory work set; gates; thresholds; rule tables |
| Depth of discretionary common-cause propagation | Authority, reading or validation status of any record |
| Creation of specialised analysis branches from the catalogue (e.g. kernel cross-check, Monte Carlo uncertainty, higher precision, alternative formulation) | The catalogue, tables or strategies themselves during a job |
| Review-queue ordering | What requires human confirmation |

**Strategy profiles.** A finite, versioned set of profiles (parameter sets for the items in the left column), each qualified through the lanes. Switching between profiles happens only at round boundaries, by deterministic rules over recorded in-job statistics (probe yield by type, conflict density, share of unexplained content, verification inconclusive rate).

```
StrategyChange { id, round, from_profile, to_profile, rule_id, triggering_statistics, remaining_budget }
```

- **INV-20** Profiles and switching rules are production-lane data authored by people; none is generated at run time. Number of switches per job is capped.
- **INV-21** A switch reallocates discretionary budget only.
- **INV-22** Every specialised branch is an Activity with a declared purpose linked to an Investigation or Obligation; a branch with neither is not schedulable.

## 4. Stopping rules and residual uncertainty

Three layers, evaluated in this order:

1. **Mandatory work — never stopped by yield or budget shortage.** Baseline examination of every enumerated element; all mandatory obligations; verification required by the family; audits A1/A5 on the release closure; gate evaluation. If the job's budget cannot cover mandatory work, the job is *incomplete* and cannot be released; it is not truncated.
2. **Discretionary investigation — stops when** any of: (a) fixed point (no schedulable work); (b) discretionary budget exhausted; (c) marginal yield (new confirmed Findings, or hypotheses resolved, per work unit) below the configured threshold for a configured number of consecutive rounds. Thresholds are configuration with provenance, derived from the benchmark; none is assumed here.
3. **At stop — a Residual Uncertainty Statement** (a record) is produced for each part and for the project:

| Item listed | Disposition (exactly one) |
|---|---|
| Open Findings | blocks release · requires human review |
| Open obligations (mandatory) | blocks release |
| Open obligations (discretionary) | requires human review · acceptable under rule *R* (cited) · accepted by Decision |
| Investigations closed `unexplained` or with incomplete search | requires human review · accepted by Decision |
| Assumption-ledger entries in the release closure | cleared by rule (cited) · accepted by Decision |
| Coverage vector and enumeration limits | reported (see Coverage refinement) |
| Stop reason and unused budget | reported |

- **INV-30** "Acceptable" is never a judgement made by the runtime. It is either a cited pre-defined acceptance rule (e.g. *the element lies outside every release closure and outside the declared scope*) or an engineer's Decision with reason.
- **INV-31** A low-yield stop is recorded as such and is **never** presented as evidence that nothing remains; an estimate of undetected defects (e.g. by capture–recapture between two detection paths) may be shown as *advisory* with its assumptions — it is an unverified hypothesis (H-STOP) and is not an input to any gate.

## 5. Open questions

Explanation catalogues and decisive-test definitions per finding kind (engineering authorship needed); discrimination tables (need benchmark data); yield thresholds; cost functions per probe type; audit sampling rates and their cost; whether A5 replay before every release is affordable on single-node deployments; human-factors design of the investigation dossier so that reviewers engage with alternatives instead of accepting the top hypothesis.
