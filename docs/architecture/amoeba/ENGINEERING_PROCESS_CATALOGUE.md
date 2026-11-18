# Engineering-Side Process Catalogue

Version 0.1 (preparation for Specification 04) · 18 November 2026 · Proposed · Responds to review finding W1

The drawing-side processes are catalogued in the [migration note](EXISTING_ARCHITECTURE_MIGRATION.md) §3. This note gives the engineering side — Build, Verify, Investigate, Propagate, Audit, Release — the same treatment, so that AMOEBA's governance of the whole lifecycle is designed rather than asserted. Contracts are outlines; Specification 04 defines them fully. Algorithms refer to Spec 02 (provisional).

M = mandatory when applicable; D = discretionary (budgeted, investigation-linked).

## Build (Z2)

| Process | Inputs | Outputs (Claims / Relations) | Class |
|---|---|---|---|
| `build.structure` | Signed observation view; document/part relations | Assembly → Part structure; `part_of` | M |
| `build.identify` | Accepted observations for a part | Candidate shape-module identifications (dispatch to every candidate) | M |
| `build.constraints` | Usable-as-H dimensions, usable-as-S geometry, accepted hypotheses | Constraint system; `defined_by` | M |
| `build.solve` | Constraint system | Parameters with residuals, DOF report (ALG-01/02); Findings for under-/over-determination, ambiguity | M |
| `build.interfaces` | Parts, assembly placements, joint information | Interface records, interface constraints; `mates_at`, `positioned_by` | M |
| `build.surfaces` | Parameters, module definitions | Surfaces, features; `bounded_by` | M |
| `build.reference` | Thickness, material, ProcessData, conventions | Reference-surface and contact-surface assumptions; `assumes`, `uses_*` | M |
| `build.intersect` | Surfaces at an interface | Intersection curves (ALG-06); `intersects` | M |
| `build.classify` | Surfaces | Development class evidence (ALG-05) — class labels follow whatever scheme the Spec 02 review settles | M |
| `build.develop` | Surface, cuts, seam specification, assumptions | Patterns with bounds (ALG-03/04/07); `developed_from` | M |
| `build.divide` | Patterns, stock, rules, cost configuration | Feasible strategies, Pareto set, explanation (ALG-12); pieces, seams after engineer's selection; `divided_from`, `joined_by` | M |
| `build.allowances` | Pieces, bends, welds, ProcessData | Allowance-adjusted geometry; `welded_as`, `formed_by` | M |
| `build.profile` | Pieces | Profiles with representation error (ALG-09), kerf handling (ALG-10); `rendered_from` | M |
| `build.dxf` | Profiles, output profile configuration | DXF entities, entity→profile map; package manifest | M |

## Verify (Z3)

| Process | Checks | Class |
|---|---|---|
| `verify.constraint` | Independent residual evaluation; interval certification | M |
| `verify.family.<module>` | Family-specific independent method (Spec 02 §9.2, ALG-13) | M |
| `verify.interface` | Interface constraints: both sides of an intersection give the same curve; seam edges equal in developed length and stations; mating circumferences equal on declared surfaces | M |
| `verify.rehearsal` | Fold-back and assembly of patterns into the intended geometry; fit-up gaps | M where a forming model exists |
| `verify.profile` | Closed, simple, oriented, nested profiles (independent implementation of ALG-08) | M |
| `verify.reimport` | Independent parser re-import and comparison with approved geometry | M |
| `verify.budget` | Tolerance/uncertainty budget (Spec 02 §7.4, §8) | M |
| `verify.process` | Machine, stock and seam-rule constraints | M |

## Investigate (Z2 logic directing Z1/Z2/Z3 probes)

| Probe | Discriminates / establishes | Class |
|---|---|---|
| `probe.alt_formulation` | Primary vs second analytic formulation | D (M after a verification disagreement) |
| `probe.kernel_crosscheck` | Result vs external kernel (independence class I-L) | D |
| `probe.precision` | Numerical vs model cause (re-evaluation in higher precision / interval) | D |
| `probe.uncertainty_mc` | Whether first-order uncertainty is valid; near-limit budget cases | D (M when the non-linearity diagnostic fails) |
| `probe.sensitivity` | Which inputs dominate an output; feeds risk weights | D |
| `probe.interface_fit` | Whether an interface failure is caused by this side or the partner | D |
| `probe.tolerance_stack` | Stack-up across an interface chain | D |
| `probe.convention` | Alternative surface-reference/unit/projection interpretation consistent with all evidence? | D |
| Drawing-side probes (re-recognition, alternative association, …) | As in the migration note | D |

## Propagate, Audit, Assess, Release

| Process | Zone | Function | Class |
|---|---|---|---|
| `propagate.obligations` | Z2 logic | Dependency traversal and obligation creation from the rule table ([Dependency Model](DEPENDENCY_MODEL.md) §4) | M |
| `investigate.manage` | Z2 logic | Open/advance/close investigations; test selection ([Investigation Model](INVESTIGATION_MODEL.md)) | M (management), tests per above |
| `audit.recompute` (A1), `audit.invariance` (A2), `audit.cross_examine` (A3), `audit.assumptions` (A4), `audit.replay` (A5) | Z3 | Self-consistency | M on release closure |
| `coverage.evaluate` | Z3 | Coverage statement at declared level | M |
| `closure.compute` | Z3 | Release closure and hash | M |
| `gates.evaluate` | Z3 | Gate evaluation record `E(P)` | M |
| `residual.statement` | Z3 | Residual Uncertainty Statement | M |
| `release.approve` | Z4 | Engineer's Approval over the closure hash | M |
| `maintain.revision`, `maintain.regenerate`, `maintain.retire`, `maintain.reclaim` | per content | Revision diff, recomputation, retirement, cache reclamation | as triggered |

## Feedback paths (what makes the lifecycle a loop)

| From | Signal / Finding | To |
|---|---|---|
| `build.solve` under-determined | Gap Finding naming the free parameter | Drawing-side investigation of the regions where that dimension could be; review |
| `verify.*` FAIL / INCONCLUSIVE | Finding + investigation | `probe.*`; `audit.cross_examine`; obligations by `computed_by` |
| `verify.interface` FAIL | Finding on the interface | Obligations on both parts; `probe.interface_fit`; possibly drawing-side re-examination of the dimensions defining the interface |
| `verify.budget` FAIL | Finding with sensitivity ranking | Review of the dominant inputs (confirm reading, calibrate ProcessData) |
| Review correction | Supersession | `propagate.obligations` → rebuild and re-verify the affected closure |
| Audit discrepancy | Finding | Quarantine; investigation |

## Not yet designed (for Specification 04)

Shape-module contracts in process form; interface-constraint types per joint; nesting; multi-part packages with shared stock; forming-method models; the domain contract for other fabrication domains; cost functions in work units for each process.
