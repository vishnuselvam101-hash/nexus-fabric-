# RC-001 — Header with Oblique Branch: Case Status

Version 1.0 · 18 November 2026 · Governed by the [Reference-Case Intake Specification](../REFERENCE_CASE_INTAKE_SPECIFICATION.md)

## Status

| Field | Value |
|---|---|
| Case ID | RC-001 |
| Geometry | Cylindrical header with one oblique branch (the actual drawing may differ; the case follows the drawing, not this description) |
| Designation | `development` — fixed in advance: as the first real case it will inform the design and cannot later be used as confirmatory benchmark evidence |
| Lifecycle state | **S0 — Prepared.** No material received |
| Authorisation | None |
| Files held | None |
| Ground truth | None |
| Reviewer | None appointed |
| System runs on this case | None (no system exists) |
| Evidence level of anything in this folder | Not applicable — the folder contains structure only |

**This case contains no engineering evidence. Nothing about it may be described as validated, reviewed or observed.**

## What to request from the factory (first request)

1. One **authorised** drawing of a header with an oblique branch, in the form it was originally received (PDF, scan, DXF/DWG — unaltered), with written authorisation and its scope.
2. **All revisions** of that drawing that exist, and what changed between them.
3. The **approved development** and/or **DXF** that the factory produced for it, and which drawing revision each corresponds to.
4. How the part was actually made: rolled plate or purchased pipe for each component; set-on or set-in joint; plate division and seam position; allowances used and where they come from; who applied kerf compensation.
5. Any **known error, ambiguity or query** on this drawing, and how it was resolved.
6. Whether the part was fabricated, and any inspection or nonconformity record.
7. The name and qualifications of the engineer who can confirm dimensions and interpretation, and of a second engineer who can review independently.

If an item does not exist, we need that stated — "does not exist" is different from "not sent".

## Missing-evidence report (instantiated from the template)

### A. Not yet obtained

| Item | Status | Needed for |
|---|---|---|
| Written authorisation and scope | `not_requested` | S1; everything |
| Original drawing file(s) | `not_requested` | S1; dimensions 1–4 |
| Drawing revisions and change descriptions | `not_requested` | S1; provenance |
| Factory-approved development | `not_requested` | Dimension 7 |
| Factory-approved DXF | `not_requested` | Dimension 10 |
| Joint type and defining surfaces (set-on / set-in) | `not_requested` | Dimensions 4, 5, 7 |
| Rolled plate versus purchased pipe, per component | `not_requested` | Dimensions 7, 8 |
| Plate division and seam decisions | `not_requested` | Dimension 8 |
| Allowances used and their sources | `not_requested` | Dimension 9 |
| Kerf compensation ownership | `not_requested` | Dimensions 9, 10 |
| Known errors or ambiguities and their resolution | `not_requested` | Annotation §11 |
| Fabrication and inspection records | `not_requested` | S5 |
| Annotator (named, qualified) | `not_requested` | S2 |
| Independent fabrication-engineer reviewer | `not_requested` | S3 |
| Independent mathematical reviewer for the branch-intersection model | `not_requested` | S4 item 8 |
| Reviewer-set numerical tolerances for comparison | `not_requested` | Dimensions 4, 7, 10 |

### B. Unavailable · C. Withheld · D. Received unreviewed · E. Reviewed and rejected · F. Disputed

None in any of these categories — **because nothing has been requested or received**, not because everything is in order.

### G. Information absent from the source · H. Unresolved ambiguities

Cannot be assessed: there is no source.

### I. Comparison dimensions

| # | Dimension | Can be compared? |
|---|---|---|
| 1–10 | All | **No** — no truth exists |

### J. Distance to state S4

| Requirement (intake specification §7) | Met? |
|---|---|
| 1 Authorisation | No |
| 2 Provenance | No |
| 3 Engineer-confirmed dimensions | No |
| 4 Geometric interpretation | No |
| 5 Interface constraints and dependency edges reviewed | No |
| 6 Manufacturing reference | No |
| 7 Manufacturing decisions and allowances | No |
| 8 Mathematical model review and independent recomputation | No |
| 9 Known errors and ambiguities | No |
| 10 Independent reviewer sign-off | No |
| 11 Separation-policy compliance statement | Not yet applicable |
| 12 Missing-evidence report with no undefined state | Structure in place; content pending |

**0 of 12 met.**

## Records for this case (to be created from the templates when material arrives)

Provenance record · ground-truth annotation · dependency-review evidence · mathematical model review (Spec 02 §4.4, branch on cylinder) · reviewer sign-off(s) · this report, updated. Confidential content is stored outside this repository unless the authorisation says otherwise.

## Change log

| Date | Change | By |
|---|---|---|
| 18 Nov 2026 | Case prepared at S0 | Project documentation |
