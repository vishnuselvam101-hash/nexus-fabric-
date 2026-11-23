# Wall thickness in the header–branch joint — design

**DESIGN — awaiting the owner's decision. No code is written until the owner approves this design.**

**Development build. Not validated. Outputs must not be used for manufacturing.**

Date: 21 Nov 2026 · Work plan task **1.7** (set-on joint, remaining case, RB-001) in `docs/project/CLI_WORK_PLAN.md`;
of phase 5 item 1 only the wall thickness and neutral surface of the header–branch joint are designed here, and bend
allowance, weld gap and kerf compensation are not (section 7) ·
Starting point: engine `nf-engine 0.4.2` (`crates/nf-core/src/lib.rs:57`), branch `claude/nexus-fabric-strategy-8guhxz`.

Nothing in this document is implemented. Today wall thickness, the neutral factor, bend allowance, weld gap and kerf
are not modelled (`crates/uem/src/develop.rs:1-3`, `:17`; `crates/nf-core/src/job.rs:861`), and that stays true until
the owner decides otherwise. The formulas below are derivations written for review, at the evidence level of
Specification 02 (EV-1: not independently reviewed). Every number here is taken from the code (file:line) or from
Specification 02 (section and line), or is written as an open decision for the owner or a question for a fabrication
engineer. The only exceptions are three illustrations computed for this document from cited inputs (sections 0.1 and
3.1), each labelled as such: they are not product outputs, not dimensions and not acceptance values. Line numbers of
files refer to commit `b032c7d`. Nothing in this design needs a network call, telemetry, a CDN, a cloud service, an
update service or any AI: every input is typed by a person or recorded as a fabrication value, and every computation is
local and deterministic.

Short names: `develop.rs` = `crates/uem/src/develop.rs`; `verify.rs` = `crates/uem/src/verify.rs`; `job.rs` =
`crates/nf-core/src/job.rs`; "Spec 02" = `docs/specifications/02_MATHEMATICAL_ENGINEERING_SPECIFICATION.md`;
"FAB package" = `docs/reviews/FABRICATION_ENGINEERING_REVIEW_PACKAGE.md` (its statements H1–H18 are hypotheses that no
fabrication engineer has reviewed yet); "regression 004" = `crates/nf-core/tests/regressions/004-set-on-joint-bore-on-header-outside.json`,
cases numbered 1–6 in file order.

---

## 0. Summary

### 0.1 For the owner, in plain words

Today the product treats each wall as a surface with no thickness. This design says what would have to be entered,
computed and asked of a fabrication engineer to take the wall into account, and what would change. Nothing is built
until the owner decides. Section 8 gives every decision with a recommendation.

**Decide now:**

1. **D8 — a finding about today's set-on output.** With a real wall, the set-on branch end that today's file draws
   (cut square on the curve of its bore) sits on the header without entering the header wall only for a branch at
   exactly 90° on the header centre line. For a leaning or offset branch, part of the end lies inside the header wall,
   and nothing warns the user. Illustration, computed for this document from cited inputs (not a product output, not
   independently reviewed): the leaning joint of regression 004 case 2 (β = 60°, δ = 20 mm, header outside radius
   161.95 mm, branch bore radius 77.095 mm) with the DN150 wall t = 7.11 mm of REVIEW_B:135 (the radii in
   `verify.rs:1531-1532` imply 7.055 mm; with that value both numbers below are about 0.03 mm smaller) enters the
   header wall by t·cot 60° = 4.105 mm along the branch axis on the generator on the side of the lean (φ = 270°, exact
   by section 2.4), and by 4.147 mm at most, near φ = 265.28° (largest value of S(φ; ρ, R_o) − S(φ; r_i, R_o) over
   the wall, found by golden-section search on S over r_i ≤ ρ ≤ r_i + t with φ sampled every 0.01°, in binary64; to be
   reproduced by the stage-B reference, section 5). Recommendation: record it in the defect register now and add a
   text advisory to those results now; no cut line changes. The register row and the advisory state the amount only
   symbolically (t·|cot β| on the generator on the side of the lean); the millimetre figures above are not copied
   anywhere until the stage-B reference has reproduced them (section 5, item 2).
2. **D1** — approve stages A and B of section 9, which change no cut line; hold stages C and D.
3. **D3** — keep entering the development radius, add the wall thickness, and check the implied neutral factor
   exactly (section 3.1).
4. **D4, D6, D9, D10** — field names; no thickness for `branch_stub`; no template output yet; the second edge of a
   non-square cut given as a table, not as drawn geometry.
5. **D5, D7** — the fit-up verdict in gate G06, INCONCLUSIVE; deploy the stages to the live server in as few
   deployments as possible, because each deployment marks every stored result for recomputation (section 4.1).

**Decide after a fabrication engineer answers (section 6):** D2, which set-on end models are offered (S2, S3; stage
C), after T1, T2 and T11; set-in and set-through (stage D) after T3; and every value only an engineer can give (k,
fit-up acceptance, clearance, kerf: T4, T6, T3, T8). Until then the gates that need them stay INCONCLUSIVE.

### 0.2 Technical summary

1. **Why today's set-on works.** The set-on of Spec 02 §4.4 (branch inside surface on header outside surface) needs
   one curve for both parts, and that curve is defined by two entered radii: the header outside radius and the branch
   bore. No wall thickness is needed (section 1.5).
2. **The wall normal is radial; today's formulas are a special case.** Between coaxial cylinders the wall normal is
   radial, so moving a point of a contact curve along it to the development surface keeps its branch coordinates
   (φ, s) and its header coordinates (x, ω) (section 2.2). Under assumption N (rolling keeps the length of the neutral
   surface and the normals of the wall; section 2.3, question T9) the pattern point is then (r_n·φ, s) on the branch
   and (x, R_n·(ω + π)) on the header: mapping (M). Today's formulas (`develop.rs:871`, `:1064-1067`) coincide with (M)
   if the entered development radius equals r_n; the code neither computes nor checks r_n. A wall changes which radii
   the curves are computed for, where the development radius comes from, and whether the two parts of a joint share
   one curve; it does not need a new chart.
3. **A square-cut end has a width.** The cut end of the branch spans the wall. With a square (radial) cut, the end
   rests on the header with one fibre of the wall at each azimuth (every fibre at once in one degenerate case), and
   which fibre that is follows from the sign of ∂S/∂ρ. For the centred perpendicular tee (β = 90°, δ = 0) the bore
   fibre governs everywhere, which is a derived reason for the Spec 02 §4.4 pairing. **For every other accepted
   geometry (β ≠ 90° or δ ≠ 0) there are azimuths where today's bore-cut set-on end would lie inside the header
   wall**; on the generator on the side of the lean it is t·|cot β| along the branch axis. Today this is not
   detected, because t is not an input (section 2.4).
4. **The RB-001 open case is not a square cut.** A saddle on the branch OUTSIDE surface over an opening the size of the
   bore cannot be fitted with a square cut: at the flanks of a tee the inner part of the end would enter the header
   wall. As "two curves" it is an end fitted to the header surface, whose cut through the wall leans by an angle that
   changes round the saddle. What the shop means by it is a question for a fabrication engineer (section 2.5, T1).
5. What a thickness model needs (section 3), what it changes and how old requests keep their exact behaviour (section
   4), how it is checked (section 5), the questions for a fabrication engineer (section 6), what stays out (section 7),
   the owner's decisions (section 8) and a staged order of work (section 9).

---

## 1. The current model, exactly

### 1.1 Zero-thickness reference surfaces

- "All radii are *reference-surface* radii supplied by the caller: thickness and process allowances are NOT modelled
  in this build" (`develop.rs:1-3`). Every pattern carries the assumption `ASSUME_REF`: "Radii are taken as
  reference-surface (neutral) radii exactly as entered; thickness, neutral-factor, weld gap, shrinkage and kerf are not
  modelled" (`develop.rs:17`).
- Every cut line is "the finished part edge on the development surface"; kerf is not compensated and no bevel, root
  gap or weld allowance is included (`KERF_NOTE`, `develop.rs:158`). The branch end and the header opening are cut
  "square through the wall (constant height through the thickness, as a cut perpendicular to the flat plate gives)"
  (`develop.rs:955`, `:1170`).
- Every pattern is drawn from the outside of the formed part (`MARKING_SIDE`, `crates/uem/src/lib.rs:126`); the plate
  is rolled so that the drawn face becomes the outside (`develop.rs:921`, `:1133`); the header is "rolled from plate"
  (`develop.rs:1169`).
- Gate G06 is INCONCLUSIVE: "Radii are used as reference-surface radii exactly as entered; thickness, allowances, weld
  gap and kerf are not modelled" (`job.rs:856-862`).
- A request field named `thickness` is refused by name, so that nobody believes it was applied (BE-016; `job.rs:204-206`,
  `:242-260`).

So each part is a surface of zero thickness for the intersection (the **contact radius**) plus a second, separately
entered radius for the development (the **development radius**). The engine never derives one from the other.

### 1.2 Request fields per family and how the radii enter

| Family | Radius fields | Surface attribute | How the radii enter the pattern |
|---|---|---|---|
| `cylinder_shell` | `r_dev` (`job.rs:38-41`) | none (RB-010, task 1.4) | developed width 2π·r_dev (`develop.rs:181`) |
| `cone_frustum` | `r1`, `r2` (`job.rs:42-46`) | none | taken as reference radii at the end planes (`develop.rs:237-239`) |
| `eccentric_reducer` | `r1`, `r2`, `e` (`job.rs:47-52`) | none | reference radii; "a uniform reference-surface offset is a simplification (Spec 02 research item R-05)" (`develop.rs:610`) |
| `branch_stub` | `header_radius`, `branch_radius` (contact), `branch_dev_radius` (`job.rs:53-59`) | one `surface_reference` for both contact radii, optional, absent = `unknown` (`job.rs:60-63`) | saddle s(φ) from the two contact radii (`develop.rs:630-634`), drawn at (r_dev·φ, s(φ)) (`develop.rs:871`), width 2π·r_dev (`develop.rs:888`) |
| `header_with_branch` | `header_radius`, `header_dev_radius`, `branch_radius`, `branch_dev_radius` (`job.rs:66-75`) | shared `surface_reference` and per part `header_surface_reference`, `branch_surface_reference` (`job.rs:76-94`), a part without its own takes the shared one (`job.rs:1624-1635`); `joint` (`job.rs:95-101`) | ONE geometry value built from the two contact radii serves both parts (`job.rs:1213-1226`): the saddle as for `branch_stub`; the opening ω = asin((δ + r·cos φ)/R), x = station + s·cos β − r·sin φ·sin β, drawn at (x, R_dev·(ω + π)) (`develop.rs:1057-1067`) |

The contact radii are the radii of Spec 02 eq. (4.3) (Spec 02 §4.4, lines 392-395; `Branch`, "Header contact radius",
"Branch contact radius", `develop.rs:616-627`). The development radii only scale the circumferential coordinate of the
patterns (Spec 02 §4.4, line 397). Spec 02 §4.1 (line 334) defines the contact radius as "the surface of the wall that
defines the cut — inside, outside or reference; an engineering decision linked to weld preparation".

### 1.3 `surface_reference` and the BE-001 rule

Values: `outside`, `inside`, `neutral`, `unknown` (`job.rs:23`). `radius_consistency` (`job.rs:1375-1459`) applies
Spec 02 §6.1 (lines 481-483: r_n = r_i + k·t with 0 < k ≤ 0.5) and r_o = r_i + t, hence r_i ≤ r_dev ≤ r_o
(`job.rs:1378-1381`). It is applied to each part of the assembly against its own surface (`job.rs:1150-1170`) and to
the branch stub (`job.rs:995-1043`).

| Declared surface of the contact radius c | Refused (code) | Accepted with a note | Accepted silently |
|---|---|---|---|
| `outside` | r_dev > c: `radius_dev_exceeds_outside` (`job.rs:1429-1434`) | r_dev = c, "zero thickness" (`job.rs:1428`); 2·r_dev ≤ c: B2-001 advisory "mid-thickness (k = 0.5) is impossible" (`job.rs:1435-1438`) | c/2 < r_dev < c (`job.rs:1439`) |
| `inside` | r_dev < c: `radius_dev_below_inside` (`job.rs:1441-1446`) | r_dev = c (`job.rs:1440`) | every r_dev > c, with no upper bound (`job.rs:1447`; REVIEW_B2:148 says the bound needs r_o or t) |
| `neutral` | r_dev ≠ c beyond 1e-12 relative: `radius_neutral_mismatch` (`job.rs:1449-1454`) | — | r_dev = c (`job.rs:1448`) |
| `unknown` | — | always: "could not be checked" (`job.rs:1455-1457`) | — |

"Equal" means within 1e-12 relative, "binary64 noise only; this is not an engineering tolerance" (`job.rs:1371-1373`).
No thickness, k or thickness range is assumed (`job.rs:1392-1396`).

### 1.4 The joint today

- `joint` is absent or `"set_on"` (`job.rs:1113-1149`). `set_on` requires header `outside` and branch `inside`
  (`SET_ON_HEADER_SURFACE`, `SET_ON_BRANCH_SURFACE`, `job.rs:1072-1073`, `:1116`); otherwise `joint_pairing_invalid`,
  whose message ends "this build has no wall thickness from which the bore could be derived" (`job.rs:1117-1134`). Any
  other value, for example `"set_in"`, is `joint_type_unknown` (`job.rs:1136-1147`; regression 004 case 5).
- Undeclared joints compute the same single curve and are labelled "not declared (the branch end cut and the header
  opening are cut on one curve of the entered radii)" (`job.rs:1259-1263`), with a G02 note calling it a zero-thickness
  idealisation (`job.rs:1360-1362`).
- The DXF states the joint with the keys `NF-JOINT` (`set-on` or `not declared`), `NF-HEADER-SURFACE`,
  `NF-BRANCH-SURFACE` and two TEXT lines (`crates/nf-dxf/src/lib.rs:47-58`, `:69-115`; `docs/backend/DXF_METADATA.md:14-31`).
  A file that states any other `NF-` key fails the quality gate (`DXF_METADATA.md:27`).
- The interface checks take the header geometry and the branch geometry as separate arguments (`verify.rs:938-956`,
  `:1009-1025`), but the job passes the same value twice (`job.rs:1302-1321`). Both checks require the folded opening
  and the folded saddle to be one curve (`verify.rs:984-1006`, `:1074-1089`).
- **Unverified convention boundary (external review C, RC-16, `REVIEW_C_EXTERNAL.md:35`).** The fold-back functions
  (`fold_header`, `fold_branch`) and `Joint3` share the frame of Spec 02 §4.4. The interface checks therefore confirm
  that the two parts agree WITH EACH OTHER in that frame, not that the frame itself matches the physical forming
  convention: a convention error common to both sides passes every check. Likewise a mirrored part of a
  mirror-symmetric joint (the centred perpendicular tee) is the same part, so no interface check can name that mirror.
  This boundary is stated as UNVERIFIED until a fabrication engineer confirms the frame and the marking-side
  convention (section 6; the marking side is already declared not reviewed, `uem/src/lib.rs`, `MarkingSide`). Passing
  the same geometry as `hb` and `bb` is correct for today's zero-thickness single-curve model (1.5); once thickness
  exists the two geometries are derived separately per part and stay distinct (4.3).

### 1.5 Which set-on pairing works today, and why the other does not

- **Works: branch INSIDE surface on header OUTSIDE surface** (Spec 02 §4.4 parameters, line 398; FAB package H2 and H4,
  lines 69 and 71). The saddle and the opening are then the same curve, the trace of the branch bore on the header
  outside surface. It needs exactly the four entered radii: `header_radius` = header outside radius,
  `branch_radius` = branch bore, and the two development radii. No other surface of either wall enters, so no thickness
  is needed. Regression 004 cases 1 and 2 (the DN300/DN150 example of REVIEW_B:135-136) carry `"expect": "verified"`
  in the fixture (not run for this document).
- **Does not work: saddle on the branch OUTSIDE surface, opening = bore** (REVIEW_B:133, the open part of RB-001;
  `docs/backend/DEFECT_REGISTER.md:86`). These are two curves: the saddle needs the branch outside radius, the opening
  needs the bore. The request has one branch contact radius and no thickness, so the second radius cannot be formed. A
  declared `set_on` with a branch surface other than `inside` is refused (`job.rs:1116-1134`; regression 004 case 3).
  Entered as an undeclared joint with both outside diameters, the opening is cut at the branch outside diameter and the
  branch passes through it (REVIEW_B:137).
- **Not seen today:** with a real wall, the bore-cut square end of the working pairing is free of interference at every
  azimuth only for the centred perpendicular tee (section 2.4). The size of the interference needs t.

---

## 2. The geometry with a wall (derivation)

### 2.1 Surfaces and symbols

| Part | Inside surface | Outside surface | Neutral (development) surface of a rolled part |
|---|---|---|---|
| Header | R_i | R_o = R_i + T | R_n = R_i + K·T |
| Branch | r_i | r_o = r_i + t | r_n = r_i + k·t |

T and t are measured along the wall normal; for a circular cylinder that is the radial direction (2.2), so the radial
wall equals the normal wall. K and k follow Spec 02 §6.1 (0 < k ≤ 0.5, k from calibration or test, CAL/PT; line 483).
For a cone the radial wall is t/cos α (Spec 02 §4.2, line 352); cones are not covered here (section 7).

Frame of Spec 02 §4.4 (lines 392-395) and of the code (`develop.rs:630-643`): header axis = x; branch axis through
(0, δ, 0) with direction (cos β, 0, sin β), 0 < β < π; branch azimuth φ; s = distance along the branch axis.

- Branch cylinder of radius ρ: `P(φ, s; ρ) = (s·cos β − ρ·sin φ·sin β, δ + ρ·cos φ, s·sin β + ρ·sin φ·cos β)`
  (`develop.rs:636-643` with r → ρ).
- Header cylinder of radius R′: `H(x, ω; R′) = (x, R′·sin ω, R′·cos ω)`, with ω as in `develop.rs:1063`.
- Height at which the branch cylinder of radius ρ meets the header cylinder of radius R′ on the upper sheet (Spec 02
  eq. 4.3 with r → ρ, R → R′):

  `S(φ; ρ, R′) = [√(R′² − (δ + ρ·cos φ)²) − ρ·sin φ·cos β] / sin β`,

  defined for every φ if and only if |δ| + ρ ≤ R′ (Spec 02 §4.4, line 399: "Full saddle requires |δ| + r ≤ R"); at
  equality S is still defined and only its slope is unbounded. The derivative formulas of section 2.4 need the strict
  inequality |δ| + ρ < R′; the code refuses at |δ| + r ≥ R·(1 − 1e-6) (`develop.rs:829-831`). Today's saddle is
  S(φ; r, R) with the two contact radii.

### 2.2 The wall normal, and the mapping of a contact point to the development surface

**Branch.** ∂P/∂ρ = n_b(φ) = (−sin φ·sin β, cos φ, sin φ·cos β). Then |n_b|² = sin²φ·sin²β + cos²φ + sin²φ·cos²β = 1;
n_b·∂P/∂s = n_b·(cos β, 0, sin β) = 0; n_b·∂P/∂φ = ρ·(sin φ cos φ sin²β − sin φ cos φ + sin φ cos φ cos²β) = 0. So
n_b is the unit outward normal of every branch cylinder at (φ, s), whatever ρ and s are, and the straight line along the
wall normal through P(φ, s; ρ_c) is {P(φ, s; ρ)}: moving along the wall normal changes ρ only.

**Header.** ∂H/∂R′ = n_h(ω) = (0, sin ω, cos ω): unit, perpendicular to ∂H/∂x and to ∂H/∂ω. The line along the header
wall normal keeps (x, ω).

**Development.** Spec 02 §4.1 (line 330): u = r_dev·θ, v = z is an isometry of the reference surface; Spec 02 §6.1
(line 483): "the developed length is the length of the neutral surface". So the neutral-surface point P(φ, s; r_n)
develops to (r_n·φ, s), and H(x, ω; R_n) to (x, R_n·(ω + π)) (the seam opposite the branch, `develop.rs:1066`).

**Result (M), conditional on assumption N (section 2.3) and so on question T9.** The geometric step needs no
assumption: a point of a contact curve with branch coordinates (φ, s) on the branch face of radius ρ_c moves along the
wall normal by |r_n − ρ_c| (k·t outward from the bore; (1 − k)·t inward from the outside surface) to the neutral
surface at the same (φ, s); a header contact point (x, ω) on the face of radius R_c moves by |R_n − R_c| to (x, ω) on
the neutral surface. The step to the pattern uses N: only if the neutral surface keeps its length and the wall normal
stays the normal of the flat plate is the pattern point (r_n·φ, s) on the branch and (x, R_n·(ω + π)) on the header.
Today's formulas (`develop.rs:871`; `develop.rs:1064-1067`) coincide with (M) if the entered development radii equal
r_n and R_n; the code neither computes nor checks r_n or R_n (it checks only that the development radius lies on the
permitted side of the declared contact surface, section 1.3). The verifier's developed-length check already builds the
cut on the development cylinder at the same (φ, s) (`verify.rs:529-545`). Thickness needs no new chart.

### 2.3 What a cut in the flat plate becomes (assumption N)

**Assumption N** (to be confirmed, question T9): when the plate is rolled, the neutral surface keeps its length (Spec
02 §6.1); a line normal to the flat plate becomes the wall normal of the shell; the wall keeps its thickness and the
axial coordinate does not change. Spec 02 §6.3 (line 509) lists "thinning/elongation in heavy rolling" as a process
fact from physical test (PT), so N is an assumption, not a fact. The code already relies on it (`develop.rs:955`).

Under N, the material point at plate coordinates (u, v), at depth ζ from the neutral plane (positive toward the drawn
face, which becomes the outside), lands at φ = u/r_n, s = v, ρ = r_n + ζ. Three consequences:

- **N1 (square cut).** A cut square to the flat plate along one plate curve becomes the radial surface over one
  (φ, s) curve. The cut edge is the same (φ, s) curve on the inside face, the neutral surface and the outside face. **A
  DXF line of a square cut represents all three edges at once; it does not pick an edge.**
- **N2 (non-square cut).** An end whose edges lie at different (φ, s) on the two faces appears on the flat plate as two
  different curves, one on each face, and both are drawn with u = r_n·φ, not with r_o·φ or r_i·φ. The cut leans from
  the plate normal in the v direction only, by atan(Δv / t), where Δv is the difference of the two curves at the same φ.
- **N3 (no rolling).** A part that is not rolled from plate (purchased pipe marked with a wrap-around template; FAB
  package H15 and H16, lines 82-83) is not developed on its neutral surface: the template lies on the surface it is
  wrapped on, (M) holds with that radius in place of r_n, and the template material adds an allowance only a
  fabrication engineer can state (T5). Today every pattern says it is rolled from plate (`develop.rs:921`, `:1133`,
  `:1169`); the entered development radius is not checked against any process.

### 2.4 Which fibre of a square-cut end touches the header

Fix φ, and let the end be cut square at height s_c(φ) (N1). The fibre of the end face at radius ρ, r_i ≤ ρ ≤ r_o, is
the point P(φ, s_c; ρ). Its y = δ + ρ·cos φ does not depend on s and its z grows with s (sin β > 0), so it is outside the
header cylinder of radius R′ if and only if s_c > S(φ; ρ, R′). Define the **axial standoff**

`d(φ, ρ) = s_c(φ) − S(φ; ρ, R′)`: d > 0 is a gap along the branch axis, d = 0 is contact, d < 0 means the fibre lies
inside the header surface (where that is header material: **interference**).

With y = δ + ρ·cos φ and h = √(R′² − y²):

- `∂S/∂ρ = [−y·cos φ / h − sin φ·cos β] / sin β`
- `∂²S/∂ρ² = −R′²·cos²φ / (h³·sin β) ≤ 0`

S is concave in ρ, strictly where cos φ ≠ 0. So at each φ with cos φ ≠ 0 exactly one fibre reaches the header
first: r_i if ∂S/∂ρ(r_i) ≤ 0; r_o if ∂S/∂ρ(r_o) ≥ 0; otherwise the interior fibre ρ* where ∂S/∂ρ = 0 (unique, because
∂S/∂ρ strictly decreases with ρ there). The stationary fibre has a closed form: ∂S/∂ρ = 0 gives
y·cos φ = −h·sin φ·cos β, hence y* = −R′·sign(cos φ)·sin φ·cos β / √(cos²φ + sin²φ·cos²β) and ρ* = (y* − δ)/cos φ; the
governing fibre is ρ* clamped to [r_i, r_o]. For cos φ = 0 (φ = 90° or 270°), ∂S/∂ρ = −sin φ·cos β / sin β does not
depend on ρ and S is linear in ρ: for β ≠ 90° one face governs (r_o where the slope is positive, r_i where it is
negative); for β = 90° S does not depend on ρ at all and every fibre touches at once (the tee below). The closed form
is part of the derivation to be reviewed independently (section 5, item 1).

Special cases:

- **Centred perpendicular tee** (β = 90°, δ = 0): ∂S/∂ρ = −ρ·cos²φ / h ≤ 0 for every φ and ρ. The bore fibre governs
  everywhere; the outside fibre stands off by S(φ; r_i, R′) − S(φ; r_o, R′) ≥ 0, zero at φ = 90° and 270°.
- **Generator on the side of the lean**, in the acute angle between branch and header (φ = 270° for β < 90°,
  φ = 90° for β > 90°): there cos φ = 0, so ∂S/∂ρ = |cot β| > 0 for any δ, and S is linear in ρ. The outside fibre
  governs, and its height exceeds the bore fibre's by exactly t·|cot β|.
- **Offset perpendicular branch** (β = 90°, δ ≠ 0): at the azimuths where cos φ has the sign opposite to δ and
  |cos φ| < |δ|/r_i, ∂S/∂ρ(r_i) = −(δ + r_i·cos φ)·cos φ / h > 0.

**Consequence.** The bore-cut square end s_c = S(φ; r_i, R_o), which is today's set-on curve, is free of interference at
every azimuth **if and only if β = 90° and δ = 0**. For every other accepted set-on geometry, regression 004 case 2
(β = 60°, δ = 20 mm) included, the end enters the header wall next to its bore edge at some azimuths: on the generator
on the side of the lean by t·|cot β| along the branch axis. The amount needs t and is not computed today. For
regression 004 case 2 with t = 7.11 mm the illustration of section 0.1 gives 4.105 mm there and 4.147 mm at most
(decision D8).

**Defect in today's convention text, found while writing this design (not yet in the register; section 9).** Every
branch pattern (both families build it with `develop::branch_stub`, `job.rs:1219`, `:1577`) states "PHI = 90 DEG IS
THE GENERATOR ON THE SIDE AWAY FROM THE LEAN OF THE BRANCH (TOWARD DECREASING STATION FOR BETA BELOW 90 DEG), PHI = 270
DEG THE GENERATOR ON THE SIDE OF THE LEAN" (`develop.rs:924`), without limiting it to β < 90°. For β > 90° the axis
direction (cos β, 0, sin β) points toward decreasing station, so the branch leans that way (`develop.rs:171` states
the case below 90°), and the generator at φ = 90°, offset from the axis by r·(−sin β, 0, cos β)
(`develop.rs:636-643`), is then on the side of the lean. The text is therefore wrong on every branch pattern with
β > 90°. It is not a thickness question; it belongs to task 1.3 (conventions stated in the DXF and the result).

### 2.5 Joint types: surfaces in contact, cut edge, DXF line

Set-on opening (the trace of the bore on the header outside surface; `develop.rs:1057-1067` with r = r_i, R = R_o):
`O_on(φ)`: ω = asin((δ + r_i·cos φ)/R_o), x = station + S(φ; r_i, R_o)·cos β − r_i·sin φ·sin β.

**Set-on S1 — square end on its bore edge (today's curve).**
- Branch cut s_c = S(φ; r_i, R_o). In contact: the bore edge with the header outside surface. The other fibres stand
  off (tee) or interfere (section 2.4).
- Branch DXF line: (r_n·φ, s_c). It is today's line when `branch_dev_radius` = r_n, and it represents all edges (N1).
- Header opening O_on, square through the header wall; DXF line (x, R_n·(ω + π)), today's line. At the header inside
  face the opening lies at the same (x, ω) (N1), which is not on the bore cylinder. For the tee, at φ = 0° and 180°
  (across the header), sin ω = r_i/R_o, so the inside-face edge is at y = r_i·R_i/R_o: a lip r_i·T/R_o wide inside the
  bore; along the header axis there is none. Whether the opening should instead be cut along the branch axis is
  REVIEW_B:70 (item 13) and :121, question T2.

**Set-on S2 — square end resting on its governing fibre (no interference).**
- Branch cut s_c = S*(φ) = max over r_i ≤ ρ ≤ r_o of S(φ; ρ, R_o). In contact at each φ: the governing fibre of
  section 2.4 (bore edge, outside edge or a fibre inside the face). Everything else stands off. For β ≠ 90° or δ ≠ 0
  this is not the trace of any one fibre, and it differs from the opening O_on: two curves.
- Branch DXF line (r_n·φ, S*(φ)); represents all edges (N1). Header as S1, or as T2 decides. Needs t.

**Set-on S3 — end fitted to the header outside surface (the RB-001 open case).**
- The end face lies on the header outside surface: its outside edge at S(φ; r_o, R_o) (the "saddle on the branch
  outside surface") and its bore edge at S(φ; r_i, R_o), which coincides with the opening O_on. Two curves; contact
  over the whole end face.
- The fitted face is curved through the wall (S is concave in ρ). A straight cut between the two edges lies on the
  chord, which for a concave function is below the curve: the middle of the face would enter the header surface by at
  most t²/8 · max |∂²S/∂ρ²| along the axis (the interpolation-error bound of a chord). Small for a thin wall, not zero;
  acceptance is question T6.
- Rolled branch (N2): on the drawn (outside) face the edge is (r_n·φ, S(φ; r_o, R_o)); on the other face
  (r_n·φ, S(φ; r_i, R_o)). The cut leans by atan([S(φ; r_i, R_o) − S(φ; r_o, R_o)] / t) from the plate normal, and the
  sign of the lean changes round the saddle for β ≠ 90° or δ ≠ 0 (section 2.4). A DXF line can carry only one of the
  two edges on the cutting layer; the file must say which. The other edge is given as a table in the result and the
  records (the height of the other edge and the lean at stated azimuths), and the file states where it is (D10). A
  layer `REFERENCE` exists (`crates/uem/src/lib.rs:45-49`, `job.rs:368-374`) and is not a cut layer by
  `is_cut_layer` (`crates/nf-dxf/src/lib.rs:60-63`), but no output emits it (`job.rs:372` is its only use), and the
  DXF quality gate treats every entity that is not an annotation as cut geometry
  (`crates/nf-dxf/src/quality.rs:362-363`) and fails any on a layer other than CUT_OUTER or CUT_INNER (rule
  `wrong_layer`, `quality.rs:446-457`). A file with
  the second edge drawn on `REFERENCE` would therefore be withheld today; drawing it is an owner option (D10) that
  needs the gate, the re-import comparison and the independent reader changed first (stage C). A cutter that cuts
  square cannot produce this end (T11).
- Purchased pipe (N3): the template carries the outside edge at the template radius, and the cut is aimed at the bore
  edge, which is FAB package H16 (line 83).
- **The same saddle cut square (call it S4) cannot be fitted over a bore-sized opening:** at the tee flanks
  (φ = 0°, 180°) the bore fibre's own intersection height is above the cut by S(φ; r_i, R_o) − S(φ; r_o, R_o) > 0, so
  the inner part of the end enters the header wall (section 2.4 with s_c = S(φ; r_o, R_o)). This design does not
  propose S4.

**Set-in — the branch passes through the opening and ends at the header inside surface.**
- Branch end, square and not standing into the header bore: S*_in(φ) = max over ρ of S(φ; ρ, R_i) (section 2.4 with
  R′ = R_i); a fitted variant is S3 on R_i. Needs |δ| + r_o < R_i.
- Opening: it must clear the branch outside surface through the whole header wall. The trace of the branch outside
  cylinder on the header cylinder of radius R′ is T(φ; R′): ω = asin((δ + r_o·cos φ)/R′),
  x = station + S(φ; r_o, R′)·cos β − r_o·sin φ·sin β. A square cut makes one (x, ω) curve on both faces (N1), so the
  zero-clearance square opening is the boundary of the union, over R_i ≤ R′ ≤ R_o, of the regions enclosed by T(·; R′).
  For the tee the region is (x − station)² + R′²·sin²ω ≤ r_o², which grows as R′ falls: the union is the trace on R_i,
  and at the outside face the gap across the header is r_o·T/R_i (none along the header axis). For β ≠ 90°, x depends
  on R′; for δ ≠ 0, the ω-range of the region at a given x moves as R′ changes instead of only widening (with β = 90°,
  x = station − r_o·sin φ for every R′). In either case the regions are not nested in general, and the union must be
  computed by a new algorithm with its own error bound; it is not approximated. **A square plate cut cannot give an
  opening that follows the branch at both faces** except along the header axis of a tee; an opening that does is cut
  along the branch axis and has two edges, T(φ; R_o) on the outside face and T(φ; R_i) on the inside face (N2).
  Clearance: FAB package H3 (line 70), question T3. No number exists.
- Today `set_in` is refused (regression 004 case 5). An undeclared joint with header `inside` and branch `outside`
  computes the trace on R_i for both parts; for the tee its opening is the zero-clearance square set-in opening derived
  above, while its branch end S(φ; r_o, R_i) leaves the bore fibre inside the header bore by
  S(φ; r_i, R_i) − S(φ; r_o, R_i) along the axis. It is not called set-in (`job.rs:1259-1263`).

**Set-through — the branch projects into the header.**
- Opening as set-in. End: either a plain end square to the branch axis at s = s_p (a drawing value), or a contoured
  end at a stated projection; the drawing must say which and how the projection is measured (T3). A plain end lies
  inside the header bore if and only if it is below the upper-sheet intersection and above the lower-sheet one for every
  fibre (Spec 02 §4.4 line 391: which root is used is a declared attribute). The upper-sheet height is concave in ρ and
  the lower-sheet height convex, so the extremes over the wall are at its two faces: s_p < min over φ of
  min(S(φ; r_i, R_i), S(φ; r_o, R_i)), and s_p above the corresponding maximum of the lower sheet. The branch pattern of a
  plain end is a rectangle, with no saddle.

### 2.6 Summary tables

What each part's contact surface becomes:

| Joint, end model | Branch surface in contact | Header surface in contact | Curves |
|---|---|---|---|
| Set-on S1 (today) | bore edge | outside surface | one: bore trace on R_o |
| Set-on S2 | governing fibre of the square end (2.4) | outside surface | two: S* and O_on |
| Set-on S3 (RB-001 open case) | whole end face, fitted | outside surface | two: outside-edge saddle and O_on (= bore edge) |
| Set-in | outside surface against the opening's cut face, with clearance; end at the inside surface | cut face of the opening; inside surface | end curve and opening region |
| Set-through | outside surface against the opening's cut face | cut face of the opening | opening region; end plane or contour |

What a DXF line represents:

| Part | Square cut (S1, S2, square set-in, plain set-through) | Non-square cut (S3, fitted set-in, opening along the branch axis) |
|---|---|---|
| Branch | one (φ, s) curve drawn at (r_n·φ, s): the cut edge on every face (N1) | one named edge on the cutting layer; the other edge as a table in the result and the records, not drawn (D10; drawing it on `REFERENCE` needs the gate change of 2.5); lean per azimuth (N2) |
| Header | one (x, ω) curve drawn at (x, R_n·(ω + π)) on every face (N1) | one named face's edge on the cutting layer; the other face's edge as for the branch |
| Both | lengths round the part at r_n (rolled plate) or at the template radius (N3); the finished edge, kerf not compensated (`develop.rs:158`) | as left; with cutting taper the reference face of the line matters (Spec 02 §6.4, line 513) |

---

## 3. Inputs a thickness model needs

"User" = typed from the drawing by the person entering the job. "Engineer" = a fabrication-engineer value, recorded as
process data with its source class (Spec 02 §6, line 479: EI engineering input, MD material database, CAL calibration,
PT physical test) and, per Spec 02 §6.6 (line 521), value, uncertainty, validity range, evidence and owner. "Derived" =
computed by the engine.

| # | Input | Per | Kind | Source class | Needed for | Today |
|---|---|---|---|---|---|---|
| 1 | Radius of one face and which face it is | part | user | EI | everything | exists: contact radius + surface reference (`job.rs:66-94`) |
| 2 | Wall thickness T, t | part | user; which value (nominal, minimum, measured) is an engineer decision (T7) | EI/MD (Spec 02 §6.2 table, line 499) | faces, checks, every two-curve model | not an input; a field named `thickness` is refused (`job.rs:204-206`) |
| 3 | Process: rolled from plate, or purchased pipe with a template | part | user, confirmed by engineer (T5) | EI | which radius develops (2.3) | implicit "rolled" (`develop.rs:921`, `:1133`, `:1169`) |
| 4 | Neutral factor K, k, defined as (r_n − r_i)/t (Spec 02 §6.1) | part, material, thickness, r_i/t band, process | engineer (T4) | CAL/PT; EI with recorded justification (line 483) | r_n | not an input; 0 < k ≤ 0.5 used as a range only (`job.rs:1378-1381`); minimum k is register question 1 |
| 5 | Validity range of the rolling model in r_i/t, material, width | process | engineer (T4; register question 6, largest t/R) | CAL (Spec 02 §6.3 line 509; §4.1 limits line 338) | whether k applies | none |
| 6 | Development radius R_dev, r_dev | part | user today; derived or checked (3.1) | — | patterns | entered (`job.rs:68`, `:72`) |
| 7 | Joint type and end model (S1, S2, S3; set-in square or fitted; set-through plain or contoured) | joint | user (drawing); which models are allowed: engineer (T1, T3) | EI | curves | `set_on` = S1 only (`job.rs:1113-1149`) |
| 8 | Which face's edge the cut or mark line is, for a non-square end or opening | part | engineer and drawing (T1, T2, T10) | EI | DXF statement | none (square cut assumed, `develop.rs:955`) |
| 9 | Cut direction through the header wall: square to the plate (radial) or along the branch axis | joint | engineer (T2) | EI/CAL | opening | radial implied (`develop.rs:1170`) |
| 10 | Fit-up acceptance: largest gap and largest interference, and the measure (along the axis or along the normal) | joint | engineer (T6) | EI | the acceptance verdict | none |
| 11 | Opening clearance (set-in, set-through) | joint | engineer or welding procedure (T3) | EI | opening | none |
| 12 | Projection s_p and its datum (set-through) | joint | user (drawing); convention by engineer (T3) | EI | end | none |
| 13 | Template allowance (template material) | part | engineer (T5) | EI | template radius | none |
| 14 | Bevel angle, root face, root gap | joint | welding procedure | EI (Spec 02 §6.5, line 517) | only if weld preparation is ever modelled (section 7) | not modelled (`develop.rs:158`, `:955`) |
| 15 | Kerf width, which party compensates, taper reference face | process | engineer (T8) | CAL (Spec 02 §6.4, MATH-18, line 513) | DXF statement; compensation only if the owner decides | not compensated (`develop.rs:158`) |
| 16 | Seam weld gap g | part | drawing or welding procedure (T12) | EI (Spec 02 §4.1 line 330, §6.3 line 509) | developed width | out of scope unless asked |
| — | r_i, r_o, r_n per part; implied k; S, S*, governing fibre; standoff and interference profile; lean per azimuth (S3); inside-face lip; set-in opening region; limits \|δ\| + r_o < R_o or R_i | — | derived | — | — | — |

### 3.1 The development radius: two options (owner decision D3)

- **(a) Derive it.** Enter one face radius, its surface, t and k; the engine computes r_n = r_i + k·t. Nothing is
  compared, so no tolerance is needed, but no thickness job can compute until a fabrication engineer has supplied k.
- **(b) Keep entering it** (today's required field) and add t. The engine refuses the pair unless the implied
  k = (r_dev − r_i)/t lies in 0 < k ≤ 0.5, the rule of Spec 02 §6.1, which needs no fabrication value; it compares the
  implied k with the engineer's range once that exists and reports INCONCLUSIVE until then. For an `outside` radius
  this replaces the B2-001 advisory (`job.rs:1435-1438`) by an exact rule, and for an `inside` radius it supplies the
  missing upper bound (REVIEW_B2:148). External review C asks for these bounds also on requests without a thickness
  (RC-02, `docs/backend/REVIEW_C_EXTERNAL.md:21`); that is tracked to task 1.4, not to this design. The RC-02
  verification says "no refusal without fabrication questions 1 and 6"; that concerns requests **without** a thickness,
  where the bounds cannot be formed. With t entered, an implied k outside (0, 0.5] contradicts the rule of Spec 02 §6.1
  (line 483) itself, not a fabrication value, so refusing it pre-empts no engineer. Only the two ends of that range are
  refused; any narrower range (a minimum k above 0, register question 1; the r_i/t band, question 6) is not applied and
  stays INCONCLUSIVE until question T4 is answered.
- **How the comparison is made (option b).** k is not formed by division for the check. With r_o = r_i + t,
  0 < k ≤ 0.5 is the same as r_i < r_dev and 2·r_dev ≤ r_i + r_o. Both are compared with the binary64 allowance of
  `SAME_RADIUS_REL` (1e-12 relative, "binary64 noise only; this is not an engineering tolerance", `job.rs:1371-1373`),
  in the same form as the `same` test of `radius_consistency` (`job.rs:1415`): the upper bound is broken only when
  2·r_dev − (r_i + r_o) > `SAME_RADIUS_REL`·max(|2·r_dev|, |r_i + r_o|), and an r_dev within that allowance of r_i
  counts as k = 0 and is refused. Why the form matters: k = 0.5 (mean radius) is the typical value for rolled shells
  (Spec 02 §6.1, line 483), so the most common input lies exactly on the bound. The DN300 header of regression 004
  (outside radius 161.95, development radius 156.795) with the wall t = 10.31 of REVIEW_B:135 (bore 151.64, the header
  radius of regression 004 case 4) is exactly at mean radius; illustration computed for this document: in binary64
  (IEEE 754) the quotient (156.795 − (161.95 − 10.31))/10.31 evaluates to 0.5000000000000001, so a check written as
  "k ≤ 0.5" would refuse it, while 2·156.795 and 151.64 + 161.95 both evaluate to 313.59. That the allowance covers
  the rounding of the entered values and of the two operations is to be shown in the stage-A derivation (section 5,
  item 1), not assumed. The allowance covers binary64 rounding only. A development radius above the mean radius by
  any larger amount, including the rounding of a value written on a drawing, is refused. Whether the check may allow
  for such rounding, and by how much, is an engineering tolerance: question T7 and decision D3, never assumed.
- **If k ever becomes a request field** (option a, or both options offered), entering both r_dev and k for an
  `outside` or `inside` radius gives r_n two values, and comparing them would need a tolerance that nobody has:
  refuse that combination, or require equality within `SAME_RADIUS_REL` (D3). A `neutral` radius is exempt: r_dev = c
  is already forced (`job.rs:1448-1454`), so a k entered with it gives r_n no second value; it only places the faces
  (table below).

Recommendation for the owner's decision: (b) first, because it computes without any process value and keeps the
current required field. k is then not a request field, so a `neutral` radius with a thickness is refused in stage A
(the faces cannot be placed).

How the faces follow from the entered radius c and t:

| Declared surface of c | r_i | r_o | r_n (option b) | Refused when |
|---|---|---|---|---|
| `outside` | c − t | c | entered | t ≥ c (no bore); implied k outside (0, 0.5], compared as above |
| `inside` | c | c + t | entered | implied k outside (0, 0.5], compared as above |
| `neutral` | c − k·t | c + (1 − k)·t | c (r_dev = c forced, `job.rs:1448-1454`) | without k the faces cannot be placed: refused for every model that needs a face, which in stage A (no k field) is every thickness request; with k (option a) the faces are placed and r_n is not over-determined |
| `unknown` | — | — | — | always, for a request with a thickness: no face can be placed |

With t, an `outside` branch radius (the natural input, the pipe outside diameter, REVIEW_B:137) gives the bore r_i, so
S1 becomes expressible from the outside diameter. The pairing rule `job.rs:1116` then becomes "the faces the end model
needs must be derivable", for requests that carry a thickness only. The entered face is then no longer the contact
face; what the existing surface keys, TEXT lines and record fields mean for such a request is fixed in section 4.2.

---

## 4. Outputs that change, and compatibility

### 4.1 Compatibility rule for requests without thickness

- **What the compatibility test pins** (`crates/nf-core/tests/compat.rs:5-9`, fixture
  `crates/nf-core/tests/fixtures/compat_0.4.0.json`): for every case of its corpus, the same parse result or parse
  error text, the same state, refusal text and refused field, and for an accepted case the same cut geometry byte for
  byte and the same pattern quantities and curves. The annotation (TEXT entities and 999 comments), the closure and the
  records may change. The test must pass **without regenerating the fixture**; the regression cases 001–010 and the
  full validation corpus must give the same answers.
- **What this design promises in addition**, for a request that carries none of the new fields: the same refusal
  code and detail, the same advisories, gate verdicts and gate texts, and the same DXF statements (keys and TEXT). Only
  the engine identifier changes, and with it the records and the closure hash (`compat.rs:9` allows both).
  **One exception, only if the owner chooses it (D8):** an advisory on today's `set_on` results with β ≠ 90° or δ ≠ 0
  changes their G02 text and their closure hash, and nothing that `compat.rs` pins. Regression 006 is the precedent:
  B2-001 changed the G02 detail and the closure hash of the affected inputs in the same way and left patterns, DXF and
  gate pass/fail unchanged (its `description` and `fixed_in`).
- **New fields get new names** (proposed below); the field `thickness` stays an unknown field and is refused in every
  family. Its refusal is pinned by `compat.rs:252` (a hand-written case) and `:315-317` (generated `header_with_branch`
  cases carry it with probability 0.05),
  `crates/nf-core/tests/refusal_codes.rs:342-346`, `crates/nf-server/src/tests.rs:500-503` and `:2801-2807`, and
  `tests/e2e_smoke.sh:67`. If the owner prefers the name `thickness`, every one of these is a changed expectation that
  must be listed with old value, new value and reason (work plan rule A.2).
- A `joint` other than `set_on` in a request without the new fields keeps `joint_type_unknown` and today's text
  (`compat.rs:312-314` draws `set_in`; regression 004 case 5).
- **Engine identifier and G21.** A stored result is REJECTED with gate G21 "Computed by a superseded engine version …
  — recompute", and its files are withheld, whenever its engine identifier differs from the running engine's
  (`crates/nf-core/src/lib.rs:75-77`; `crates/nf-server/src/main.rs:1776-1782`). The identifier embeds a hash of the
  engine sources (`lib.rs:57`; `crates/nf-core/build.rs:17-20`), so any change to those sources marks every stored
  result, whether or not the 0.4.x number changes (REVIEW_A_ARCHITECTURE.md:159-160). Stage A does so although it
  changes no cut geometry, and so does every later stage, each time a build containing it is deployed. There is no
  recompute endpoint yet (REVIEW_A_ARCHITECTURE.md:157; task 2.2, RA-007), so each stored job must be entered again.
  The version-number bumps themselves (rule A.8) were delegated by the owner on 21 Nov 2026, "one per task that changes
  results" (work plan `docs/project/CLI_WORK_PLAN.md:54-58`), although the plan's closing list still names any bump as
  a stop-and-ask point (`:459`); the live server is updated only on the owner's word (`:58-59`, task 4.6). Combining
  bumps therefore saves nothing; what matters is how many deployments mark the stored results, decision D7.

### 4.2 What changes for a request that carries a thickness

Sections 4.2 to 4.4 are indicative: field names, key names, refusal-code names and the gate placement show the shape
of the change and are fixed at implementation (decisions D4 and D5). Approving this design does not freeze them.

| Output | Change | Today |
|---|---|---|
| Request | new optional fields on `header_with_branch`, for example `header_wall_thickness`, `branch_wall_thickness`, `header_process`, `branch_process`, `end_model` (names are proposals, D4); the new lengths join `LENGTH_PARAMS`, so the implementation limits of every length apply to them (a positive value below 1e-6 mm or a magnitude above 1e8 mm is refused; "implementation limits, not engineering rules", `job.rs:1472-1493`) | `job.rs:37-103`, `FAMILY_FIELDS` `job.rs:116-152` |
| Patterns | S1: the same lines as today for the same radii. S2, S3, set-in, set-through: new curves, for new requests only | `develop.rs:793-959`, `:977-1172` |
| Development class | Spec 02 calls the as-fabricated result class A (§0.2 line 35, §4.1 line 339, §4.4 line 401) and a class may only be weakened (MATH-02, line 42). The code has E-C and E-N only (`crates/uem/src/lib.rs:83-95`); thickness jobs would be labelled A. Whether today's labels change is not part of this design (the compatibility rule forbids it) | `develop.rs:916` |
| Quantities | r_i, r_o, r_n and implied k per part; for each end model the governing fibre round the saddle, the largest gap and the largest interference with their azimuths; S3 lean range; inside-face lip; set-in opening extent | — |
| Pattern texts | `ASSUME_REF` (`develop.rs:17`), `KERF_NOTE` "NO BEVEL" (`develop.rs:158`) and the assumptions at `develop.rs:954-955`, `:1170` get thickness-job versions; the old texts stay for old requests | — |
| DXF statements | new `NF-` keys (wall thickness per part; entered face and development face per part, beside the existing contact-face keys, see below; end model; "square cut" or the face whose edge the cutting layer carries) and joint states beyond `set-on` / `not declared`; matching TEXT lines and `JOINT_TEXT_PREFIXES`. The reader, the re-import check and the quality gate learn every new key before any file carries it, because an unknown `NF-` key fails the gate (`DXF_METADATA.md:27`) | `crates/nf-dxf/src/lib.rs:47-58`, `:69-115` |
| Second edge (S3 and similar) | default: a table in the result and the records (height of the other edge and lean at stated azimuths), with a DXF statement saying where it is; never on a `CUT` layer. Geometry on `REFERENCE` only if the owner chooses it (D10), after the quality gate (`wrong_layer`, `quality.rs:446-457`), the re-import comparison and the independent reader accept that layer (stage C) | `crates/uem/src/lib.rs:45-49`; no output emits `REFERENCE` (`job.rs:372`) |
| Records | parameter claims for each new user input with today's axes (`job.rs:1178`); fabrication values as their own records with source class, value, uncertainty, validity range, evidence and owner (Spec 02 §6.6); the `convention` record carries the wall data (`job.rs:1172`); one `intersection_curve` record per curve when the curves differ (`job.rs:1250-1256`); the `interface` record names the contact faces and the end model (`job.rs:1264-1270`); dependency edges thickness → curves → both patterns. **Append-only:** new kinds are added, no stored record is rewritten, and old results are marked by G21 when read | — |
| Gates | G02 states the wall data and stays INCONCLUSIVE (`job.rs:924-941`, `:1358-1367`); G05 class A; G06 says thickness is applied as entered and lists what is not modelled, and stays INCONCLUSIVE until the fabrication values exist (`job.rs:856-862`); G08: a proved bound for every new quantity or INCONCLUSIVE (task 1.2); G10: the checks of 4.3; G11: the same rules for the new curves; G15: the new statements read back, and the contact-face check below. Where the fit-up acceptance sits (G06 or a new gate) is D5; it is INCONCLUSIVE until question T6 is answered and never PASS before | `job.rs:820-943` |
| Refusal codes | section 4.4, added to `docs/backend/REFUSAL_CODES.md`, `refusal_codes.rs` and the form labels in English and Japanese. The sentence "Thickness, allowances, weld gap and kerf are not modelled, so no rule here depends on them" (`REFUSAL_CODES.md:18-20`) changes | — |
| Messages | the `joint_pairing_invalid` text (`job.rs:1130`), the G02 sentence "no wall thickness is modelled" (`job.rs:1359`) and the `radius_consistency` notes "Thickness is not modelled" (`job.rs:1416-1420`, `:1435-1438`) stay for requests without thickness; thickness requests get their own texts | — |
| Interface | form fields per part; the BE-001 rule mirrored in `web/app.js` (`DEFECT_REGISTER.md:20`) extended; English and Japanese; `docs/backend/UI_CHANGES_NEEDED.md` | — |

**Meaning of today's joint statements for a request with a thickness (stage A, before any such request is
accepted).** Every existing statement assumes that the entered radius is the contact radius:

| Statement | Today | For a request with a thickness |
|---|---|---|
| `NF-HEADER-SURFACE`, `NF-BRANCH-SURFACE` | "the wall surface the header's / branch's contact radii refer to" (`DXF_METADATA.md:21-22`); the job's `JointStatement` is built from the request's surfaces (`job.rs:1086-1088`) | unchanged meaning: the CONTACT face, derived from the end model (S1: header `outside`, branch `inside`), never copied from the entered surface |
| entered face, development face | not stated (they are the contact surface and r_dev) | keys, TEXT and record fields of their own per part (names: D4), for example the entered face `outside` and the development face "neutral, implied k" or "template surface" (N3) |
| set-on TEXT "BRANCH INSIDE SURFACE ON HEADER OUTSIDE SURFACE, OPENING IS THE BRANCH BORE" | `crates/nf-dxf/src/lib.rs:101-102` | true for S1 whatever face was entered, because the contact faces are derived; S2, S3, set-in and set-through get their own `NF-JOINT` value and TEXT |
| "HEADER SURFACE: … - BRANCH SURFACE: …" | `crates/nf-dxf/src/lib.rs:106-112` | the contact faces; the entered and development faces get a TEXT line of their own |
| "not declared" TEXT "… ONE CURVE OF THE ENTERED RADII" | `crates/nf-dxf/src/lib.rs:104` | never on a thickness job: a thickness request without a declared joint and end model is refused (`end_model_invalid`, 4.4) |
| `convention` record | surfaces of the contact radii, scope "contact radius and its development radius" (`job.rs:1172`) | entered face, contact face and development face per part as separate fields |
| `interface` record | contact surfaces and the constraint "branch end cut and header opening are the same intersection curve" (`job.rs:1264-1270`) | contact faces and end model; the one-curve constraint only for S1; two-curve models state their own constraint |
| `Branch` value | "Header contact radius", "Branch contact radius" (`develop.rs:616-627`) | the derived contact radii (S1 with branch `outside`: the bore r_i = c − t) |

The quality gate compares a file only with the job's own `JointStatement` (`quality.rs:322-324`), so it cannot see a
statement built from the wrong face: if the statement were still built from the entered surfaces as today
(`job.rs:1088`), an S1 request entered with branch `outside` would write `NF-BRANCH-SURFACE: OUTSIDE` next to the
set-on TEXT and pass. Stage A therefore adds a check, reported under G15,
that the contact faces stated in each file equal the faces whose radii produced the curve, as recorded in the
`intersection_curve` and parameter records; a mismatch withholds the file.

### 4.3 Verification checks whose meaning changes

- The joint is no longer one curve. `verify_interface` and `verify_joint` require the folded opening and the folded
  saddle to coincide (`verify.rs:984-1006`, `:1074-1089`); for S2, S3, set-in and set-through the two parts have
  different curves by definition. For those models the checks become separate membership checks, each with its own
  stated tolerance basis: (i) the branch's governing edge (both edges for S3) lies on the header surface it rests on,
  by implicit residual, a formulation that does not use the parametrisation (Spec 02 §4.4 (i), line 403); (ii) the
  opening lies on the bore trace (set-on) or clears the branch outside surface at every header radius (set-in); (iii)
  for a square end, no fibre enters the header wall: the governing fibre is found in closed form at each tested
  azimuth (2.4), and the azimuth sampling carries a proved resolution or the check is INCONCLUSIVE; (iv) the gap and
  interference profile is stated, not judged, until question T6 is answered. The job then passes geometries derived
  separately per part (today it passes the same value twice, `job.rs:1302-1321`; external review C, RC-16,
  `REVIEW_C_EXTERNAL.md:35`). Separate geometries close the shared-input gap, not the shared-frame convention
  boundary of 1.4: that stays UNVERIFIED until a fabrication engineer confirms the frame.
- `validation/reference/families.py` shares the single-curve joint model (REVIEW_B:107). It is extended first, from a
  written derivation, never from `develop.rs` (work plan rule A.9 and task 1.5).
- The 3D mating check of the validation harness also assumes one curve: "the folded header opening and the folded
  branch saddle must coincide in 3D" (`validation/checks/compare.py:11`, function `mating`, `:485-486`;
  `validation/README.md:24`). It would fail S2, S3, set-in and set-through by construction. Two-curve form: the
  opening and the branch edge that the end model puts in contact coincide (S1; the bore edge of S3); the other edge of
  S3 lies on the header outside surface; for S2 every fibre of the folded end lies on or outside the header outside
  surface and the governing fibre touches it; for set-in the folded opening clears the folded branch outside surface
  at both header faces. Each form is derived independently first (rule A.9).
- The planned DXF-only mating test of task 1.5 ("confirm the two cut edges coincide", work plan
  `CLI_WORK_PLAN.md:274-276`) has the same single-curve form. For two-curve joints it becomes: the edges the two files
  name as in contact coincide, and each other named edge lies on the surface its end model states, with the standoff
  and interference profile reported, not judged, until question T6 is answered.
- The branch's developed-length check (`verify.rs:529-545`) already follows (M) and stays.

### 4.4 Refusal codes (names are proposals)

| code | rule | fields | detail |
|---|---|---|---|
| `wall_thickness_not_below_radius` | the thickness is at least the outside radius it is taken from (no bore) | the thickness, the radius | yes: `limit` the radius, `measured` the thickness |
| `neutral_factor_out_of_range` | implied k outside (0, 0.5] (Spec 02 §6.1), compared as r_i < r_dev and 2·r_dev ≤ r_i + r_o with the `SAME_RADIUS_REL` allowance (3.1) | the development radius, the radius, the thickness | yes: `limit` the bound on the development radius (r_i or (r_i + r_o)/2), `measured` the development radius; the message states the implied k |
| `wall_face_undefined` | a thickness with surface `unknown`, or `neutral` without k for a model that needs a face (in stage A, where k is not a field: every `neutral` thickness request) | the surface field, the thickness | no |
| `end_model_invalid` | the end model is not one this build offers for the joint, or its inputs are missing | `joint`, `end_model` | no |
| `branch_tangent_or_partial` (existing, `develop.rs:829-831`) | the same rule applied to the outermost fibre: \|δ\| + r_o ≥ R′·(1 − 1e-6), with R′ = R_o (set-on) or R_i (set-in, set-through) | the radii, the thicknesses, `delta` | yes |
| `set_through_end_not_inside_bore` | the plain end is not inside the header bore (2.5) | the projection | yes |
| `opening_outside_shell` (existing, `develop.rs:1079-1087`) | the opening, including a set-in opening region, does not lie inside the shell length | as today | as today |

An interference is never a refusal by itself while no acceptance value exists; it is a stated quantity with an
INCONCLUSIVE acceptance, and a FAIL (state REJECTED) only against a value a fabrication engineer has supplied.

---

## 5. Validation plan

1. **Independent derivation first.** Before any engine code, a reviewer who has not read the implementation derives
   S(φ; ρ, R′), its concavity and the governing fibre, S*, the two edges and the lean of S3, the inside-face lip, the
   set-in opening region, the plain set-through condition, and the mapping (M) with N1–N3, from Spec 02 §4.4 and §6.1
   and first principles, together with the argument that the `SAME_RADIUS_REL` allowance of 3.1 covers the rounding
   of the entered values and of the two operations. The derivation goes into `docs/backend/TOLERANCE_POLICY.md` and
   into `validation/` with the derivation in the diff (work plan rule A.9). The derivations in this document are
   input to that review, not a replacement for it.
2. **High-precision reference.** The mpmath tier (`validation/reference/hp.py`, 40 digits, `validation/README.md:22`)
   for every new curve and quantity. The governing fibre by maximising S over [r_i, r_o] directly (golden-section search
   on S), which does not use the derived ∂S/∂ρ and so stays independent of it; where cos φ = 0 and β = 90°, S does not
   depend on ρ and every fibre touches (2.4), so the reference reports the height, not a fibre. The illustration of
   section 0.1 (4.105 mm and 4.147 mm) is reproduced here before it is quoted anywhere else. The set-in union by a
   polygon-union method written independently, with its resolution stated.
3. **Error bounds.** Each new quantity gets an a-priori bound like `s_error_bound` (`develop.rs:646-677`), or its gate
   stays INCONCLUSIVE (task 1.2).
4. **Regression cases just inside and just outside each new bound**, chosen by the test author at the bound and not as
   engineering values: thickness against the outside radius; implied k at 0 (excluded), just above 0, at exactly 0.5,
   and above 0.5 by just more than the allowance of 3.1. The case at exactly 0.5 uses the DN300 header of regression
   004 at mean radius: `outside` radius 161.95 with development radius 156.795 and t = 10.31 (REVIEW_B:135), and the
   same wall entered as `inside` radius 151.64 (regression 004 case 4); both must be accepted although the quotient
   evaluates to 0.5000000000000001 (3.1). Further: |δ| + r_o against R_o·(1 − 1e-6) and against R_i·(1 − 1e-6); the
   centred tee against β = 90° plus and minus a small angle and δ plus and minus a small offset (the S1 interference
   changes from none to some); an azimuth where the governing fibre leaves the bore face; s_p at the bore limit; a
   set-in opening at the shell end.
5. **Limits and identities.** t → 0 makes S1, S2 and S3 coincide with today's curve; for the centred tee S1 = S2
   exactly; the bore edge of S3 equals today's set-on curve for the same inputs; on the generator on the side of the
   lean the outside fibre's height exceeds the bore fibre's by t·|cot β|; the tee lip is r_i·T/R_o; the set-in union of
   the tee is the trace on R_i with gap r_o·T/R_i.
6. **Compatibility.** The compat fixture unchanged (4.1); regressions 001–010; the full corpus with 0 disagreements; the
   BE-016 tests.
7. **Planted faults** (`validation/seeded_faults.py`): inside and outside faces swapped; r_o used where r_i is needed;
   a rolled part developed at r_o instead of r_n; the lean sign reversed; the opening taken from the wrong face; the
   thickness ignored; the implied-k check written as the quotient compared with 0.5 (it refuses the mean-radius case
   of item 4); the entered face written as the contact face in the DXF (4.2). Each must be detected.
8. **Gates INCONCLUSIVE until the fabrication values exist** (k range, fit-up acceptance, clearance). Nothing PASSes on
   an assumed value, and nothing reaches APPROVED.
9. **Physical evidence.** A real cut and fitted joint compared with its patterns (work plan phase 6) is the only
   evidence about a shop; until then every result stays EV-1 and NOT RELEASED.

---

## 6. Questions for a fabrication engineer

These extend the register's list ("Questions for a fabrication engineer" in `docs/backend/DEFECT_REGISTER.md`,
questions 1–10, cited by number) and the FAB package (statements H2–H4, H10, H11, H14–H16, lines 69-83; its
questions, line 87). Where a question here refines a register question, the column "Register" names it, and the
register entry is to be extended rather than a second question added: T1 and T2 refine question 7 (set-on joint with
wall thickness); T4 refines question 1 (minimum K-factor) and, with input 5 of section 3, question 6 (largest t/R);
T8 refines question 9 (kerf and CAM); T10 refines question 5 (marking side). T3, T5, T6, T7, T9, T11 and T12 are new.
No answer is assumed anywhere in this design.

| # | Question | Decides | Register |
|---|---|---|---|
| T1 | How is the end of a set-on branch cut: square to the pipe wall, or fitted to the header surface with a lean that changes round the saddle? Which edge rests on the header: the bore edge (Spec 02 §4.4, H2), the outside edge, or wherever a square end touches first (2.4)? When the saddle is laid out on the branch outside surface (the RB-001 open case), what happens to the inner part of the end (2.5, S3 and S4)? | the set-on end model; S2 or S3 | refines 7 |
| T2 | Is the set-on opening matched to the bore on the header outside face, on the inside face, or cut along the branch axis? Is the lip at the inside face (2.5) accepted, ground off, or avoided by the cut direction? Are openings cut in the flat plate before rolling, or in the rolled shell? (H4; REVIEW_B:121) | the opening | refines 7 |
| T3 | For set-in and set-through: the clearance between the branch outside surface and the opening; which header face governs; whether "flush" means "no fibre inside the bore" or "touching"; the projection of a set-through branch, how it is measured, and whether its end is plain or contoured (H3) | set-in, set-through | new |
| T4 | The neutral factor k per material, thickness, r_i/t band and process, confirmed to be k in the definition used here, (r_n − r_i)/t, the distance of the neutral surface from the INSIDE face divided by t (tables and shops define "K-factor" differently); the minimum k; the range of r_i/t where the rolling model holds (Spec 02 §6.3) | r_n, the k check | refines 1 and 6 |
| T5 | Is each part rolled from plate, purchased pipe marked with a wrap-around template, or made another way (for example on a pipe-profile cutting machine)? For a template: its material thickness and the surface it is made for (H14–H16) | the development radius | new |
| T6 | The largest gap and the largest interference along a joint that the shop accepts before welding, and the measure: along the branch axis, along the wall normal, or across the joint | the fit-up acceptance verdict | new |
| T7 | Which wall thickness to enter: nominal, minimum with mill tolerance, or measured (Spec 02 §6.2 table)? To what precision are radii and wall thickness written on the drawings, and may the implied-k check of 3.1 allow for that rounding, and by how much? | input 2; any rounding allowance (D3) | new |
| T8 | Kerf width per process, material and thickness; which single party compensates it (Spec 02 §6.4, MATH-18); the reference face for cutting taper (REVIEW_B:122) | DXF statement; any later compensation | refines 9 |
| T9 | Does assumption N hold for the plates in use: does a line square to the flat plate stay square to the wall after rolling, without thinning or axial stretch? | the mapping of 2.3 | new |
| T10 | On which face are cut lines marked, and are they marked on the flat plate or on the formed part? | which edge a DXF line names | refines 5 |
| T11 | Can the shop's cutters cut a lean that changes along the cut, or is every flat-plate cut square? | whether S3 can be cut from the file or must be ground | new |
| T12 | The seam weld gap g of a rolled shell, if the owner asks for it (Spec 02 §4.1, §6.3) | developed width | new |

---

## 7. Out of scope unless the owner asks

Each of these is not modelled now and is not designed here; the outputs keep saying so.

- **Bend allowance for rolling beyond the position of the neutral surface:** unbent flat ends and pre-bending on
  three-roll machines (Spec 02 §6.3, line 509, CAL).
- **Weld shrinkage** Δ_w and distortion (Spec 02 §6.3, §6.5; PT).
- **Springback** (Spec 02 §6.2, line 505); it changes tool settings, and the achieved radius enters through r_i.
- **Thinning and elongation in heavy rolling, and axial strain** (Spec 02 §6.3); assumption N excludes them.
- **Weld preparation**: the bevel schedule from the two wall normals, cos Ω(φ) = n_branch · n_header (Spec 02 §6.5,
  line 517, class A), root face and root gap. S3 reports its lean per azimuth; no bevel schedule is designed.
- **Kerf compensation** (offsetting the profile; Spec 02 §6.4, ALG-10): the line stays "not compensated"; only the
  statement of the reference face is designed. Offsetting would change cut geometry and is a separate change (rule A.8).
- **Seam weld gap** g (T12).
- **Thickness for other families:** cylinder shell (needs task 1.4's surface attribute first), cone (radial wall
  t/cos α, Spec 02 §4.2 line 352; the normal is not radial, so (M) does not hold as written), eccentric reducer (the
  wall direction varies round the cone, so the offset is not uniform; Spec 02 §4.3 line 378, R-05).
- **`branch_stub`:** stays zero-thickness in the stages below (D6). It has one surface reference for both radii
  (`job.rs:60-63`) and no joint field, so it cannot state which face of which part is in contact.
- A purchased-pipe template as its own output type (D9); reinforcement pads; several branches on one header (work plan
  phase 5 item 2).

---

## 8. Decisions for the owner

Listed in the order in which they are needed; the numbers are identifiers only. The owner's delegation of 21 Nov 2026
(work plan `CLI_WORK_PLAN.md:54-58`) gives the CLI decisions the plan reserves for the owner, among them engine-version
bumps and "implementation of the design documents after an adversarial review", each reported; values only a
fabrication engineer can give and anything on the live server stay with the owner (`:58-60`).
This design keeps the stricter wording of task 1.7, "No code until the owner approves" (`:287`).

| # | When | Decision | Recommendation |
|---|---|---|---|
| D8 | now | Whether the finding of section 2.4 (today's set-on end, with a real wall, enters the header wall for β ≠ 90° or δ ≠ 0) is entered in the defect register now, and whether today's `set_on` results for those geometries carry an advisory now. Size, as an illustration (section 0.1): 4.105 mm at φ = 270° and 4.147 mm at most for regression 004 case 2 with t = 7.11 mm. The advisory changes the G02 text and the closure hash of those old requests, not their geometry; it is the one exception to 4.1, as B2-001 was (regression 006) | Yes to both. The register row quotes no millimetre figure until the stage-B reference reproduces it. The advisory cannot give the amount, because t is not an input; it says that with a real wall the bore-cut end enters the header wall at some azimuths (for β ≠ 90°, by t·\|cot β\| along the branch axis on the generator on the side of the lean), and that a fabrication engineer must check the fit-up. It is an engine source change, so G21 marks the stored results when it is deployed (4.1, D7) |
| D1 | now | Approve, change or reject this design, and the order of stages in section 9 | Approve stages A and B, which change no cut line; hold C until T1, T2 and T11 are answered and D until T3 is |
| D3 | now | The development radius: option (a) derived from k, or option (b) entered and checked (3.1); whether entering both r_dev and k is refused for `outside` and `inside` radii; whether the implied-k check may allow for rounding of entered values | (b), with the comparison of 3.1 and the binary64 allowance only; no rounding allowance until T7 is answered; k not a request field in stage A; if k becomes one, refuse both values for `outside` and `inside`, `neutral` exempt |
| D4 | now | The names of the new fields and keys | Keep `thickness` an unknown field so that no BE-016 expectation changes (4.1); the names proposed in 4.2 |
| D6 | now | Whether `branch_stub` gets thickness | Not in stages A–D (section 7) |
| D9 | now | Whether a template for purchased pipe (N3) is wanted as an output | Not now; reconsider after T5 is answered |
| D10 | now | How the second edge of a non-square cut is given (2.5, 4.2) | A table in the result and the records, with a DXF statement saying where it is; geometry on `REFERENCE` only later, after the gate change |
| D5 | before stage B | Where the fit-up acceptance verdict sits: gate G06 or a new gate | G06 (`assumptions_backed`, already INCONCLUSIVE on every job, `job.rs:856-862`), so no new gate identifier is needed; reconsider a gate of its own when T6 gives a value against which the verdict can FAIL |
| D7 | before the first deployment | Whether the stages are deployed to the live server separately or together. Each deployment of a build with changed engine sources marks every stored result for recomputation (G21), and no recompute endpoint exists (4.1) | One version bump per stage on the branch, as delegated; deploy stages A and B to the live server together, after task 2.2's recompute endpoint if possible; C and D together later. The live server only on the owner's word (task 4.6) |
| D2 | after T1, T2, T11 | Which set-on end models the product offers (S1 kept as today; S2; S3) | Decide when the answers exist; until then S1 only, with the stage-B fit report |

---

## 9. Proposed stages after approval

Each stage starts with the independent derivation and the reference (section 5, items 1–2), changes the pattern
mathematics at most once (rule A.8), keeps the compatibility rule of 4.1, bumps the engine version once (as delegated,
work plan `CLI_WORK_PLAN.md:54-58`, and reported), is deployed as D7 decides, and leaves every DXF NOT RELEASED.

| Stage | Content | Cut geometry |
|---|---|---|
| A | Optional wall thickness and process per part on `header_with_branch`; faces and implied k derived, with the comparison of 3.1; the refusals of 4.4 for impossible walls; quantities; the meaning of the existing joint statements for thickness requests, the new keys and the contact-face check (4.2); statements in the result, the records and the DXF | unchanged for the same radii |
| B | Fit analysis of today's set-on (S1) with a thickness: gap and interference profile round the saddle, inside-face lip of the opening; acceptance INCONCLUSIVE | unchanged |
| C | Two-curve set-on: S3 (the RB-001 open case) and, or, S2, after T1, T2 and T11; the second edge as a table (D10), or on `REFERENCE` only after the quality gate (`quality.rs:446-457`), the re-import comparison and the independent reader accept that layer; interface checks with separate geometries, and the 3D mating check of `validation/checks/compare.py` and the DXF-only mating test of task 1.5 in their two-curve form (4.3) | new curves, new requests only |
| D | Set-in and set-through, after T3, with the same two-curve checks | new curves, new requests only |

Not done by this document (one-file rule of the task): the work plan status of task 1.7; a defect-register row for the
finding of section 2.4 (D8); a defect-register row for the convention text at `develop.rs:924`, wrong on every branch
pattern with β > 90° (section 2.4; task 1.3); and the register's question list extended as section 6 maps it (T1 and
T2 into question 7, T4 into questions 1 and 6, T8 into question 9, T10 into question 5) with T3, T5, T6, T7, T9, T11
and T12 added as new questions.
