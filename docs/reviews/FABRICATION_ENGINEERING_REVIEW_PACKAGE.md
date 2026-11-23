# Fabrication Engineering Review Package

Dependency model and interface rules · Version 1.1 · 18 November 2026 · Statement identifiers H1–H18, R1–R14, T1–T14, RT1–RT10 are stable and unchanged from v1.0

## 0. What this is — and is not

- A **request for review** by a qualified fabrication engineer (steel pipe and plate fabrication; ideally with development/marking-out and welding experience in the factory concerned).
- Every statement below was drafted from general principles and is at evidence level EV-1, not independently reviewed. **None has been checked by an engineer, against a factory's practice, or against a physical part.** The three examples are symbolic paper exercises, not factory cases.
- **No review has taken place.** The status column is empty on purpose. Nothing in this package may be cited as validated until a named reviewer has completed it.
- Evidence level of every statement: **EV-1, unreviewed proposal** ([Evidence Levels](../architecture/EVIDENCE_LEVELS.md)). A returned review raises a statement to EV-2 only, for the reviewer's stated scope.
- **Language:** this English version is the master. A Japanese version must be translated and then independently checked by a Japanese-speaking fabrication engineer before use at the factory; **no Japanese version exists yet.**
- The software design does not need to be understood to review this. Each statement is a plain claim about fabrication: *if X changes or is wrong, Y must be re-checked*, or *for the parts to fit, Z must be true*.

Source documents (for the project team): [Dependency Model](../architecture/amoeba/DEPENDENCY_MODEL.md), [Spec 02 §4 and §6](../specifications/02_MATHEMATICAL_ENGINEERING_SPECIFICATION.md).

## 1. How to respond

For each numbered statement give one classification and, where useful, a comment:

| Code | Meaning |
|---|---|
| **U** | Valid universal engineering relationship (true for any shop making this kind of part) |
| **P** | Process-specific (true for a particular process — e.g. rolled from plate but not for purchased pipe; laser but not gas cutting) — say which |
| **F** | Factory- or customer-specific rule (a convention or standard of this shop or client) — say what the rule is |
| **C** | Depends on a value that must be measured or calibrated (allowance, factor, clearance) — say how it is obtained today |
| **X** | Wrong or misleading — please correct |
| **?** | Cannot say / depends on something not stated — say what |

Also wanted throughout: **what is missing** — relationships an experienced engineer would check that are not listed. Missing relationships are the most valuable finding.

### Who should review

A fabrication engineer with practical experience in: steel pipe fabrication; branch intersections and reducers; plate development and division; welding and assembly interfaces; interpretation of manufacturing drawings. Preferably **independent of whoever authored these statements** (they are at evidence level EV-1 and have not been independently reviewed; the reviewer should not be someone who edited them).

### Order of work

1. Classify the statements in sections 2–5 **from your own knowledge first**.
2. Only then look at Annex A (proposed consequences and severities) and correct it. Annex A is an aid for prioritising at evidence level EV-1, not independently reviewed; it contains no suggested classifications, and it should not influence step 1.
3. Add missing relationships (section 6a).
4. Complete the reviewer record and return the document.

### What to record for every statement you review

| Field | Content |
|---|---|
| Classification | U / P / F / C / X / ? |
| Evidence or reasoning | Why — experience, a procedure, a standard you can name, a past job, a calculation. "I have always done it this way" is acceptable and should be written as such |
| Scope | For P: which process. For F: which factory or customer. For C: which value, and how it is obtained today |
| Suggested correction | For X or partly-right statements: the corrected wording |
| Confidence | Certain / fairly sure / unsure |

A statement counts as reviewed only when classification **and** reasoning are filled in by the named reviewer. Unanswered statements stay unreviewed; partial review is welcome.

### Reviewer record (to be completed by the reviewer)

| Name | Role and qualifications | Relevant experience (years; pipe / branches / reducers / plate development / welding / drawing interpretation) | Factory or organisation | Independent of the statements' author? | Review date(s) | Sections reviewed | Signature |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

---

## 2. Priority 1 — Header with an oblique branch

Situation: a cylindrical header with a branch pipe joined at an angle, possibly offset from the header centreline; a second branch and an end flange on the same header.

| # | Statement | Class | Comment |
|---|---|---|---|
| H1 | The end-cut of the branch and the opening in the header are two views of the same curve: the intersection of one chosen surface of the branch wall with one chosen surface of the header wall | | |
| H2 | For a **set-on** branch, the curve is defined by the branch's *inside* surface meeting the header's *outside* surface | | |
| H3 | For a **set-in** (stub-in) branch, the header opening is defined by the branch's *outside* surface (plus a clearance) passing through the header wall | | |
| H4 | The opening in the header for a set-on branch matches the branch bore | | |
| H5 | If the header diameter is re-interpreted (inside ↔ outside), all of these must be re-made or re-checked: branch end-cut; header opening; header shell developed width; position of the opening on the shell pattern; weld preparation along the joint | | |
| H6 | The same change requires re-checking **every other branch** on that header, even though their own dimensions did not change | | |
| H7 | The same change requires re-checking parts that mate with the header: end flange or cap bore, reinforcing pads or saddles, supports | | |
| H8 | If one diameter on a drawing was found to be inside rather than outside, every other diameter on that drawing should be re-confirmed, because the same convention probably applies | | |
| H9 | Changing the branch angle or offset changes the branch end-cut, the header opening and the weld preparation, but not the header shell width | | |
| H10 | Changing the branch wall thickness changes the end-cut curve (through the surface used) and the root gap/preparation, but not the header opening for a set-on joint | | |
| H11 | The weld preparation angle varies around the joint because the angle between the two walls varies; on oblique branches the preparation at the acute (heel) side differs in kind from the obtuse (toe) side | | |
| H12 | The header's longitudinal seam must be kept a minimum distance from any branch opening (*what distance, by what rule?*) | | |
| H13 | Adjacent openings must be a minimum distance apart (*rule?*) | | |
| H14 | When the header is rolled from plate, its flat length is the circumference at the neutral (approximately mean) diameter, less any gap, plus/minus allowances | | |
| H15 | When the header is **purchased pipe**, no shell pattern exists; the opening is marked with a wrap-around template made for the *outside* surface | | |
| H16 | A branch template that wraps around the outside of the branch pipe must be developed on the outside circumference even though the fit curve is defined on the inside surface | | |
| H17 | The branch's overall length (e.g. to its flange face) depends on the header surface used and must be re-checked with H5 | | |
| H18 | When the branch diameter approaches the header diameter, the joint changes character near the sides of the header (the curve becomes very steep) and needs special treatment | | |

Questions: Which surfaces do you actually use for H2/H3? Who decides set-on versus set-in, and is it marked on the drawing? Are openings cut before or after rolling, and does that change the pattern? Which of these relationships have caused real errors in your experience?

## 3. Priority 2 — Reducer in halves, seams and mating joints

Situation: a concentric or eccentric reducer made from plate in two (or more) pieces, welded along longitudinal seams and to a pipe at each end.

| # | Statement | Class | Comment |
|---|---|---|---|
| R1 | A reducer between two parallel circular ends, joined by straight lines between corresponding points, can be developed without stretching the plate — including the eccentric case | | |
| R2 | When a reducer is divided into pieces, the two edges that will be welded together must have exactly equal developed length | | |
| R3 | If the development of one piece is found wrong, the other piece cut from the same development must be treated as wrong too | | |
| R4 | The developed length of each end arc must equal the circumference of the mating pipe end, measured on corresponding surfaces | | |
| R5 | When the reducer and the pipe have different wall thickness, a rule decides which surfaces are aligned (inside flush, outside flush, or centred) and that rule changes the reducer's diameters | | |
| R6 | Reducer patterns are developed on the neutral (approximately mean) surface of the plate, with the wall thickness measured square to the sloping wall | | |
| R7 | For an eccentric reducer, the orientation of the flat side (e.g. flat on top / flat on bottom) is a design requirement that must be carried to the pattern and to seam placement | | |
| R8 | Seams are placed by rule (*on the symmetry plane? away from the flat side? away from the bottom?*) | | |
| R9 | The number of pieces is decided by available plate size and by rolling/pressing capacity | | |
| R10 | A reducer formed on a press brake in a series of hits is faceted, not a true cone; the bend lines must be marked on the pattern, and the number of hits affects how well the ends fit a round pipe | | |
| R11 | Extra material is left at the ends and trimmed after forming (*is this done, how much?*) | | |
| R12 | Weld shrinkage at longitudinal seams reduces the finished circumference and is compensated in the pattern (*how?*) | | |
| R13 | Changing the plate thickness requires re-developing the patterns, not merely re-labelling them | | |
| R14 | Very shallow reducers (nearly cylindrical) and very steep ones need different handling in marking out and forming | | |

Questions: How are reducers actually formed in the factory (cone rolls, press brake, both)? What accuracy of end circumference is expected before fit-up? What goes wrong most often?

## 4. Priority 3 — Rectangular-to-round transition and frame interfaces

Situation: a transition piece between a rectangular flange frame and a round duct or collar, made from plate.

| # | Statement | Class | Comment |
|---|---|---|---|
| T1 | The intended shape is four flat triangular faces (one on each side of the rectangle) joined by four conical corner pieces, each fanning out from a corner of the rectangle to a quarter of the circle | | |
| T2 | That shape can be developed without stretching the plate | | |
| T3 | In practice the corners are formed by a number of press-brake bends along lines radiating from each rectangle corner; the finished corner is faceted | | |
| T4 | The number of bends per corner is chosen by rule or experience (*how?*) and determines how round the round end is | | |
| T5 | Bend lines must be marked on the pattern (and are needed in the DXF on a non-cutting layer) | | |
| T6 | The rectangular end's dimensions on the drawing are inside or outside the plate; which one determines how it fits the frame | | |
| T7 | The way the transition meets the frame (inside the frame, butted to it, lapped over it) changes the rectangular end's developed size | | |
| T8 | The round end's developed length must equal the circumference of the mating duct/collar on corresponding surfaces | | |
| T9 | Seams are placed on the centreline of a flat face; the piece is made in one, two or four parts depending on size | | |
| T10 | Matching seam edges must have equal developed length | | |
| T11 | Changing the plate thickness changes: bend allowances, the corner at the rectangular end, fit to the frame, fit to the duct, weld preparation, and whether the available machines can form it | | |
| T12 | A sharp corner at the rectangular end exists only in theory; in practice a small radius or relief is formed there, which affects the fit to the frame | | |
| T13 | If the circle is offset from the centre of the rectangle, all four corners are different and each must be developed separately | | |
| T14 | If the two end planes are not parallel, the simple construction of T1 no longer applies | | |

Questions: Do your designers draw transitions this way, or with a different intended surface? Are they ever rolled rather than press-braked? Is the pattern checked by a trial piece?

## 5. Proposed "if this, then check that" rules

These drive what the system would insist on re-checking before release. For each: is the consequence right, is anything missing, is it always required or only sometimes?

| # | If… | …then these must be re-checked before anything is released | Class | Comment |
|---|---|---|---|---|
| RT1 | A dimension is corrected or re-interpreted | Everything calculated from it, down to the cutting file | | |
| RT2 | One of two intersecting parts changes | The intersection curve, and the other part's cut or opening | | |
| RT3 | Anything changes on one side of a joint | The fit of the part on the other side | | |
| RT4 | A pattern changes | The length and matching points of every seam edge it shares | | |
| RT5 | Thickness, material or a forming/cutting allowance changes or is outside its proven range | Every pattern, bend and weld preparation using it; whether the plate still fits the stock and the machines | | |
| RT6 | A dimension was misread from the drawing | Other dimensions read from the same poor-quality area | | |
| RT7 | A drawing-wide convention is found to be different (inside/outside, units, projection method, scale) | Every dimension interpreted under that convention | | |
| RT8 | A calculation method is found faulty | Every part calculated by that method, including jobs already released | | |
| RT9 | Two statements of the same dimension disagree | Both, and whatever depends on either | | |
| RT10 | A drawing revision changes one part | The parts it mates with; whether the revision note applies to other parts too | | |

## 6. General questions

1. Are dimensions on your customers' drawings normally to the inside, outside or centre of the wall? Does it vary by customer or by part type?
2. Which side of the plate are patterns marked on, and how is the "hand" (mirror image) of a part controlled?
3. Who applies kerf compensation — the person preparing the DXF or the cutting machine/CAM?
4. What tolerances apply to cut patterns, to formed diameters and to fit-up gaps?
5. Which checks do your engineers always perform before releasing a development to cutting?
6. Can you describe past errors in developments or cutting files: what was wrong, how it was found, what it cost? (These would also be used, with permission, as benchmark cases.)
7. Which joint types and part families are missing from this package?

## 6a. Missing relationships (to be added by the reviewer)

| New ID (N1, N2, …) | Part family | Relationship in your words ("if … then … must be checked" or "for … to fit, … must hold") | Classification | Consequence if ignored | Severity | Reasoning / example |
|---|---|---|---|---|---|---|
| | | | | | | |

## 7. Status tracker

| Section | Statements | Reviewed | U | P | F | C | X | ? | Missing items added |
|---|---|---|---|---|---|---|---|---|---|
| 2 Header and branch | H1–H18 | 0 | – | – | – | – | – | – | – |
| 3 Reducer | R1–R14 | 0 | – | – | – | – | – | – | – |
| 4 Transition | T1–T14 | 0 | – | – | – | – | – | – | – |
| 5 Rules | RT1–RT10 | 0 | – | – | – | – | – | – | – |

**Current state: 0 of 56 statements reviewed. All are provisional (EV-1).** No row of this tracker may be changed without a returned review bearing the reviewer's record.

## 8. What happens with the answers

- **U** → kept as a general rule of the dependency model.
- **P** → kept with an explicit process condition.
- **F** → moved to factory/customer configuration with the reviewer's rule recorded and attributed.
- **C** → becomes a calibration requirement; no default value is built in.
- **X** → corrected or removed; the correction is recorded with attribution.
- **?** and missing items → open questions in the Decision Register.

The completed package, with the reviewer's identity and date, is the evidence that a rule was reviewed. A second factory's review may classify the same statement differently; both are kept.

---

## Annex A — Proposed engineering consequence, severity and downstream impact

**Read only after classifying the statements.** At evidence level EV-1, not independently reviewed, and itself subject to review: please correct the consequence, the severity and the impact wherever they are wrong. The annex answers: *if the software lacked this relationship, or applied it wrongly, what could happen?* It is used to prioritise review by manufacturing risk rather than by count.

Proposed severity scale: **Critical** — parts likely scrapped or unable to be assembled, or a defect could leave the factory. **Major** — rework, re-cutting or significant fit-up correction. **Minor** — inconvenience, small adjustment at fit-up, or documentation only.

### Header with branch

| ID | Consequence if the relationship is wrong or missing | Proposed severity | Downstream manufacturing impact | Reviewer's severity | Reviewer's comment |
|---|---|---|---|---|---|
| H1 | Branch cut and header opening made from different curves; they do not match | Critical | Gap or interference all round the joint; re-cut branch or re-open header | | |
| H2 | Wrong surfaces used for a set-on joint; saddle profile too deep or too shallow by about a wall thickness | Critical | Poor root fit; excess gap at crotch or flanks; weld defects or re-cut | | |
| H3 | Opening for a set-in joint too small or too large | Major | Branch will not enter, or excessive gap to fill | | |
| H4 | Opening does not match the branch bore | Major | Flow obstruction or exposed edge; re-cut opening | | |
| H5 | Header diameter interpretation changed but branch cut, opening or shell width not updated | Critical | Several parts cut wrongly from one mistake | | |
| H6 | Second branch on the same header not re-checked | Critical | A branch nobody looked at does not fit | | |
| H7 | Flange, cap, pad or support not re-checked after a header change | Major | Mating part does not fit at assembly; often discovered late | | |
| H8 | The same inside/outside mistake repeated on other diameters | Critical | Systematic error across the whole drawing | | |
| H9 | Angle or offset changed without updating cut, opening and weld preparation | Critical | Branch at wrong angle or position, or does not seat | | |
| H10 | Branch thickness change not carried into the cut curve and preparation | Major | Incorrect root gap; weld quality | | |
| H11 | Uniform weld preparation used where it should vary | Major | Incomplete penetration or excessive weld volume at heel/toe | | |
| H12 | Longitudinal seam passes through or too near an opening | Major | Weld intersection; possible non-compliance with the applicable rule; re-layout | | |
| H13 | Openings too close together | Major | Insufficient ligament; distortion; possible non-compliance | | |
| H14 | Wrong developed length for a rolled header | Critical | Shell diameter wrong; nothing mating to it fits | | |
| H15 | Shell pattern produced for purchased pipe, or template made on the wrong surface | Major | Wasted work; opening marked wrongly | | |
| H16 | Wrap-around template developed on the wrong circumference | Major | Cut line displaced around the pipe; profile does not fit | | |
| H17 | Branch overall length not re-checked | Major | Flange face position wrong; site fit problem | | |
| H18 | Near-equal diameters treated as an ordinary case | Major | Unworkable knife-edge at the flanks; poor fit and weld | | |

### Reducer

| ID | Consequence if the relationship is wrong or missing | Proposed severity | Downstream manufacturing impact | Reviewer's severity | Reviewer's comment |
|---|---|---|---|---|---|
| R1 | Eccentric reducer treated as needing approximation (or the reverse) | Major | Unnecessary error, or false confidence in the pattern | | |
| R2 | Seam edges of unequal length | Critical | Pieces cannot be welded without step or gap; ends do not close | | |
| R3 | Only one half corrected after a fault | Critical | Second half scrapped or mismatched | | |
| R4 | End arc length does not match the pipe | Critical | Step or gap at the girth joint | | |
| R5 | Wrong alignment rule for unequal thickness | Major | Internal or external step; possible rejection | | |
| R6 | Pattern developed on the wrong surface | Critical | Finished diameters off by a multiple of the thickness | | |
| R7 | Flat-side orientation lost | Critical | Reducer installed the wrong way up, or seam in a forbidden position | | |
| R8 | Seam placed against the rule | Major | Rework or rejection | | |
| R9 | Piece does not fit the plate or the machine | Major | Re-division after cutting has been planned | | |
| R10 | Faceting ignored | Major | Ends not round enough to fit; extra fit-up work | | |
| R11 | Trim allowance omitted or double-counted | Major | Reducer short, or wasted material | | |
| R12 | Shrinkage ignored or wrongly applied | Minor to Major | Circumference slightly off; depends on diameter and tolerance | | |
| R13 | Old pattern reused at a new thickness | Critical | Diameters and fit wrong | | |
| R14 | Extreme proportions handled by the standard method | Major | Inaccurate marking; forming difficulty | | |

### Transition

| ID | Consequence if the relationship is wrong or missing | Proposed severity | Downstream manufacturing impact | Reviewer's severity | Reviewer's comment |
|---|---|---|---|---|---|
| T1 | Intended shape misunderstood | Critical | Whole pattern wrong | | |
| T2 | Treated as approximate when exact (or the reverse) | Minor | Unnecessary warnings, or unjustified confidence | | |
| T3 | Forming method not reflected in the pattern | Major | No bend lines; shop improvises | | |
| T4 | Too few bends | Major | Round end does not fit the collar | | |
| T5 | Bend lines missing or placed on a cutting layer | Critical | Bend lines cut by the machine, or part cannot be formed accurately | | |
| T6 | Inside/outside of the rectangular end misread | Critical | Does not fit the frame by twice the thickness | | |
| T7 | Wrong joint arrangement with the frame | Major | Re-cut or re-make frame | | |
| T8 | Round end circumference wrong | Critical | Does not fit the duct | | |
| T9 | Unsuitable seam position or piece count | Major | Forming or welding difficulty | | |
| T10 | Seam edges of unequal length | Critical | Pieces do not close | | |
| T11 | Thickness change only partly carried through | Critical | Several misfits from one revision | | |
| T12 | Corner radius or relief ignored | Minor to Major | Fit to frame at the corners; cracking risk | | |
| T13 | Offset transition developed as symmetric | Critical | Wrong shape | | |
| T14 | Non-parallel ends developed by the simple method | Critical | Wrong shape | | |

### Rules

| ID | Consequence if the rule is wrong or missing | Proposed severity | Downstream manufacturing impact | Reviewer's severity | Reviewer's comment |
|---|---|---|---|---|---|
| RT1 | Stale results released after a correction | Critical | Cutting file inconsistent with the corrected drawing | | |
| RT2 | Partner part's cut not updated | Critical | Intersecting parts do not fit | | |
| RT3 | Mating part not checked | Critical | Misfit found at assembly | | |
| RT4 | Seam mismatch | Critical | Pieces do not close | | |
| RT5 | Allowance used outside its proven range | Major to Critical | Dimensions off; cracking or forming failure | | |
| RT6 | Neighbouring misreads not found | Critical | More wrong dimensions from the same cause | | |
| RT7 | Convention error not propagated | Critical | Systematic error across the drawing | | |
| RT8 | Faulty method's other results not recalled | Critical | Defective parts in other jobs | | |
| RT9 | Conflict resolved on one side only | Major | Hidden inconsistency | | |
| RT10 | Revision not carried to mating parts | Critical | Old and new parts do not fit | | |
