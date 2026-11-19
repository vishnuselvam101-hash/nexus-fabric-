# Engineering / mathematics review: launch film (launch30/film.html)

Reviewer role: engineering, CAD and mathematical verification. Date: 2026-11-19.
Scope: every equation, number, shape and capability label in `film.html` (EN and JA), checked against the product code (`crates/uem`, `crates/nf-core/src/job.rs`, `crates/nf-dxf`), Spec 02 eq. (4.3) and `docs/IMPLEMENTATION_STATUS.md`.

## Method

1. **Read the code**: `uem::develop` (`Branch::s`, `branch_stub`, `header_with_opening`), `uem::verify`, `uem::profile`, `nf_core::job` (`HeaderWithBranch`, layer mapping, DXF, re-import), `nf_dxf` (writer and reader), and the film script.
2. **Computed independently** in Python with mpmath (30 digits, adaptive quadrature). These are exact curve lengths, not polylines: the 3D length of the intersection curve, the developed saddle length on the branch, the developed opening perimeter on the header, and s(φ) at key angles.
3. **Got the product's own numbers**: a throw-away Rust binary in the scratchpad, linked to `nf-core`/`uem` by path, ran `nf_core::job::run(HeaderWithBranch{…})`. It printed quantities, per-curve lengths and layers, checks, and the layer names in the actual DXF text. The build output was deleted afterwards and no product file was touched. `cargo test -p uem -p nf-dxf -p nf-core`: all pass.
4. **Read the film's own values** through `window.LENGTHS`, rendered stills, and checked the shapes on the pixels (symmetry of the opening, where the pattern's valley sits).

Inputs, the same as the product's assembly test case `job.rs::tests::assembly()`: R = 150, R_dev = 150, r = 100, r_dev = 100, β = 60°, δ = 0, header length 1200, station 600, s_end = 400. The film uses `S_END = 4.0` (400 mm), so the stub length matches.

## Values: independent vs film vs product

| Quantity | Independent (exact) | Film before | Product (job output / DXF) | Film after |
|---|---|---|---|---|
| s(0) = s(180°) | 129.099445 | — | s_at_0 129.099445 | — |
| s(90°) | 115.470054 | — | s_at_90 115.470054 | — |
| s(270°) = s_max | 230.940108 | — | s_max 230.940108 | — |
| 2πR | 942.477796 | 942.478 | 942.477796077 (bottom/top) | 942.478 |
| 2πr | 628.318531 | 628.319 | 628.318530718 (square_end) | 628.319 |
| Header length | 1200 | 1200 | 1200 (seam_a/b) | 1200 |
| 3D intersection-curve length | 701.665093 | — | — | — |
| Header opening (CUT_INNER, `branch_opening`) | 701.665093 | **701.665** | **701.662797** (opening_perimeter; 720-chord polyline) | **701.663** |
| Branch `saddle_cut` (one curve of CUT_OUTER) | 701.665093 | **701.665** labelled "CUT_OUTER" | **701.664228** | **saddle_cut 701.664** |
| Branch seam_a = seam_b | 270.900555 | — | 270.900555126 | — |
| Whole branch CUT_OUTER perimeter | 1871.784734 | (not shown) | 1871.783869 | (not shown) |
| Whole header CUT_OUTER perimeter | 4284.955592 | (not shown) | 4284.955592 | (not shown) |
| Opening x-range rel. station | −28.868 … 202.073 | same formula | 571.132 … 802.073 (station 600) | — |
| Opening half-width (developed) | 109.459 | same formula | v 361.780 … 580.698, centred on πR = 471.239 | — |

Layer names in the product's DXF text: header `CUT_INNER`, `CUT_OUTER`; branch `CUT_OUTER` only (`job.rs::layer_name`). The film's layer names match.

## Verdict per item

### 1. Equations and printed numbers: correct after corrections

- **s(φ) formula**: identical to Spec 02 eq. (4.3) with δ = 0 ("zero offset" in the film means δ = 0) and to `Branch::s`. Correct.
- **2πR = 942.478, 2πr = 628.319, 1200, ⌀300, ⌀200, 60°, R150/r100/60°**: correct.
- **Live read-out (scene 3)**: s was evaluated at the exact animation angle, while φ was printed rounded to 0.1°. The ds/dφ slope is about 1.39 mm/°, so the printed pair did not reproduce. At t = 16.5 the film showed "193.8°, 145.84 mm", but s(193.8°) = 145.78. **Corrected**: s is now evaluated at the printed angle, and the frame shows 193.8°, 145.78 mm.
- **Cut lengths "701.665"**: this is the exact length of the curve. The product does not print it. It reports the length of its 720-chord DXF polylines: 701.663 for the opening and 701.664 for the saddle. The difference is chord shortening, within the product's reported eps_rep. The panel is labelled "computed by the product's mathematics engine", so the film now computes the lengths with the product's discretisation: 720 chords, uniform in φ, as in `develop.rs`. That reproduces the product to 1e-12 (`window.LENGTHS` open = 701.6627968062375, saddle = 701.664228095528).

### 2. Concern A (branch "CUT_OUTER 701.665"): confirmed misleading, corrected

In the product, the branch's CUT_OUTER layer is the whole closed outline: `square_end` 628.319 + `seam_b` 270.901 + `saddle_cut` 701.664 + `seam_a` 270.901 = **1871.784 mm**. 701.66x is only the `saddle_cut` curve, the developed intersection curve. On the header, by contrast, CUT_INNER is a single closed polyline (`branch_opening`), so "CUT_INNER 701.663" correctly names a whole cut.

The film's label now reads `CUT_OUTER` / `saddle_cut 701.664`. That is the product's own curve name, and the line is identical in EN and JA.

The opening and the saddle are both developments of the same 3D curve. Development is an isometry, so their exact lengths are equal (701.665093), which is a nice fact. The 0.001 difference in the printed values is only the polyline discretisation.

### 3. Concern B (724.766 / 705.435): explained, different inputs, no product defect in the mathematics

These captures come from **Engineering job #3** (`assets/app-pattern-header.png`, "corrects job #2"). Its inputs are header_radius **142** (contact radius), header_dev_radius **150**, and otherwise the same values. With those inputs:

- Branch `saddle_cut`: independent value 705.436155, product 705.435199 (720 chords). The capture shows **705.435198684**, a match.
- Header `branch_opening`: independent value 724.767953, product 724.765552. The capture shows **724.765551875**, a match.

The opening (724.77) is longer than the saddle (705.44) in that job because the header contact curve on the r = 142 surface is projected radially to the development radius 150. The developed v-coordinate becomes 150·ω, while x is unchanged. That is consistent: a neutral-radius development of the same contact curve, with both checks and the interface check passing. So there is no defect in the film and no defect in the mathematics.

**Observations for the lead (not defects in the computation):**

- (a) Job #3 declares `surface_reference = "outside"` while the contact radius is 142, smaller than the development radius of 150. An outside surface cannot have a radius smaller than the neutral radius. The product records the declaration (G02 INCONCLUSIVE) but does not check it against the radii. That is a missing plausibility check, worth an issue.
- (b) Scene 2 of the film (t ≈ 8–10.7) shows this job #3 capture: saddle_cut 705.435198684, R142. Scenes 3–4 then show R150 with 701.66x. A careful viewer could notice the two different assemblies. Either accept it, since it is a different job and real software, or recapture scene 2 from an R150 job.

### 4. Shapes: correct

- **Opening on the flat header plate**: same formula as `header_with_opening`: x = s cosβ − r sinφ sinβ, u = R·asin(r cosφ / R). The longitudinal seam is opposite the branch (the plate edges at u = ±πR), as in the product. The opening is **symmetric about the axial line u = 0**: a pixel mirror test on the 23.5 s still gave a 100 % match, and the product's hole is centred on v = πR. It is asymmetric along the axis, as it should be for β = 60°. The blunt end lies toward −x (x = −28.9) and the narrower end toward +x (x = +202.1, the side the branch leans to). The widest point is at x = +64.5. Size: 231.0 × 218.9 mm.
- **Branch pattern**: the seam is at φ = 0, as in the product. The top view shows the pattern **mirrored left–right** compared with the product's DXF/app preview: its deep valley (s_max at φ = 270°) is at 25 % of the width instead of 75 %. This is the same plate seen from the other face. Because s(φ) = s(π − φ) when δ = 0, it is also exactly the product pattern with the seam on the opposite flank (φ = 180°, same seam length 270.9). The geometry is valid; it just does not match the app's orientation. This is a lead decision (not a number or label, so I did not change it).
- **3D view**: header points at k = 0 are (x, R sin(u/R), R cos(u/R)). Branch points from `brPt` with w = r(φ − π) reduce algebraically to `Branch::point` in the product. The opening and the saddle therefore coincide in 3D, and the blue curve lies on both surfaces.
- **Scene 1 fictional drawing**: the elevation and the end view use the same scale and are placed consistently (third-angle, end view to the right). Checked: 1200 long, ⌀300 in both views, branch ⌀200 at 60° to the header axis, branch axis through the header axis at mid-length (station 600). The branch outline generators meet the header top edge at x = −28.9 and +202.1 mm, as computed. The intersection curve dips 38.2 mm below the top edge at φ = 0 (z = √(R² − r²) = 111.8). End view: the branch strip has width ⌀200 and the end ellipse has semi-axes 100 × 50 (r·cosβ). The title block reads R150 / r100 / 60°. All consistent.
- **One drafting nicety**: the header's top silhouette line is drawn continuously across the branch opening. Strictly, it ends at the two branch outline lines. This is cosmetic; I left it unchanged.

### 5. Capability labels: correct, with notes

| Film label | Status doc / code | Verdict |
|---|---|---|
| Implemented · Pattern mathematics (eq. 4.3) | uem branch stub + header opening: INTEGRATED | OK |
| Implemented · Independent verification checks | `uem::verify`, INTEGRATED; class I-F only (same crate) | OK. "Independent" is the product's own term, and the app shows "I-F" |
| Implemented · Interface check between parts | `verify_interface`, INTEGRATED | OK |
| Implemented · DXF written, re-read and compared | `nf_dxf::write` + independent `read` + `reimport_check` (gate G15) | OK |
| Implemented · Actual software (scene 2 captures) | real app captures | OK |
| Planned · Drawing reading and 3D reconstruction | Drawing intelligence NOT STARTED | OK |
| Planned · Manufacturing-constraint evaluation | not implemented | OK |
| Planned capability · concept (scene 3, 3D model) | 3D visualisation NOT STARTED | OK |
| Demonstrated · computed by the product's maths engine · redrawn for this film (scene 4) | numbers now equal product output | OK after the fix |
| Disclaimer "Not validated for manufacturing" | all jobs REVIEW REQUIRED, G16 absent | OK |

Notes for the lead:

- The headline and VO line "Traceable CAD deliverables" fit only in part. Record-level traceability exists (pattern → DXF record, parameter claims). Per-entity traceability is IN DEVELOPMENT / NOT STARTED.
- `IMPLEMENTATION_STATUS.md` contradicts itself: line 111 lists "inner profiles" as NOT STARTED, while line 63 lists the header opening (inner profile, CUT_INNER) as INTEGRATED. The code has it, so this is a documentation error, not a film error.

## Corrections made to film.html (3 lines; layout and style unchanged; EN and JA share the code)

1. `LEN.open` / `LEN.saddle` are now computed with `polyLen(…, 720)`, the product's 720-chord discretisation (comment added). Printed values: 701.663 (CUT_INNER) and 701.664 (saddle).
2. Branch number line: `701.665` changed to `saddle_cut 701.664`, under the existing `CUT_OUTER` line.
3. The scene 3 read-out evaluates s(φ) at the angle as printed (rounded to 0.1°).

Re-rendered and inspected: `stills/r06/en-16.5.png`, `en-23.5.png`, `ja-16.5.png`, `ja-23.5.png` (13 and 20 also re-rendered, unchanged). Console errors: 0. The draft MP4s were **not** re-rendered.

## Not verified

- No CAM system import of the DXF was done (the product has not done one either).
- Spec 02 eq. (4.3) itself was taken as given. Its derivation was checked only in the sense that the curve lies on both cylinders (residuals ~1e-13).
- The narration audio and subtitles were not checked against numbers; they contain none.
