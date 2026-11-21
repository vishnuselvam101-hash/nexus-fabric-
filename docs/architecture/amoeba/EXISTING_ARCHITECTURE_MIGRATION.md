# Existing Architecture → AMOEBA: Migration and Reconciliation

Version 0.1 · 18 November 2026 · Supporting note to the [AMOEBA architecture](../../specifications/AMOEBA_NATIVE_SYSTEM_ARCHITECTURE.md)

**Rule followed:** Specifications 01–03, the ADRs and the master plan were **not edited** by this task. Everything below is a mapping or a *proposed amendment* awaiting decision. Status of the earlier documents is unchanged: 01 draft, 02 provisional (independent review outstanding; four-class development scheme still only a proposal), 03 provisional.

## 1. Concept mapping

| Existing concept | Where | AMOEBA term | Change |
|---|---|---|---|
| Trust zones Z0–Z4 | Spec 01 §1.2 | Unchanged; govern every process | None |
| Ingestion service | Spec 01 §2, Spec 03 §1.1 | Membrane | Rename |
| — | — | Engulfment + Snapshot | **New** |
| Engineering Truth Graph | Spec 01 §6 | Records component of the EOS | Extended |
| Job orchestration | Spec 01 §2 | AMOEBA runtime (scheduler, supervisor, budgets) | Extended |
| Pipeline stages | Spec 01 §1.3, Spec 03 | Processes in lifecycle phases, signal-driven | Reframed |
| Stage outcomes (unsupported / ambiguous / insufficient) | ARCH-05 | Issues + coverage states | Extended |
| Experimental vs production versions | Spec 01 §7 | Lanes (experimental / candidate / production) | Formalised |
| Failure isolation | Spec 01 §7 | Sandboxed decomposition; quarantine; controlled termination | Extended |
| Incremental recomputation, job recovery | Spec 01 §7 | Regeneration | Rename |
| Geometry cache | Spec 01 §7 | Reclaimable artefacts (autophagy) | Rename + rule |
| Engineering Knowledge Centre (module 13) | Master plan §4 | Cellular memory | Rename + rules |
| UEM | Spec 02 | Pure library called by Build/Verify processes | None |
| Release gates G01–G16 | Spec 02 §10 | Evaluated by a Verify process on an EOS version | + proposed G17 |
| ObservationSet | Spec 03 §10 | Signed view of an EOS version | Reframed |
| Reconciliation rules RC-01…15 | Spec 03 §7 | `reconcile.rules` process; outputs are Issues | None |
| Interpretation hypotheses | Spec 03 §5.5 | Branches (records); elimination = controlled termination with tombstone | None in substance |
| Corroboration rules | Spec 03 §3.6 | Independent-evidence thresholds (quorum) | Tightened (independence must be measured) |

## 2. Specification 01

Coherent. Proposed amendments (not applied):

| ID | Amendment |
|---|---|
| A-01 | §6.1: Observation record carries four orthogonal axes — origin (8), authority (4), reading state, lifecycle status ([EOS note §4](ENGINEERING_ORGANISM_STATE.md)) |
| A-02 | §6: introduce EOS as the top-level state; truth graph becomes its Records component; add Snapshot, RegionTree, Issues, CoverageLedger, ProcessLedger, SignalLog |
| A-03 | §1.3: replace the linear stage list by the lifecycle table; keep the stage list as the "typical path" |
| A-04 | §2: job orchestration → AMOEBA runtime with budgets, deterministic scheduling, supervision |
| A-05 | §7: lanes; quarantine; tombstones; reclaimable-artefact class |
| A-06 | §3.1: twelve UEM subsystems (carried over from Spec 02 §14) |
| A-07 | §1.3 stage 5 development labels — **remains undecided** pending the engineering review of Spec 02; AMOEBA takes no position |

## 3. Specification 03 — component-by-component mapping

| Spec 03 component | § | AMOEBA process | Phase | Mechanism(s) | Coverage domain |
|---|---|---|---|---|---|
| Common ingestion, limits, problem files | 1.1 | `membrane.admit` | 1 | membrane, immune (reject/quarantine) | D1 |
| — whole-package intake | 8.1 | `engulf.snapshot`, `engulf.enumerate` | 2 | engulfment | D1–D4 denominators |
| Format parsers (PDF, raster, DXF, DWG, STEP) | 1.2 | `digest.document.<format>` | 3 | sandboxed decomposition, differentiation | D2, D3 |
| PDF page classification | 1.3 | `digest.page.classify` (uncertain ⇒ dispatch to both pipelines) | 3 | differentiation | D2 |
| Render cross-check (DI-03) | 1.2 | `explore.probe.render_compare` | 4 | probe, immune | D3 |
| Pre-processing and transform chain | 2 | `digest.page.prepare` | 3 | decomposition | D4 |
| Page segmentation, view-region detection | 2.2 | `digest.region.segment` | 3 | decomposition | D4/D5 |
| Text detection, recognition with n-best | 3.2 | `explore.recognise.*` (one process per recogniser) | 4 | probes; alternatives preserved as branches | D5 |
| Normalisation, grammar parsing | 3.2–3.3 | `digest.element.parse` (deterministic) | 3/4 | decomposition | D5 |
| Vocabulary interpretation | 3.4 | `explore.interpret.vocabulary` (candidates only) | 4 | memory | D5 |
| Reading validation states | 3.6 | Independent-evidence threshold + Review | 6/7 | quorum; human | — |
| Feature extraction, role classification | 4 | `digest.element.geometry`, `explore.classify.role` | 3/4 | decomposition; probes | D3/D4 |
| Clean-up operations | 4.3 | `digest.element.cleanup` (derived records) | 3 | decomposition | D3/D4 |
| View typing, projection method, scale | 5.1–5.4 | `relate.views`, `relate.projection`, `relate.scale` | 5 | signalling | D7 (views) |
| Candidate interpretations | 5.5 | `relate.hypotheses` (generate, Z1) + `reconcile.evaluate` (deterministic) | 5/6 | branches; termination with tombstone | D8 |
| Dimension association | 5.6 | `relate.associate` | 5 | signalling; probes on ambiguous ones | D8 |
| Section/detail references | 5.7 | `relate.references` | 5 | signalling | D6 |
| Observation model | 6 | Schema gate on every append | all | — | — |
| Reconciliation RC-01…15 | 7 | `reconcile.rules`; `reconcile.investigate` directs probes at the regions of a conflict | 6 | immune response, chemotaxis | Issues |
| REVIEW REQUIRED triggers | 7.3 | Issues + coverage states → gates | 6/9 | immune | — |
| Multi-document relationships, revisions | 8 | `relate.documents`, `maintain.revision_diff` | 5/11 | signalling, regeneration | D1, D6, D8 |
| AI/human boundaries | 9.1 | Zone + schema permissions per process type | all | — | — |
| Human review workflow | 9.2 | Review phase; queue ordered by priority function | 7 | chemotaxis (ordering only) | — |
| Output contract | 10 | `review.sign_view` → signed EOS view | 7 | — | — |
| Security | 11 | Membrane + sandboxed decomposition + quarantine | 1/3 | immune | — |
| Tests | 12 | Extended by the AMOEBA benchmark | — | — | — |

### Proposed amendments to Specification 03 (from the technical review; not applied)

| ID | Review finding | Proposed amendment |
|---|---|---|
| A-03-1 | Two recognisers are not necessarily independent | §3.5/§3.6: "independent recognisers" replaced by "recognisers with a recorded dependency profile (architecture, training data, preprocessing)". Agreement counts as corroborating evidence only to the extent the **measured joint failure rate** on the benchmark supports it. Test plan: add *correlated-failure* metrics — joint error rate, conditional error rate given agreement, error correlation — per recogniser pair and per glyph class |
| A-03-2 | Vertical Japanese text: a normalised string loses meaning | §3.2/schema: every text observation retains per-glyph records (code point as recognised, glyph box in source coordinates, baseline direction, orientation, writing mode), reading order as an explicit, alternative-bearing relation, and grouping (line, block, association to symbols). The normalised string is a *derived* record |
| A-03-3 | Parentheses are not always reference dimensions | DI-30 weakened to: parenthesised (and other marked) values keep their notation as a `notation` attribute; the *interpretation* (reference, alternative unit, count, part mark, remark) is a candidate requiring a drawing- or customer-specific rule or human confirmation. Only a value whose interpretation `reference` is accepted is barred from class H |
| A-03-4 | Origin, reading state and authority are different dimensions | Schema: four orthogonal axes with the formal `usable_as_H / usable_as_S` predicates (EOS note §4); remove wording that reads as a single status |
| A-03-5 | Every library unevaluated until fully recorded | Research register: add a **Dependency Evaluation Record** per candidate — exact version, source hash, licence text hash, transitive dependencies with licences, model weights hash and licence, training-data terms, distribution obligations per deployment mode, evaluator, date. Status of every current entry: *unevaluated*. No dependency enters any lane above experimental without a complete record |
| A-03-6 | "All detected observations processed" ≠ fully inspected | Adopt the [Coverage Model](COVERAGE_MODEL.md): denominators from the source; unrecognised text, unsupported symbols, unexplained geometry and unexamined regions are counted states; ObservationSet carries a coverage statement; proposed gate G17 |

## 4. Specification 02

Coherent; UEM unaffected. Constraints AMOEBA places on itself: exploration decides *what* to compute, never *how* a qualified algorithm computes (AMB-21); verification independence (ARCH-03, MATH-40) is not altered by quorum logic; lanes implement gate G14.

| ID | Proposed amendment |
|---|---|
| A-02-1 | §10: add gate G17 `coverage_complete` (critical; not resolvable by Decision except per-element exclusion) |
| A-02-2 | §10.2 G14: reference the lane model |
| A-02-3 | §9.1: independence classes extended with a "shared dependencies" field, aligning with quorum rules |

## 5. ADRs

No existing ADR decision is changed. ADR-001 (modular core + isolated workers) and ADR-006 (immutable content-addressed graph) are the foundations AMOEBA builds on. New ADRs are proposed in [PROPOSED_ADRS.md](PROPOSED_ADRS.md).

## 6. Master plan

Proposed (not applied): describe AMOEBA as the governing architecture in §4; relate the four "prioritised innovations" to AMOEBA components (Truth Graph → EOS records; Verification Tribunal → Verify phase with independence-aware thresholds; Uncertainty Map → provenance axes + Issues; Fabrication Rehearsal → Build/Verify processes). The development-class wording in §6.2 C is left as is.

## 7. Migration order (documentation only)

1. Owner confirms the AMOEBA definition.
2. Independent architecture review.
3. Decide proposed ADRs 009–014 and amendments A-01…A-07, A-02-x, A-03-x.
4. Apply accepted amendments as new versions of Specs 01–03 (with change logs).
5. Write Specification 04 natively against the EOS and the process model.
