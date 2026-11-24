# Ground-Truth Annotation Schema

> Template. Contains no engineering evidence. Fill in only from the source material or from a named person's statement; leave unknown items as their status, never as a guess. Status vocabulary: [intake specification §4](../REFERENCE_CASE_INTAKE_SPECIFICATION.md).
> Produced by a named annotator **without sight of any system output** for the case (separation policy SP-2, SP-3). Every item cites the source region on the original. Where the drawing is ambiguous, record the ambiguity — do not resolve it unless a named engineer does, and then record who.

## Header
| Case ID | Annotator (name, qualifications) | Dates | Files annotated (IDs, revisions) | Truth version and hash | System output seen before annotation? (must be "no", or explain) |
|---|---|---|---|---|---|
| | | | | | |

Each item below carries: **status** (intake §4), **evidence level** (EV-2…EV-5, [definition](../../../architecture/EVIDENCE_LEVELS.md)), **source** (file ID, sheet, region), **confirmed by**.

## 1. Source interpretation
| Item | Value | Source | Status | Confirmed by |
|---|---|---|---|---|
| Parts shown and their identifiers | | | | |
| Which views / notes / tables belong to which part | | | | |
| Sheet-level declarations: units, general tolerance, scale, projection statement | | | | |
| Content judged irrelevant to the part (and why) | | | | |

## 2. Dimensions and units (one row per written dimension — all of them)
| Dim ID | Value **exactly as written** | Unit (stated / sheet default / unknown) | Kind (linear, diameter, radius, angle, …) | Tolerance as written | Notation (parentheses, underline, frame, prefix, count) | Interpretation of notation — and who decided | Feature it measures | Surface reference (inside / outside / centre / unknown) — and basis | Source region | Legibility (clear / doubtful / illegible) | Confirmed by |
|---|---|---|---|---|---|---|---|---|---|---|---|
| | | | | | | | | | | | |

Dimensions needed to define the part that are **not on the drawing**: list in the missing-evidence report. Do not enter a value here.

## 3. Projection and views
| Item | Value | Evidence on the drawing | Status | Confirmed by |
|---|---|---|---|---|
| Projection method (first / third angle / not determinable) | | (symbol / text / geometry / factory statement) | | |
| Views: ID, type, label as written, region | | | | |
| View relations (which is projected from which, sections, details) | | | | |
| Scale per view; any not-to-scale indication | | | | |
| Would the other projection method give a mirror-image part? | | | | |

## 4. Component geometry (engineer's interpretation)
| Component | Shape type | Parameter | Value | Derived from which dimensions (Dim IDs) | Interpretation decisions made, and by whom | Status |
|---|---|---|---|---|---|---|
| Header | | | | | | |
| Branch | | | | | | |
| (others) | | | | | | |

Position and orientation of the branch relative to the header: angle, offset, station — each with source Dim IDs.

## 5. Interface constraints
| Interface ID | Parts | Joint type (set-on / set-in / butt / fillet / flanged / …) and source | Mating features | Which wall surfaces define the fit — and who says so | Fit condition (gap, alignment rule) and source | Weld preparation as specified, and source | Status |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

## 6. Dependency edges
Recorded in the [dependency-review evidence record](03_DEPENDENCY_REVIEW_EVIDENCE.md); only edges reviewed by an engineer are truth.

## 7. Surface development (reference)
| Pattern | For component / surface | Reference source (factory-approved development file ID / independently produced) | Reference surface used (inside / mean / outside / other) and basis | Method by which the factory produced it, if known | Status (`reviewed_accepted` required for comparison) | Reviewer |
|---|---|---|---|---|---|---|
| | | | | | | |

## 8. Plate division and seam decisions
| Decision | What was decided | Reason (as stated by the factory, or unknown) | Source | Status |
|---|---|---|---|---|
| Number of pieces | | | | |
| Seam position(s) | | | | |
| Rolled from plate or purchased pipe (each component) | | | | |
| Stock plate used | | | | |

## 9. Manufacturing allowances actually used
| Allowance (rolling/neutral factor, weld gap, shrinkage, trim, kerf and who compensates, bevel) | Value | Source (procedure, engineer statement, measured, **unknown**) | Evidence level | Status |
|---|---|---|---|---|
| | | | | |

Unknown allowances stay unknown; comparison on that item is then not performed.

## 10. Expected DXF output
| File ID | Corresponds to drawing revision | DXF version, units as stored | Layers and their meaning (as the factory states) | Entity types used | Which profiles are cut / marked / reference | Accepted by which CAM system | Status |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

## 11. Known errors and ambiguities in the source
| ID | Description | Location | How the factory resolved it (source) or "unresolved" | Effect on the part |
|---|---|---|---|---|
| | | | | |

## 12. Comparison readiness
| Dimension (1–10) | Truth complete / partial / absent | Numerical tolerance for comparison — value and who set it | Notes |
|---|---|---|---|
| | | | |

## Change record (separation policy SP-5)
| Date | Item | Old | New | Reason | Prompted by a system output? (post_hoc yes/no) | Changed by | Approved by |
|---|---|---|---|---|---|---|---|
| | | | | | | | |
