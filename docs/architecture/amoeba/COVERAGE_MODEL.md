# Coverage Model

Version 0.1 · 18 November 2026 · Supporting note to the [AMOEBA architecture](../../specifications/AMOEBA_NATIVE_SYSTEM_ARCHITECTURE.md) §6. Proposed [P].

## 1. Why

A system that reports "all detected observations processed" says nothing about what it failed to detect. Coverage must be measured against the source, and must make unrecognised text, unsupported symbols, unexplained geometry and unexamined regions countable.

## 2. Denominators (fixed at engulfment, per Snapshot)

| Domain | Countable elements | How enumerated (independent of interpretation) |
|---|---|---|
| D1 Documents | Files in the Snapshot | Snapshot |
| D2 Pages / spaces | PDF pages, image frames, DXF model/paper spaces and layouts, STEP products | Structural parse |
| D3 Vector content | Every painted path, text object, image and annotation object (PDF); every entity incl. those inside blocks (DXF/DWG); every shape (STEP) | Parser enumeration |
| D4 Raster content | Every connected ink component (or tile of ink, for very large components) of the **unfiltered** image above a declared size, plus a residual "faint ink" measure below it | Deterministic image analysis with recorded parameters |
| D5 Annotations | Text-like groups (from D3 text objects, glyph-outline clusters, D4 text-like component clusters), symbols | Derived from D3/D4 |
| D6 Explicit references | Section/detail labels, drawing references, balloons/item marks, revision marks | From D5 after recognition (so D6 inherits D5's blind spots — stated) |
| D7 Engineering objects | Parts, surfaces/features, parameters, constraints, patterns, profiles | From the EOS engineering side |
| D8 Dependencies | Cross-reference edges and boundary references | From the EOS |

## 3. States (exactly one per element)

| State | Definition |
|---|---|
| `fully_examined` | (a) explained: linked to at least one current record that accounts for it (a part-geometry role, an annotation, sheet furniture, …) **or** excluded by a recorded human Decision (e.g. company logo); (b) every process applicable to its type has completed; (c) no open Issue involves it; (d) no unresolved boundary reference |
| `partially_examined` | Some applicable processes completed; at least one pending, budget-stopped, failed, or a boundary reference unresolved |
| `unexamined` | No interpreting process has completed on it |
| `unsupported` | Identified as a kind of content no production process can interpret |
| `ambiguous` | Examined; more than one surviving interpretation, or an open conflict involves it |

"Detected but unrecognised" (ink or text-like content that no process could explain) is `partially_examined` with reason `unexplained` — it is never dropped from the count.

Engineering-domain analogues (D7): a parameter is fully examined when determined, traceable and covered by required verification; a constraint when consumed and its residual certified; a profile when validated and compared after re-import.

## 4. Aggregation and reporting

- Parent state = worst child state in the order `unexamined > unsupported > ambiguous > partially_examined > fully_examined`.
- A **coverage statement** is a record: EOS version, scope, and for each domain the count (and area/ink share where meaningful) per state, with links to the element lists. No single percentage is ever the headline.
- Statement wording is generated from the counts. "Fully inspected" (or equivalent) is produced only when all counts outside `fully_examined` are zero for the stated scope (AMB-11).
- Every statement carries the fixed caveats: coverage is not correctness; D4 cannot count marks below the ink threshold; D6 depends on recognition.

## 5. Coverage gate (proposed G17)

`coverage_complete(part P, EOS v)` = PASS iff every element whose region intersects the views, notes, tables and documents related to P is `fully_examined`; and every other element of the Snapshot is either `fully_examined` or assigned to another part/scope or excluded by Decision. INCONCLUSIVE if enumeration itself was limited (e.g. tile-level D4 on an over-size scan). FAIL otherwise. Not resolvable by Decision except through explicit per-element exclusion records.

## 6. Limits and open questions

- Cost and stability of ink-component enumeration on large or noisy scans; choice of size threshold (benchmark).
- Touching content (text over linework) forms one component: accounted as `partially_examined` until separated.
- Hidden or masked PDF content (clipped, white-on-white) is in D3 and must be explained or flagged.
- Coverage of *absent* information (a missing sheet, a dimension never drawn) is not coverage at all; it is handled by gap Issues.
- Reviewer workload created by exclusion decisions for irrelevant content; mitigation through per-customer sheet-furniture templates (cellular memory), measured in the benchmark.
