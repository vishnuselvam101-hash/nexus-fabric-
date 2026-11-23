# Review B — computational engineering, conventions, numerics and DXF

19 November 2026 · engine `nf-engine 0.3.0` (commit a447b0a) · reviewer B (independent; wrote none of the product, the specification or `validation/`).

Standing position, unchanged by this review: **nothing produced by this software is validated for manufacturing. 0 of 56 engineering statements have been reviewed by a fabrication engineer.** Everything below concerns whether the mathematics, the conventions and the files are internally right and stated; it cannot establish that any part is right in a workshop.

## 0. Verdict in brief

| Area | Score /10 | One-sentence justification |
|---|---|---|
| Geometric correctness | 8 | Every pattern I derived independently (branch, header opening, eccentric reducer, cone) matches the product to ≤ 1e-11 mm, from the outside, with the stated seams, and the two parts of an assembly mate from the DXF files alone; the deduction is for a joint model that cannot represent a set-on joint with wall thickness (RB-001). |
| Mathematical integrity | 7 | The equations follow Spec 02 and are correct, but the specification's own requirements for adaptive sampling, certified ε_rep and bounded ε_num (ALG-07/09, MATH-20) are replaced by fixed sampling and estimates. |
| Numerical stability | 7 | Rounding errors stay ≤ 1e-11 mm right up to the refusal limits and the refusals are consistent across families; ε_num is still not a bound (×1.9 reproduced), and the polyline error grows with part size (2.4 mm at R = 1000) with no gate acting on it. |
| DXF fidelity | 7 | The files are clean (ezdxf audit 0 errors on 23 files, bit-exact round trip, exact arcs, shortest round-trip numbers), but they don't say which edges are seams, which way to roll, which end is station 0, or who applies kerf, and closed polylines carry no closed flag. |
| Strength of verification (product's own checks) | 4 | The checks invert the product's own constructions, come from the same author and live in the same crate; the interface check receives the same `Branch` object for both parts, and the re-import check compares the file with the product's own intended entities, so a wrong but self-consistent result passes them all. |
| Independence of `validation/` | 6 | It uses different numerical methods and frames (and it found D1), but it takes input meaning, seam placement, the single-curve joint model and the accept/refuse limits from the same unreviewed specification and from the product's own documentation. |
| Honesty of stated limitations | 8 | NOT RELEASED marking, estimates labelled, provisional marking side and 0/56 are all stated throughout; small overstatements remain (RB-006, RB-008). |
| **Overall** | **6** | The geometry is right in the frame the specification defines, but three conventions that decide whether the parts fit the intended pipework are unstated or unrepresentable, and no product check can see them. |

**The third failure of the same kind (RB-001).** Each part is right in its own convention: the header opening is cut to "the branch contact surface" and the branch end to "the header contact surface". The job is labelled *set-on*. No check compares the contact convention with the joint type. The most natural input (pipe ODs, `surface_reference: "outside"`) produces a "set-on" header whose opening is the branch **outside** diameter, so the branch has no land to sit on and drops through. Every check PASSes. The specification's own set-on example (branch inside surface on header outside surface) is **refused** under every declared surface and can be entered only as `unknown`, which switches off the radius checks.

**CRITICAL findings (a wrong part produced without warning): none in the strict sense.** Every output is marked NOT RELEASED, G16 is NOT_IMPLEMENTED and G02 is INCONCLUSIVE. RB-001, however, produces a wrong part with all verification checks PASS and only generic warnings. It would become CRITICAL the day the release gate is opened.

## 1. Independent derivation and comparison (work item 1)

### 1.1 Method

I wrote all of this code myself, in Python with mpmath at 40–50 digits, and did not import or read `validation/` until I had my own numbers. The scripts are under the session scratchpad (`reviewB/mine.py`, `cmp_asm.py`, `cmp_cone.py`, `cmp_tan.py`, `cmp_hdr.py`, `fitgap.py`, `threeway.py`, `dxfaudit.py`). The product was a private copy of `target/release/nf-server` on port 8853, and its DXFs were read with **ezdxf** (not the product's reader).

- **Branch / header.** The header axis is X and the branch axis runs through (0, δ, 0) with direction (cos β, 0, sin β). I chose my own azimuth basis deliberately opposite to the specification's (e₂ = e₁ × a). Each ruling meets the header through the general quadratic, upper root. The outside view is decided **numerically** for every chart: (∂P/∂u × ∂P/∂v)·n_out > 0, not taken from the algebra. For the header plate, the seam sits at −z and the direction of the angle is picked by the same test.
- **Eccentric reducer.** ψ(θ) is the limit of the summed true 3D angles between successive rulings (Richardson extrapolation, m = 16/32 sub-steps per product segment), and ρ(θ) is exact. I did not use the closed-form dψ/dθ of eq. (4.1). A triangulated strip unroll (O(h²)) decides which way round is outside. A note on my own error: the triangulated strip is not exact even though each ruling quad is planar, because the facets are secant planes. At n = 360 it is 0.0056 mm off for ECC-01, which is the faceting error of the traditional shop method.
- **Cone.** Closed form: L = r / sin α, Φ = 2π sin α.
- **Mating, using only the DXF files.** I rolled each DXF plate with the drawn face outward (the declared side): the header about DXF X, the branch about DXF Y. I projected radially to the contact radius, then searched for the best **proper** rigid motion of the rolled branch cut onto the rolled header opening (ICP from 40 random starts). This test uses no product parameter: no station, β, δ or φ datum.

### 1.2 Results

| Case (inputs) | Quantity | Mine | Product (JSON = ezdxf-read DXF, bit-identical) | `validation/` reference | Notes |
|---|---|---|---|---|---|
| asm1: R 150, R_dev 150, r 100, r_dev 100, β 60°, δ +25, station 500, s_end 400, L 1200 | saddle vertex → my outside-view curve | — | max 1.1e-13 mm | 1.6e-13 vs product, 1.1e-13 vs mine | mirrored branch would be off by 113.8 mm |
| asm1 | header opening vertex → my outside-view curve | — | max 1.6e-13 mm | 3.4e-13 vs product | mirrored header off by 72.5 mm |
| asm1 | mating from DXF only, both outside | — | gap ≤ 0.021 mm (limited by ICP convergence; exact placement gives 0.0014 mm) | — | with the branch marked on the other face: 14.7 mm |
| asm2: R 300, R_dev 294, r 80, r_dev 76, β 115°, δ −120, station 900, s_end 700, `outside` | saddle / opening | — | 1.1e-13 / 2.3e-13 mm | agrees | mating 0.022 mm (ICP), 0.0009 mm exact placement; other face 13.3 mm |
| ECC-01 r1 200 r2 100 h 300 e 100 | total developed angle | 1.86189128922466219 | 1.861891289225 (diff 1.1e-15) | 1.8618912892246624 | outer vertices: max 1.0e-12 mm vs mine; outside view; proper fit 0.0056 mm vs triangulated strip, mirror 572 mm |
| ecc2 r1 350 r2 120 h 150 e 60 | total developed angle | 5.20501869127505063 | diff 1.8e-15 | diff 1.8e-15 | vertices 1.6e-12 mm; ε_rep 0.01184 equals the true chord deviation |
| CONE-01 | L₁, L₂, Φ | 632.4555320336759, 316.22776601683796, 113.84199576606164° | identical in the DXF ARC entities | identical | seam on +x; arcs exact |
| near-tangent stubs (§3) | s(φ) | mpmath | ≤ 1.0e-11 mm off | **5.8e-11 mm off** | the validation float tier is less accurate than the product there |

**Three-way comparison.** There is no three-way disagreement, and no case where `validation/` and the product agree while I differ. The only asymmetry is in the near-tangent region (hn2, br_t5, br_t1). There the validation's float reference (bisection) is 4–6× further from mpmath than the product is, so its float tier cannot judge ε_num there; only its mpmath tier can.

**Position, orientation, seam, start angle, direction, units, face.** All patterns are drawn from outside, as declared. The branch seam is on the ruling at +y, i.e. the side toward +δ. u starts at that seam and increases in the outside-view direction, and v = s is measured from the header axis. The header plate has x along the axis from the station-0 end, the seam at the bottom, and v increasing bottom → −y → top → +y. For δ > 0 the opening lies above the plate centreline v = πR_dev; asm1 (δ = +25) has the opening's v-range centred 34.61 mm above it. The cone and reducer have the apex at the origin, seam ray on +x, ψ counter-clockwise, and for the reducer the seam at θ = 0 is the generator toward the offset. Units are mm. Every one of these agrees with my independently chosen frame once the outside test is applied. D1 is fixed, as claimed.

## 2. Conventions a fabricator relies on (work item 2)

"Stated" means stated in the result JSON or the DXF that leaves the system. Code comments and the specification don't count.

| # | Convention | In result? | In DXF? | Same across parts of a job / across families? | Product verifier | `validation/` | Risk |
|---|---|---|---|---|---|---|---|
| 1 | Marking face (pattern normal) | yes, "outside", provisional | yes (TEXT) | yes | yes: fold-back by declared side, saddle handedness, same-side check | yes (reference outside by construction, mirror tests) | physical choice unreviewed |
| 2 | Rolling axis / which edges are the seams | only as JSON curve names | **no** | **no**: header circumference along DXF **Y**; branch and stand-alone shell along DXF **X**. R 150 / L 1200 is 942.5 × 1200 as a shell and 1200 × 942.5 as a header | outline-extent check only (Defect 002 fix) | **no**: it tries every labelling of seam edges, so it cannot notice | **RB-003** |
| 3 | Seam position on the part | header: opposite branch; reducer: θ = 0 toward offset; branch: "φ = 0" | no | n/a | no (only seam-edge lengths) | adopted from the product | branch φ = 0 never described physically (RB-007) |
| 4 | Which end is which (header station-0 end; large/small end) | JSON edge "bottom" = x = 0 end (named like a circumferential edge) | **no** | — | no | observation only | low for symmetric runs |
| 5 | Direction of increasing angle, seen from a declared end (Spec §0.3) | no | no | implied by #1 | no | no | spec non-compliance (RB-007) |
| 6 | Plate origin | no | no | differs: shell corner; header end/seam corner; branch at the header-**axis** level (off the part); cone and reducer at the apex (off the part) | no | irrelevant to it (rigid fitting) | low (RB-009) |
| 7 | Station = foot of the common perpendicular on the header axis (axis work point), not the surface point | no | no | — | no | from spec/sketch | 85.8 mm difference to the surface piercing point for R 150, β 60°, δ 20 |
| 8 | s and s_end measured along the branch axis from the header axis | label in UI | no | — | no | from spec | — |
| 9 | Sign of δ relative to the lean of β | no; only an input sketch whose end-view direction is **unlabelled** (correct under third-angle reading) | no | consistent across parts (both mirror together) | **no**, undetectable | from spec + sketch | **RB-002**: a wrong sign gives a perfect mirror-image assembly, all checks PASS |
| 10 | Which radius is developed (inside/outside/neutral) and how thickness enters | assumption text: "reference-surface radii exactly as entered" | no | assembly/stub: one `surface_reference` for all radii; cone/reducer/shell: **no attribute at all** | BE-001 direction rule only; `unknown` disables it (R_dev = 10R accepted) | treats radii as exact | RB-010 |
| 11 | Contact surface of each part and joint type | "set-on (as entered)"; one curve for both parts | no | **one flag for header and branch** | interface check gets the **same** geometry for both parts, so it is blind | shares the single-curve model | **RB-001** |
| 12 | What the cut line is (part edge vs kerf centre); who compensates kerf | G11 says "not declared" | no | — | no | no | spec MATH-18 item open (RB-008) |
| 13 | Direction of the through-thickness cut (radial vs along the branch) | no (weld preparation "not modelled") | no | — | no | no | needs engineer (§6) |
| 14 | Units | yes | comment + TEXT + `$LUNITS 2` (R12 has no unit variable) | yes | yes | yes | fine |
| 15 | DXF axes ↔ plate (x right, y up, +z toward the viewer = marked face; outer CCW, holes CW) | via #1 | via #1 text | yes | G11 orientation | yes | fine |

## 3. Numerical stability (work item 3)

The errors below are measured against mpmath at 50 digits, at every product vertex. The true chord deviation was found with 64 samples per segment and then refined.

| Case (branch stub, R_dev = R, r_dev = r) | vertex error | ε_num stated | ratio | true chord dev. | ε_rep stated | ratio |
|---|---|---|---|---|---|---|
| R 150, r 149.999835 (gap 1.1e-6), β 45 | 1.6e-12 | 1.6e-11 | 0.10 | 0.1434 | 0.1431 | 1.002 |
| R 150, r 149.9985 (gap 1e-5), β 45 | 1.2e-12 | 1.6e-11 | 0.07 | 0.1960 | 0.1873 | **1.047** |
| R 150, r 99.9998, δ −50, β 15 | 1.0e-11 | 4.3e-11 | 0.23 | 0.3742 | 0.3682 | 1.016 |
| R 150, r 149.9997, β 90, s_end 160 | 1.4e-12 | 1.1e-12 | **1.27** | 0.0928 | 0.0913 | 1.016 |
| R 150, r 149.99978, β 90, s_end 151 | 2.0e-12 | 1.07e-12 | **1.88** | 0.0856 | 0.0850 | 1.007 |
| header opening, R 150, r 149.999835, β 45 (hn1) | 2.3e-12 | 8.3e-12 | 0.27 | — | 0.1431 | — |
| R 0.0015 scale model of the first row | 4.1e-17 | 1.6e-16 | 0.26 | 1.43e-6 | 1.43e-6 | 1.002 (exact scaling) |

- **ε_num.** It is still not a bound; I reproduced ×1.27–1.88. The estimate 16u(R + s_end)/sin β grows with s_end, which has nothing to do with the error, and has no term for the cancellation in R² − y². An a-priori bound that does hold is |Δs| ≤ [3u R²/√(R² − y²_max) + 4u(R + r)] / sin β (u = 2⁻⁵³). At the refusal limit √(R² − y²) ≥ R√(2·10⁻⁶), so the bound is ≈ 2.2·10³·u·R / sin β, i.e. 2.5–3e-11 mm at R = 150. It holds in every case above. The size is irrelevant to fabrication; only the label matters, and the label is honest ("estimate").
- **ε_rep.** The estimate is within 5 % of the truth and always below it (worst ratio 1.047). The error itself, however, is **proportional to size and not refined**. For R = 1000, δ −300 and β 15° the stated ε_rep is 0.035 / 0.067 / 0.21 / 0.66 / 1.89 / **2.41 mm** at gaps 1e-1 / 1e-2 / 1e-3 / 1e-4 / 1e-5 / 1.1e-6. G08 is INCONCLUSIVE whether the value is 0.001 or 2.4 mm. TOLERANCE_POLICY quotes only the R = 150 values. **Mitigating fact I measured:** both parts are cut by chords at the same parameters, and near tangency the two surfaces are nearly tangent, so the rolled DXF polylines of header and branch meet within **0.0026 mm (hn1) and 0.0016 mm (hn2)** although each lies 0.14–0.37 mm from the exact curve. Leaving ε_rep as an estimate is therefore acceptable for fit-up but not for the class "E-C exact". It is surfaced in JSON and UI, not in the DXF, and it should drive adaptive refinement (ALG-07) rather than only be reported (RB-004).
- **Refusals** are placed consistently: tangency |δ| + r ≥ R(1 − 1e-6) for stub and header alike; sin β ≤ 0.17; sin α < 1e-3 and ≥ 99.9 % of a turn for both conical families; magnitudes outside 1e-6…1e8 mm. R 1.5e6 with gap exactly 1e-6 R is refused at the boundary, as declared. Shallow β (15°) and obtuse β (115°, 170°) agree with my derivation. Scaling to 1.5e-3 mm is exact.

## 4. DXF fidelity (work item 4)

- **ezdxf**: `recover.readfile` and `doc.audit()` report 0 errors and 0 fixes on all 23 files written in this review. Entities are LINE, ARC and POLYLINE/VERTEX/SEQEND on `CUT_OUTER` / `CUT_INNER`, and TEXT only on `NF_NOT_RELEASED`. There are no zero-length segments and no duplicate vertices apart from the deliberate closing vertex. The smallest polyline segment is 0.65 mm at R 150. JSON and DXF vertices are bit-identical.
- **Closure.** The outer profile is a chain of 4 separate entities, or 3 lines plus a polyline. Shared end points are exact, and arc end points recomputed from degrees match within 1.4e-13 mm. Openings are POLYLINEs with **flag 70 = 0 (open)** and a repeated first vertex. Some CAM systems treat that as an open contour, so set flag 1 and drop the repeat. Outer profiles are CCW and openings CW, as declared.
- **Header and numbers.** `$ACADVER AC1009`, `$LUNITS 2`, `$AUNITS 0`, `$ANGDIR 0`. Extents include the annotation text and are exact (D5 fixed). Numbers are written as the shortest round-trip decimal without exponents, with at most 110 characters per line (the TEXT lines).
- **What the file does not carry:** part name; which edges are seams and which axis to roll about (RB-003); the station-0 end; the kerf convention; the ε_rep of the polyline.
- **Can the product's re-import check detect a wrong-but-self-consistent file?** No. It compares the parsed file with `to_entities(pattern)`, the same intended geometry the writer was given (this is exactly why Defect 002 passed it). It proves the writer and reader are faithful, nothing more. The "independent reader" of G15 lives in the same crate and was written by the same author. A DXF-only physical test (§1.1, rolling plus a proper rigid fit) would detect mirrored, transposed and mis-scaled parts without using any product parameter.

## 5. Independence of `validation/` (work item 5)

**Genuinely independent:** a generic ruled-surface developer (geodesic curvature, DOP853) instead of the apex integral; an oblique world frame; intersection by bisection; an mpmath tier; ezdxf plus its own parser; rigid fitting with and without mirroring. This is real independence of *numerics*, and it found D1.

**Shared with the product, taken rather than derived:**
1. Input meaning comes from Spec 02 and the product's input sketch: δ sign, φ datum, station as the axis work point, s from the common perpendicular. Spec 02 is itself at evidence level EV-1 and has not been independently reviewed. A common-mode misreading of the specification, or a specification that misdescribes workshop practice, would pass both. The report says so in §8.
2. Seam placement is adopted from the product's assumptions.
3. The joint model is the same: one (R, r) pair defines both the opening and the cut, so RB-001 cannot appear as a defect. It was recorded only as an observation.
4. The accept/refuse oracle encodes the product's own declared limits (`oracle.py` cites LEARNING_AND_STRESS_TESTING.md and `web/app.js`). It checks that the product does what it says, not that the limits are right; for example the 1e-6 R tangency limit, where ε_rep reaches 2.4 mm at R = 1000.
5. Mapping "for every admissible labelling of seam edges and plate ends" makes the comparison blind, by design, to the fact that the DXF doesn't say which edges are seams (RB-003).
6. Its float reference is less accurate than the product near tangency (5.8e-11 vs 1.0e-11 mm), so float-tier ε_num verdicts there are not meaningful.
7. It was built by the same organisation and the same tooling, on the same day, from the same documents; it is at evidence level EV-1 and has not been independently reviewed.

**What would make it stronger:** derive input semantics from physical reference drawings with engineer-confirmed ground truth (the intake specification already exists); model wall thickness and joint type (set-on / set-in) in the reference, so that contact-surface errors become visible; make the oracle's limits come from an engineering budget (ε_rep against a stated T), not from product documentation; add the DXF-only mating test; read the files with a CAD/CAM program (ODA/Teigha, a nesting package) rather than only Python; use interval arithmetic for the high-precision tier; have a different team or person own it.

## 6. What only a fabrication engineer or a physical trial can settle (work item 6)

Stated plainly: none of the following can be decided by any amount of computation in this repository.

- **Which surface the pattern is developed on**, and how thickness enters: neutral factor k, r_dev = r_i + k t (and t / cos α for cones), rolling behaviour, flat ends, weld gap and shrinkage. The product takes radii "as entered".
- **Marking side.** "Outside" is a software convention. Whether the workshop lays out on that face is unconfirmed.
- **Which wall surface defines each cut** for set-on and set-in joints, root gap, root face, bevel, and the variable bevel around a saddle. This includes whether the hole through a thick header wall is cut radially (what the flat-plate cut gives) or along the branch.
- **Kerf and who compensates it;** cutting taper and its reference face.
- **Seam placement** rules (clearance to openings, branch seam position), and allowances.
- **Whether the joint geometry in RB-001 is right for a given shop.** The review can only show that the product cannot express the specification's own set-on example with a declared surface.
- **CAM acceptance** of the DXF.

## 7. Findings

Severity follows the validation report: **high** = can produce a wrong part with no specific warning; **medium** = a stated guarantee or required convention does not hold in a way that could matter; **low** = wrong or unstated but harmless at fabrication sizes. Reproductions use the HTTP API of the release build (setup token, `X-NF-Request: 1`).

### RB-001 — Contact surfaces: one flag for both parts, one curve for both cuts; the set-on joint cannot be expressed (high)

**Status (20 Nov 2026): partly fixed.** Fixed for the specification's own set-on pairing (Spec 02 §4.4: branch INSIDE surface on header OUTSIDE surface). Each part declares its surface (`header_surface_reference`, `branch_surface_reference`), the joint is declared `joint: "set_on"`, and the opening is the branch bore (opening width 2 × 77.095 mm along the header axis, not the outside diameter). Each part's radii are checked against its own surface; every interface check passes, including the independent 3D point checks of the joint (`verify::verify_joint`). Tests: nf-core `rb001_set_on_joint_is_declared_per_part_computed_and_independently_checked`, `rb001_set_on_is_refused_unless_the_branch_bore_is_the_contact_surface`, `rb001_undeclared_joint_is_one_contact_curve_and_is_not_called_set_on`; regression `004-set-on-joint-bore-on-header-outside.json`; uem `rb001_parts_cut_for_different_surfaces_fail_the_interface_checks`. **Open:** a set-on whose saddle is cut on the branch OUTSIDE surface while the opening is the bore (two curves) cannot be expressed: there is no bore input and no wall thickness. Undeclared joints are computed as one contact curve and labelled as such. The workshop validity of the §4.4 pairing is unreviewed (EV-1).

- **Evidence.** DN300 header (OD 323.9, t 10.31) with a DN150 branch (OD 168.3, t 7.11), β 90°, δ 0.
  - Spec §4.4's set-on example, branch **inside** on header **outside**: `header_radius 161.95, header_dev_radius 156.795, branch_radius 77.095, branch_dev_radius 80.6`. Under `outside` it is REFUSED ("negative wall thickness" on the branch). Under `inside` it is REFUSED (header). Under `neutral` it is REFUSED. Under `unknown` it is accepted, all checks PASS, and the opening is 154.19 mm wide (the bore). So the correct joint is producible only by declaring the surface unknown, which also disables BE-001.
  - Natural input with both ODs, `surface_reference: "outside"`: `branch_radius 84.15, branch_dev_radius 80.6` (everything else as above). Accepted, all checks PASS, G10 and G15 PASS. The opening is **168.30 mm** wide, the branch OD, against a bore of 154.08. A set-on branch has no land and passes through the hole. The branch saddle depth is 23.58 mm instead of 19.50 (4.1 mm misfit at the flanks).
  - Both IDs, `inside`: the saddle is shaped to the header ID radius, so its depth is 21.03 mm against 19.50 needed on the OD (1.5 mm misfit).
- **Why nothing catches it.** `header_with_branch` calls `verify_interface(&header_pat, &geo, …, &branch_pat, &geo, …)` with the **same** `Branch` for both parts. The doc comment says the two geometries differ "if the two parts were made from different interpretations", but in a job they cannot. The interface record states "set-on (as entered)" while the geometry is whatever the single flag implies. This is the D1/Defect-002 pattern again: each part is correct in its own convention, and the convention is never compared with the joint the result claims.
- **Correction.** Make the surface a per-radius attribute (`header_surface`, `branch_surface`) and the joint type an input. For set-on, generate the opening from the branch bore and the cut from the declared contact pair, as two curves where they differ. Give the interface check geometries derived separately per part. Until then, refuse `outside`/`outside` and `inside`/`inside` for a job labelled set-on, or label the joint "zero-thickness idealisation", not set-on.

### RB-002 — The sign of δ (handedness of the whole assembly) is defined only by an unlabelled sketch (medium)

**Status (20 Nov 2026): open.** The sign of δ is stated in words in the drafted pattern conventions (uem `delta_notes`) and checked by the delta-side checks (uem `rb002_…`). The conventions are not yet written into the DXF or the result, and the projection method is not recorded.

- **Evidence.** asm1 with δ = +25 and with δ = −25 (otherwise identical): both jobs PASS everything. The two header plates are exact mirror images (2.5e-13 mm after v → W − v; 72.5 mm without). The opening centre is +34.61 mm and −34.61 mm from the plate centreline. +δ is +y of Spec §4.4, i.e. *away from the viewer of the side elevation*. The UI shows it only in an "end view along the header axis" without saying from which end. That view is consistent with the product under third-angle reading (view from the +x end) and mirrored under first-angle. Neither the result nor the DXF records the side of the offset relative to the lean.
- **Correction.** State it in words in the UI, the result and the DXF: "branch leans toward +x (higher station); +δ displaces it away from the viewer of the side elevation; projection: third angle". Record the projection method with the job (Spec 02 §2.1 already requires this, `FR-03-4`).

### RB-003 — Rolling axis and seam edges are not in the DXF and differ between parts of one job (medium)

**Status (20 Nov 2026): open.** Plate frames still differ between families (header circumference along Y, shell along X). The statement is drafted in the pattern conventions but not yet written into the DXF.

- **Evidence.** The same shell (R 150, L 1200) is 942.48 × 1200 as `cylinder_shell` and 1200 × 942.48 as the header of an assembly. In the assembly, the header DXF has its circumference along Y and the branch DXF along X. The DXF has no seam, axis or end labels; the JSON names are the only record, and the header's x = 0 end is named "bottom". For L ≈ 2πR_dev (e.g. R_dev 150, L 942.48) the plate is square and only those names tell the fabricator how to roll it. Rolled the other way, the opening is turned a quarter turn: Defect 002 recreated in the workshop.
- **Correction.** Use one plate frame for every shell family. Put seam edges, the rolling axis and the station-0 end on a non-cutting `REFERENCE` layer, or in TEXT ("ROLL ABOUT X; SEAMS = TOP AND BOTTOM EDGES; STATION 0 = LEFT END"). Add a check that each DXF states it.

### RB-004 — Representation error is uniform-sampled, scale-proportional and not gated (medium)

**Status (20 Nov 2026): partly fixed.** Polyline density refined until the measured chord error is at most 0.01 mm, a stated implementation limit; a part that cannot reach it is refused; both parts of a joint share one segment count, so their vertices coincide (test `rb004_polyline_density_follows_the_stated_maximum_chord_error`). **Open:** vertices are not placed at the tangent parameters; the chord limit is absolute, so the vertex count depends on size (see COMPUTATIONAL_VALIDATION_REPORT §9); no engineering budget.

- **Evidence.** §3: 2.41 mm at R 1000, δ −300, β 15°, gap 1.1e-6; 0.66 mm at gap 1e-4. The estimate is at most 4.7 % below the truth (R 150, gap 1e-5: 0.1960 true against 0.1873 stated). G08 is identical for 0.001 mm and 2.4 mm. TOLERANCE_POLICY §6 quotes only R = 150 (0.1–0.2 mm). Mitigation: the measured fit-up gap between the two parts stays ≤ 0.003 mm.
- **Correction.** Adaptive sampling with vertices at the tangent parameters and refinement until ε_rep ≤ a declared budget (Spec ALG-07/09). Report ε_rep relative to size. State the scale dependence in TOLERANCE_POLICY.

### RB-005 — ε_num is still not a bound (low)

**Status (20 Nov 2026): partly fixed.** An a-priori bound on the rounding error of the saddle and the opening is in place (`s_error_bound`, `saddle_error_bound`, `opening_error_bound`; test `rb005_eps_num_of_a_branch_is_an_a_priori_bound`). **Open:** the bound is checked at five measured points, not over the range; the quick validation run did not reach the eps_num check of the near-tangent case N-tangent-d-b90-g2e-06, so D4 was not re-measured.

- **Evidence.** Stub R 150, r 149.99978, β 90°, δ 0, s_end 151: true 2.0e-12 mm against stated 1.07e-12 (×1.88). With r 149.9997 and s_end 160: ×1.27. The estimate scales with s_end, which does not affect the error.
- **Correction.** Use the a-priori bound in §3, [3u R²/√(R² − y²_max) + 4u(R + r)] / sin β, or interval evaluation. Either one makes the value a true bound at negligible cost.

### RB-006 — The product's own checks cannot see convention errors, and two of them are described more strongly than they work (medium)

**Status (20 Nov 2026): partly fixed.** Independent 3D formulation in `uem/src/independent.rs`, used by `verify.rs` (tests `rb006_independent_checks_detect_faults_without_the_fold_back`, `rb006_independent_checks_pass_across_the_accepted_range`); G15 text and the `reimport_check` documentation no longer claim independence they do not have. **Open:** the verifier is not owned or written by another person; the DXF-only mating test (roll and fit from the files alone) is not implemented.

- **Evidence.** The interface check gets the same geometry for both parts (RB-001), and its doc comment says otherwise. G15 says "re-imported by the independent reader and compared with the intended geometry", but the reader is in the same crate, by the same author, and the comparison target is the product's own entity list. The fold-backs (`fold_branch`, `fold_header`) are the algebraic inverses of the constructions. A mirrored, transposed or wrongly-sided but self-consistent result passes them all; both earlier defects were found from outside.
- **Correction.** Add a DXF-only physical mating test (roll each file by its declared side and roll axis, proper rigid fit, no product parameters). Have the verifier owned and written separately. Reword G15 and the doc comment.

### RB-007 — Angular datum and direction not declared physically (Spec §0.3) (low)

**Status (20 Nov 2026): open.** The angular datum and direction sentences are drafted in the pattern conventions, not yet written into the DXF or the result.

The branch seam is "phi = 0", never described as "the generator on the +δ side, 90° from the crotch–heel plane". The direction of increase as seen from a declared end is stated for no family. Only the marking side fixes it implicitly. **Correction:** one sentence per family in the result and the DXF.

### RB-008 — DXF: open-flagged closed polylines, no kerf convention, no labels (low)

**Status (20 Nov 2026): partly fixed.** Openings are written as closed polylines (flag 70 = 1) without the repeated vertex (nf-core `to_entities`); the quality gate checks the flag (nf-dxf `rule_open_contour_gaps_missing_edges_and_unflagged_openings`). **Open:** the kerf statement is not yet written into the DXF, and the outer profile is still a chain of separate entities.

The opening POLYLINE has flag 70 = 0 and a repeated first vertex. The outer profile is a chain of separate entities. Kerf ownership is not declared in the file (MATH-18 is a release item in the specification). **Correction:** set the closed flag (70 = 1) without the repeated vertex, optionally write the outer profile as one closed polyline, and add a TEXT "GEOMETRY = FINISHED PART EDGE, KERF NOT APPLIED".

### RB-009 — Plate origins differ between families and are unstated (low)

**Status (20 Nov 2026): open.** Plate origins are stated in the drafted conventions, not yet in the DXF or the result.

The branch origin sits at the header-axis level (s = 0, off the part); cone and reducer at the apex; shell and header at a corner. This is harmless for CAM nesting, but confusing for manual layout from coordinates. **Correction:** state it, or normalise to the bounding-box corner.

### RB-010 — `unknown` surface disables every radius check; cone, reducer and shell have no surface attribute at all (low)

**Status (20 Nov 2026): open.** Under `unknown` the radius checks are still switched off; cone, reducer and shell still carry no surface attribute.

R_dev = 10 R is accepted under `unknown` (validation observation 3). Cone, reducer and shell radii are "reference radii as entered", with no declaration of what the user meant by them. **Correction:** require a surface declaration on every family; under `unknown`, at least warn when R_dev / R is outside 0.9…1.1.

### RB-011 — `validation/` shares the specification's semantics and the product's limits (medium; about the validation, not the product)

**Status (20 Nov 2026): open.** `validation/` is unchanged. Its quick run on this build disagrees on four cases; two are caused by the harness (see COMPUTATIONAL_VALIDATION_REPORT §9). Its shared semantics and limits are not addressed.

See §5. Agreement between product and validation (0 disagreements in 1,362 cases) is strong evidence about numerics and frames as the specification defines them. It is no evidence that the specification's input meaning, seam placement, joint model or limits are right.

## 8. Reproduction notes

The server was a private copy (`NF_SETUP_TOKEN` 24+ characters, port 8853, empty data directory). The jobs are listed in §1–§3 with their exact inputs. My scripts compare product JSON, ezdxf-parsed DXF, my mpmath derivation and (after my numbers existed) `validation/reference` in `threeway.py`. No product code, validation code or other agent's document was changed.
