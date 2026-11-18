# Drawing Intelligence Test Plan

Version 0.1 · 18 November 2026 · Companion to [Spec 03](../specifications/03_DRAWING_INTELLIGENCE_AND_INGESTION_SPECIFICATION.md); part of the [Engineering Benchmark Plan](ENGINEERING_BENCHMARK_PLAN.md) (tiers S/H/A)

**No test data exists yet.** Real drawings enter only with written authorisation (`FR-10-1`). Thresholds are not set here: they come from Specification 05/10 after the first baselines and factory tolerances. This plan fixes *what* is measured and *how*.

## 1. Data sources

| Source | Use | Availability |
|---|---|---|
| **Synthetic (tier S)** — drawings generated from known parametric parts: vector PDF and DXF with known dimensions, views, projection method and associations; then degraded to simulated scans (resolution, skew, rotation, noise, blur, JPEG/JBIG2-type compression, anisotropic scaling, fax artefacts, stamps/handwriting overlays) | Exact ground truth; controlled sweeps of one factor at a time | Can start now; requires a generator with Japanese and English annotation styles. Synthetic drawings do not capture real drafting habits — results on them are a floor, not evidence of field performance |
| **Authorised factory drawings (tier H)** | Realistic content, title blocks, conventions, defects | Open requirement |
| **Adversarial (tier A)** — malformed/hostile files, ambiguous, contradictory, incomplete, mixed-revision packages | Refusal and conflict-detection behaviour | Partly synthetic now; real examples from the factory |

Ground truth for real drawings: two independent annotators + adjudication (benchmark plan §4); annotation covers text strings, parsed dimensions, entity roles, views, projection method, associations, conflicts deliberately present, and the expected outcome (*process / review / reject*).

## 2. Test classes

| ID | Class | Variations | Expected behaviour to verify |
|---|---|---|---|
| T-VEC | Vector PDFs | Different CAD producers; text as text / as outlines / stroked fonts; missing Unicode maps; layers; rotated pages; `UserUnit`; hybrid pages; hidden OCR layers; annotations/markups | Correct page classification; exact geometry; render cross-check catches unreadable text; markups kept separate |
| T-LOW | Low-resolution scans | Resolution sweep down to illegibility | Graceful degradation: `low_resolution` flag raised before numeric false accepts rise; no confident wrong digits unflagged |
| T-HIGH | High-resolution scans | Large formats (A1/A0 at high dpi) | Correctness within resource limits; no silent downsampling |
| T-SKEW | Skewed drawings | Angle sweep | Deskew residual; coordinates traceable to source pixels (round-trip test) |
| T-ROT | Rotated drawings | 90/180/270; mixed orientations in one file; text in two directions | Correct orientation or explicit ambiguity |
| T-PERSP | Photographs | Perspective, uneven lighting | Lower-trust class assigned; measurement uncertainty reflects it |
| T-ANISO | Anisotropic scaling | x/y scale differences | Detected and reported; no false written-vs-measured conflicts caused by it |
| T-JA | Japanese annotations | Horizontal/vertical; full/half width; confusable characters; small text; handwriting; stamps over text | Character and term accuracy; alternatives contain the truth; vocabulary never auto-corrects silently |
| T-MIX | Mixed Japanese and English | Mixed strings, dual-language title blocks | Correct segmentation and parsing |
| T-DIM | Complex dimension chains | Chain, baseline, ordinate, angular, arc length, counts, tolerances in all forms, reference/not-to-scale/framed dimensions, crowded text | Parsing, flags, chain closure checks |
| T-SYM | Symbols | Diameter/radius/square/chamfer; welding; surface finish; projection symbol | Symbol accuracy; unparsed welding info yields review reason |
| T-PROJ | Projection conventions | First-angle and third-angle versions of the same part; with/without symbol; symbol contradicting geometry; symmetric and chiral parts | Method identified from evidence; conflict detected; **mirror-ambiguous cases yield Ambiguous, immaterial cases recorded as immaterial** |
| T-VIEW | Multi-view relationships | 2–6 views, sections, details at other scales, auxiliary, partial/broken views, pictorial views, designer's development view | View typing, relations, scales, correspondences; broken views suppress measurements |
| T-ASSOC | Dimension association | Dense drawings; leaders crossing; dimensions outside the view; shared extension lines | Association accuracy; `unassociated` instead of nearest-geometry guesses |
| T-CONF | Contradictory dimensions | Seeded: digit changed in one view; chain not closing; DXF text override; written vs measured; diameter/radius swap; inside/outside inconsistency; conflicting tolerances; nominal size vs written OD | Each seeded conflict detected with the correct rule ID and affected observations; written value never altered |
| T-MISS | Missing dimensions | One required dimension removed | Gap record naming the undetermined parameter; value never supplied from measurement automatically |
| T-AMB | **Ambiguous drawings (correct rejection)** | Under-defined geometry with ≥2 consistent interpretations; unknown units; unknown surface reference; projection ambiguity on a chiral part; unreadable critical digit | Outcome is REVIEW REQUIRED with the specific reason code; **any case that reaches a signed-off, releasable observation set without human resolution is a test failure of the highest severity** |
| T-REV | Revision conflicts | Mixed revisions in a package; same revision different content; hand-marked change without revision entry; newer file with older revision | Detected; no "latest upload wins" |
| T-PKG | Large engineering packages | Many sheets, several parts per sheet, assemblies + details + reference documents, missing sheets | Correct document/part identities; no cross-part contamination; missing members reported; performance |
| T-CAD | DXF (and DWG/STEP when available) | Units unset; Shift-JIS text; nested mirrored blocks; paper-space viewports; XREFs; unsupported entities; invalid STEP B-rep | Unit never defaulted; unsupported content itemised |
| T-BAD | Malformed and hostile files | Truncated, corrupted, encrypted, restricted, decompression bombs, extreme page/object counts, active content, polyglots, fuzzed inputs | Explicit rejection reasons; sandbox containment; no crash of the service; resource limits enforced |
| T-HUM | Review workflow | Seeded wrong readings and associations presented to reviewers | Human catch rate; time per item; audit completeness (also a UX study) |
| T-DET | Determinism | Repeated runs, thread counts, platforms | Identical observation sets (content hashes) on the same build; documented equivalence otherwise |

## 3. Metrics

| Area | Metrics |
|---|---|
| OCR characters | Character error rate overall and by class: digits, decimal point, sign, Latin, kana, kanji |
| Technical symbols | Per-symbol precision/recall (φ, R, SR, □, C, ±, °, ×, welding and finish symbols, projection symbol) |
| Numeric tokens | Exact-match rate of the whole value (a single wrong digit = wrong); **alternative coverage**: rate at which the true value is among the n-best when the top reading is wrong |
| Dimension extraction | Detection precision/recall; parsed-field accuracy (kind, value, unit, tolerance, flags, count) |
| Geometric features | Vector: exact entity recovery and primitive-recognition residuals. Raster: detection precision/recall per primitive; positional error in source pixels and sheet mm; role-classification accuracy per role |
| View classification | Region detection IoU; type accuracy; relation accuracy; scale error; projection-method accuracy and ambiguity-detection accuracy |
| Dimension association | Accuracy of target entities/features; rate of `unassociated` on truly unassociable vs associable dimensions |
| Conflict detection | Recall and precision on seeded conflicts per rule; correct identification of affected observations |
| Provenance completeness | Fraction of observations with resolvable source region, method and transform chain (required: all); round-trip error of mapping back to source coordinates |
| Human correction | Corrections per drawing by type; review time; fraction of class-H values needing correction; reviewer catch rate on seeded errors |
| Performance | Time and peak memory per stage and per page by input class; throughput; behaviour at limits (methodology: Spec 01 §7.1) |
| Calibration | Reliability of any score presented as a probability; otherwise scores are reported as uncalibrated |

### Safety metrics (reported separately, never averaged away)

- **Numeric false-accept rate:** wrong class-H value reaching `corroborated` (or any automatically accepted state) — the figure that decides whether DI-33's default can ever be relaxed.
- **Association false-accept rate:** wrong association passing the consistency check.
- **Missed-conflict rate** on seeded conflicts.
- **False-release rate at ingestion:** T-AMB / T-CONF / T-MISS cases producing a set with `review.required = false`.
- **Silent-change count:** any instance where a written value in the output differs from the source without a human correction record (required: zero; each instance is a defect).

Every instance counted in a safety metric is analysed individually and added to the regression set.

## 4. Protocols

1. **Stage-isolated and end-to-end** scoring (benchmark plan §6).
2. **Factor sweeps** on synthetic data to find where each flag (low resolution, extraction unreliable) must trigger; flag thresholds are then validated on real drawings.
3. **Engine comparisons** on identical inputs for every candidate in the research register; results recorded with engine version, weights hash and hardware.
4. **Local-only versus external** model comparison, on drawings whose authorisation allows external processing.
5. **Held-out discipline:** tuning (vocabularies, title-block templates, thresholds, any training) uses the development set only.
6. **Reviewer studies** with seeded errors (T-HUM), with consent, to measure how reliably humans catch wrong readings in the proposed UI.

## 5. Qualification rule

A recogniser, rule or model version is admitted to production use only with: results on tiers S and H for the input classes it will serve; safety metrics reported with counts and confidence intervals; stated limits (input classes, resolutions, languages) outside which its output is flagged; and no open silent-change defect. Admission never removes the per-job human validation required by Spec 03 §3.6 unless Specification 05 explicitly permits it for a stated case.

## 6. Requirements on the factory

`FR-10-1…9` and `FR-03-1…12`: authorised sample drawings across input classes; annotator time from engineers; examples of real ambiguous/erroneous drawings and how they were resolved; permission scope for external processing and for use in tuning.
