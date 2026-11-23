# Reference-Case Intake Specification

Version 1.0 · 18 November 2026 · Owner decision D-43 · Part of the [Engineering Benchmark Plan](../ENGINEERING_BENCHMARK_PLAN.md)

## 1. Purpose

Define how an authorised factory engineering drawing is accepted as a **controlled reference case**: a preserved original, with independently produced and reviewed ground truth, kept strictly apart from anything the system predicts. The first case, **RC-001**, is a header with an oblique branch.

No material has been received. This specification, the templates and the RC-001 folder contain **structure only**. Nothing in them is engineering evidence.

## 2. Principles

1. **Preserve the original.** Files are stored unmodified with their hash; every annotation refers to the original by coordinates.
2. **Truth before prediction.** Ground truth is produced by people before any system output for that case exists, or by people who have not seen such output.
3. **Truth and prediction never share a record.** Different files, different storage areas, different write permissions (§6).
4. **Nothing is invented.** A missing dimension, process parameter, standard or manufacturing decision is recorded as missing. The annotator does not supply a plausible value.
5. **Unavailable is not rejected.** Evidence that was never supplied and evidence that was reviewed and rejected are different states (§4).
6. **Reference first, tuning never without a record.** A case is not used to tune anything until it reaches state S2 and has been designated (§5).
7. **No validation claim** follows from a case below state S4, and then only the claim stated in §7.

## 3. Where things are stored

| Material | Location | In this repository? |
|---|---|---|
| Original drawings, CAD files, approved developments, DXFs, photographs, inspection records | Access-controlled confidential store, per the authorisation's terms | **Never** (benchmark plan §7) |
| Hashes, authorisation references, provenance record | Case folder (identifiers only, no drawing content) | Yes |
| Ground-truth annotation values (dimensions, geometry, dependencies) | Confidential store by default — they reveal the design. In the repository only if the authorisation explicitly allows | Default no |
| Templates, status, missing-evidence report (without confidential content) | Case folder | Yes |
| System predictions and comparison results | Separate prediction store keyed by case, system version and run | Never in the truth area |

## 4. Evidence status vocabulary (used for every item in every record)

| Status | Meaning |
|---|---|
| `not_requested` | Not yet asked for |
| `requested` | Asked for; not received |
| `unavailable` | The source confirms it does not exist or cannot be found |
| `withheld` | Exists but is not authorised for our use |
| `received_unreviewed` | In hand; no qualified person has examined it |
| `reviewed_accepted` | Examined by a named reviewer and accepted for the stated scope |
| `reviewed_rejected` | Examined by a named reviewer and **rejected**, with reason (e.g. the approved development is itself wrong, or does not correspond to this revision) |
| `reviewed_disputed` | Reviewers disagree; both positions recorded |
| `not_applicable` | Does not apply to this case, with reason |

`unavailable`, `withheld` and `reviewed_rejected` must never be merged or summarised as "missing".

## 5. Case lifecycle

| State | Name | Entry conditions |
|---|---|---|
| **S0** | Prepared | Case folder and templates exist; no material |
| **S1** | Intake complete | Authorisation recorded with scope; originals stored unmodified and hashed; provenance record complete for every file; designation recorded (below) |
| **S2** | Annotated | Ground-truth annotation completed by a named annotator with stated qualifications, without sight of any system output; missing-evidence report issued |
| **S3** | Engineer-reviewed | A qualified fabrication engineer, independent of the annotator, has reviewed the annotation and the dependency edges and signed; disagreements recorded |
| **S4** | Independently validated engineering benchmark | All of §7 satisfied |
| **S5** | Physically corroborated (optional) | Inspection measurements of the fabricated part, with measurement uncertainty, compared with the reference |

**Designation (at S1, before any system run):** `development` (may inform design and tuning, results never used as confirmatory evidence) or `held_out` (sealed; Benchmark Protocol rules apply). **RC-001 is designated `development`:** as the first real case it will unavoidably shape the design, so it cannot later serve as unbiased confirmatory evidence. It remains the first genuine engineering *validation case* for the dependency model and the mathematics.

A case moves backwards when its evidence is withdrawn or a reviewer retracts; the change is recorded.

## 6. Separation policy: reference truth versus AMOEBA predictions

| Rule | Detail |
|---|---|
| SP-1 Separate stores | Truth area and prediction area are distinct, with distinct write permissions. System processes have **no write access** to the truth area and, for held-out cases, no read access either |
| SP-2 Order | Truth is frozen (hash recorded, state S2) before the first system run on the case. Any system run before S2 is recorded and the case is marked `truth_possibly_influenced` |
| SP-3 Blind annotation | Annotators and reviewers do not see system output for the case while producing or reviewing truth |
| SP-4 No back-filling | A value the system produced is never copied into the truth, even if it "looks right". If truth lacks something, it is a gap |
| SP-5 Corrections | Truth changes only through a change record: who, when, what, why, and whether the change was prompted by seeing a system output (`post_hoc`). Post-hoc corrections are legitimate — the system may expose a real annotation error — but they are flagged, and metrics are reported both with the original and with the corrected truth |
| SP-6 Three-way outcomes | Every comparison item is classified: agree · system wrong · truth wrong (via SP-5) · both defensible · truth silent (gap) · system silent. "Truth silent" is never scored as system error or system success |
| SP-7 Tuning record | Any use of a case to adjust tables, thresholds, templates, vocabularies or code is logged against the case. A case so used is permanently `development` |
| SP-8 Provenance of predictions | Each prediction set records system version, configuration hash, lanes, date; it is evidence about that version only |
| SP-9 Evidence levels | Truth items carry [evidence levels](../../architecture/EVIDENCE_LEVELS.md) (EV-2 engineer-reviewed, EV-3 factory-observed, …). System output is never an evidence level |

## 7. What converts a case from an intake record into an independently validated engineering benchmark (S4)

All of the following, each filed with its record:

1. **Authorisation** in writing from the drawing's owner, stating permitted uses (internal testing; whether external reading services may process it; whether annotation values may be stored in the repository).
2. **Complete provenance** for every file: origin, revision identity, how received, hash, and the relationship between files (which development and which DXF correspond to which drawing revision).
3. **Engineer-confirmed dimensions:** every dimension on the drawing transcribed and confirmed against the original by a named person, with units, tolerances, surface reference (inside/outside/centre) and any ambiguity recorded as ambiguity.
4. **Geometric interpretation** confirmed by a fabrication engineer: projection method, views, scale, what each component is, joint type (set-on / set-in), the surfaces that define the fit.
5. **Interface constraints and dependency edges** reviewed by a fabrication engineer independent of the annotator, using the dependency-review evidence record — each edge classified and reasoned, missing edges added.
6. **Manufacturing reference:** the factory's approved development and/or DXF for the same revision with status `reviewed_accepted`; **or**, if none exists or it was rejected, an independently produced reference development with its method and reviewer — and the case is then labelled *"no factory-approved reference"*.
7. **Manufacturing decisions and allowances actually used** (plate division, seam positions, reference surface, allowances, kerf handling), each with its source; values not known are recorded as unknown, and the comparison for that item is then not performed.
8. **Mathematical model review:** the Spec 02 model(s) used for this geometry independently reviewed (model review record) and the reference geometry independently recomputed by a second person or method.
9. **Known errors and ambiguities** of the original recorded, with how the factory resolved them (if known).
10. **Independent reviewer sign-off** covering items 3–9, by a reviewer meeting the stated qualifications and independent of the annotator and of the system's authors.
11. **Separation policy compliance** statement (SP-1…SP-9), including whether any system run preceded the truth freeze.
12. **Missing-evidence report** with no item in an undefined state: every expected item is one of the statuses of §4.

At S4 the permitted claim is exactly: *"Reference case RC-nnn has engineer-reviewed ground truth for the comparison dimensions marked complete, for the stated revision."* It is one case. It validates neither the dependency model in general, nor any factory rule beyond that case, nor any process parameter (that needs calibration evidence, EV-5), nor physical conformity (S5).

## 8. Comparison dimensions

The annotation schema supports comparison on ten dimensions. For each case and each dimension the status is recorded (truth complete / partial / absent) so that a system result is only ever compared with truth that exists.

| # | Dimension | Truth content | Comparison |
|---|---|---|---|
| 1 | Source interpretation | Documents, sheets, revision, title-block fields, which content belongs to which part | Exact match of identities and assignments |
| 2 | Dimensions and units | Every written dimension: value as written, unit, tolerance, kind, surface reference, flags, source region | Exact value match; attribute match; listed misses and inventions |
| 3 | Projection and views | Projection method and its evidence; views, their types, relations, scales | Match; correct flagging where truth says "ambiguous" |
| 4 | Component geometry | Components, parameters, surfaces, as interpreted by the engineer | Parameter comparison within a tolerance **stated by the reviewer** |
| 5 | Interface constraints | Joints, mating features, fit conditions | Presence and content |
| 6 | Dependency edges | Reviewed edges with type and criticality | Missing / spurious / correct (Protocol categories K, U, S) |
| 7 | Surface development | Reference pattern(s) and the surfaces/assumptions they are based on | Boundary deviation, lengths, areas — only against a `reviewed_accepted` reference |
| 8 | Plate division and seam decisions | What the factory decided and why | Feasibility and difference; not "correctness" unless a rule is cited |
| 9 | Manufacturing allowances | Allowances used, with source and evidence level | Only where the truth value has a source; otherwise not compared |
| 10 | Expected DXF output | The approved DXF (layers, entities, units) | Geometric comparison and structural comparison, against a `reviewed_accepted` DXF |

Tolerances for numerical comparison come from the reviewer or the drawing; none is assumed here.

## 9. Records and templates

| # | Record | Template |
|---|---|---|
| 1 | Drawing and revision provenance | [01_PROVENANCE_RECORD.md](templates/01_PROVENANCE_RECORD.md) |
| 2 | Ground-truth annotation | [02_GROUND_TRUTH_ANNOTATION_SCHEMA.md](templates/02_GROUND_TRUTH_ANNOTATION_SCHEMA.md) |
| 3 | Dependency-review evidence | [03_DEPENDENCY_REVIEW_EVIDENCE.md](templates/03_DEPENDENCY_REVIEW_EVIDENCE.md) |
| 4 | Mathematical model review | [04_MATHEMATICAL_MODEL_REVIEW_RECORD.md](templates/04_MATHEMATICAL_MODEL_REVIEW_RECORD.md) |
| 5 | Independent reviewer sign-off | [05_REVIEWER_SIGN_OFF.md](templates/05_REVIEWER_SIGN_OFF.md) |
| 6 | Missing-evidence and unresolved-ambiguity report | [06_MISSING_EVIDENCE_AND_AMBIGUITY_REPORT.md](templates/06_MISSING_EVIDENCE_AND_AMBIGUITY_REPORT.md) |

First case: [RC-001-header-oblique-branch](RC-001-header-oblique-branch/CASE_STATUS.md).
