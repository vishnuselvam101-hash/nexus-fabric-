# Computational validation report

## 0. Re-run on engine `nf-engine 0.3.0` (commit 4c05456), 19 November 2026

Sections 1–8 below describe the first run, against the build of 02:38. This section is the current state. The reference (`validation/reference/`) was **not changed** between runs: it draws every pattern as seen from outside by construction (pattern normal = outward surface normal), and the product is mapped onto it with and without mirroring. Only the harness was adapted to the new contract.

**Result: 1,362 cases — 925 agree, 0 disagree, 437 refused correctly, 0 wrongly accepted, 0 wrongly refused.** The run still exits non-zero because D4 is open.

| Defect | State on 0.3.0 | Evidence from the independent run |
|---|---|---|
| D1 marking side | **fixed** | every part declares `marking_side: outside` in the result and in a DXF text line; in all 301 assemblies compared the header and the branch match the outside-view reference without mirroring (174 asymmetric joints: outside only, both parts); folded from the same face the saddle meets the opening within 4.8e-11 mm. The R150 / r100 / β60 / δ20 case: 2.6e-13 mm outside/outside; 114 and 57 mm for the mixed combinations |
| D2 non-finite output | **fixed** | 1e308 and 2e8 mm inputs refused (HTTP 422, state REFUSED); no accepted job contains a non-finite number |
| D3 `eps_rep` low near tangency | **fixed as an estimate** | largest ratio true / stated 1.052 (was 1.21); the harness now reports a ratio above 1.10. The error itself still reaches about 0.2 mm at R = 150 next to tangency; it is reported, not reduced |
| D4 `eps_num` exceeded | **still reproduces** | 11 parts in 6 near-tangent cases, factors 1.1…6.1, absolute ≤ 1.4e-11 mm. Relabelled an estimate; the value is unchanged (the product's own register lists it as open) |
| D5 DXF extents | **fixed** | header extents equal the cut-geometry box on the sides not occupied by the marking text, in every cone |
| D6 cone closing on itself | **fixed** | right cones refused from 99.9 % of a turn, as reducers are |
| D7 JSON arc points | **fixed by labelling** | each arc carries `points_exact: false` and a `points_max_deviation` not smaller than the measured sagitta |
| D8 scale dependence | **fixed inside the declared range** | exact scaling from 2^-26 to 2^16 of the test parts; outside 1e-6…1e8 mm every request is refused |

Nothing new was broken: no geometry difference from the reference appeared in any accepted job.

Contract changes absorbed by the harness (expectations changed only where the contract changed):

- first-run setup token (`--setup-token` or `NF_SETUP_TOKEN`; `--spawn` generates one);
- an engineering refusal is HTTP 422 with state REFUSED (291 cases); malformed input and unknown fields are HTTP 400 (146);
- magnitudes outside 1e-6…1e8 mm and development radii impossible for the declared surface are refusals; the oracle, the workload generator and 17 new corpus cases follow (corpus now 518);
- DXF: both readers accept non-cutting TEXT on layer `NF_NOT_RELEASED` only; any other entity off the `CUT_*` layers, any non-line/arc/polyline on them, and any change to cut geometry fails. `seeded_faults.py` plants five such tamperings (vertex moved 1e-6 mm, extra line on a cut layer, on the marking layer, on a new layer, opening moved to the outer layer) and all are reported;
- new checks: declared marking side against the side the geometry is really drawn from, agreement of result and DXF, same side on all parts, mating from the declared side. The planted D1 (mirrored saddle) and Defect 002 are both reported.

Observations that remain: a real set-on joint (branch inside surface on header outside surface) can only be declared `unknown`, because one surface reference covers all radii; with `unknown`, development radii of any size are still accepted (R_dev = 10 R passes); the station is the axis work point and the DXF does not label the station-0 end; the marking side is a provisional convention that no fabrication engineer has confirmed.

CI command (about 2 minutes, 155 cases, includes the Defect 002 and D1 detectors, the DXF tampering tests and every closed-form case):

```
pip install -r validation/requirements.txt && python3 validation/run_all.py --spawn target/release/nf-server --quick
```

It fails today on D4 only (case `N-tangent-d-b90-g2e-06`).

---

19 November 2026 · suite in `validation/` · raw results in `validation/results/latest.json`

**Build tested:** the `nf-server` release binary copied from the shared build tree at 02:38 on 19 November (engine `nf-core 0.1.0 / uem 0.1.0`, after the Defect 002 fix), driven through its HTTP API on a private data directory. Product code was not edited. Anything changed in the source tree after that copy is not covered.

## 1. Result in brief

- 1,336 cases run. **No geometric disagreement**: every accepted pattern matches the independent reference in shape, position, seam location and size, and every DXF matches its JSON result.
- **One high-severity defect found**: the two parts of a header-with-branch assembly are drawn from opposite faces of the plate (header as seen from outside, branch as seen from inside), and no output states a marking side. For offset oblique branches, marking both patterns on the same face gives a branch that does not fit. The product's own interface check cannot see this, for the same reason it could not see Defect 002.
- Seven lower-severity defects and five observations (section 5).
- The validator detects a re-created Defect 002 (`validation/seeded_faults.py`).

## 2. Method, and why it is independent

Nothing in `validation/reference/` is taken from the product's code. The product source was read only to learn its input names and where it puts its output; the conventions of the reference come from Specification 02.

| Aspect | Product | Reference |
|---|---|---|
| Cone and reducer development | closed form for the right cone; apex polar integral (4.1) for the oblique cone | one generic routine for any ruled developable: the directrix is developed by integrating its geodesic curvature (DOP853), rulings laid off as straight lines; no apex, no family formula |
| Second route | — | mpmath, 40 digits: sector angle from rim length, and separately as arc length of the rulings' spherical image; area as a 3D surface integral |
| Third route | — | triangulation unrolling with Richardson extrapolation (second-order convergence confirmed) |
| Intersection curve | explicit formula (4.3) in the frame header axis = x | header and branch cylinders placed in an oblique world frame (header axis (2,−1,2)/3); each branch ruling intersected with the header by bisection on the point-to-axis distance; mpmath tier uses a vector quadratic |
| Frames | fixed by construction | reference patterns are always seen from outside, at an arbitrary position; they are mapped to the product pattern by an explicit rigid motion fitted to corner points, tried without and with mirroring and for every labelling of seam edges and plate ends |
| DXF | own writer and reader | ezdxf and a separate minimal group-code parser; both must agree |
| Arithmetic | binary64 | binary64 (scipy, numpy, shapely/GEOS) and mpmath |

Before judging anything the reference checks itself (30 items, all pass): float developer against mpmath to 1e-11, apex-free ODE in mpmath against the apex route to 20 digits, triangulation against both, and the published values CONE-01, ECC-01, BR-03, BR-04.

Checks applied to every accepted job:

1. **DXF round trip** — entity count, type, layer (`CUT_OUTER` / `CUT_INNER`), vertices bit-for-bit against the JSON, arc points on the arc, lengths, SHA-256 against `dxf_sha256`, outline closure, outer loop counter-clockwise, opening clockwise and closed, polygon validity and opening inside the plate (GEOS), header extents, units.
2. **Point-by-point comparison** in both directions: every product vertex to the exact reference curve (numerical error), and dense reference samples to the product entity (true representation error, compared with `eps_rep`).
3. **Fold-back** — product points carried through the inverse mapping into 3D and tested by point-to-axis distances against the target surfaces; rolled seam edges must meet.
4. **Assembly mating** — the folded header opening and the folded branch saddle must coincide in 3D; evaluated for all four combinations of drawing side.
5. **Corpus expectations** — reported quantities and values measured from the DXF (areas, perimeters, bounding boxes, opening position in the plate frame) against frozen expected numbers.
6. **mpmath error test** (marked cases) — true error of product coordinates against stated `eps_num`.

Agreement tolerance for point positions: 2e-9 of the largest coordinate. For the numerically integrated reducer the product's stated `eps_num` is honoured above that, capped at 1e-6 of the size, and tested separately against mpmath.

## 3. Corpus (`validation/corpus/cases`, 501 files)

Each file holds the request, the expected outcome, expected values with tolerances, and how they were derived.

| Group | Cases | Content |
|---|---|---|
| closed_form | 9 | plain shell; cone with sin α = ½ (half annulus, 400 × 200 box, area 15000π); 3-4-5 cone (sector 216°); CONE-01; 90° tee with r/R = ½ (opening 325…475 by 150·5π/6…150·7π/6); BR-01/03/04 hand values |
| limit | 7 | e → 0 reducer against the cone (e = 0, 1e-9, 1e-6, 1e-3); flat-sided reducer; branch radius → small (opening → ellipse, centre at station + R/tan β) |
| high_precision | 22 | 6 cones, 8 reducers, 8 joints with mpmath values (17 digits stored; quantities compared to 2e-12 relative) and the mpmath error test |
| assembly | 172 | 162 joints over β = 12…168°, r/R = 0.05…0.99, δ = 0, ±½, 0.97 of the available offset, all four surface references; 10 with development radius ≠ contact radius, opening 0.01 R from either plate end, sizes ×0.01 and ×100 |
| symmetry | 39 | δ → −δ; β → 180° − β; station shift; stub = branch part of the assembly; scaling by 2, ½, 1024, 25.4, 1e-3, 1e3, ⅓ for five parts |
| refusal | 252 | zero, negative, −0 for every positive parameter; NaN, Infinity, 1e999, string, null, missing for every parameter; 1e308; r1 ≤ r2; near-cylindrical; e < 0; r ≥ R; partial and tangent intersection; β = 0, 180, −30, 200, 360, 600, 5, 175; square end inside the saddle; opening off either plate end; invalid surface reference |

Not constructible: an opening crossing the seam. The seam is fixed opposite the branch and there is no seam input; the suite measures the opening-to-seam clearance instead.

On top of the corpus: 300 seeded random requests (seed 20261119, 60 per family, sizes 0.01 mm…100 m, about a third deliberately invalid or out of range) judged by an independent accept/refuse oracle; 400 parts from the workload generator; 135 stability cases.

`validation/workload_gen.py` produces spool-style parts lists of any size (pipe sizes from a table of common outside diameters, headers with one job per branch, shells, reducers, stubs, repeated and mirrored parts, optional invalid parts) as JSON Lines with the expected outcome on each record.

## 4. Results

| Group | Run | Agree | Disagree | Refused correctly | Wrongly accepted | Wrongly refused |
|---|---|---|---|---|---|---|
| closed_form | 9 | 9 | 0 | 0 | 0 | 0 |
| limit | 7 | 7 | 0 | 0 | 0 | 0 |
| high_precision | 22 | 22 | 0 | 0 | 0 | 0 |
| assembly | 172 | 172 | 0 | 0 | 0 | 0 |
| symmetry | 39 | 39 | 0 | 0 | 0 | 0 |
| refusal | 252 | 0 | 0 | 250 | 2 | 0 |
| random sweep | 300 | 201 | 0 | 99 | 0 | 0 |
| workload | 400 | 381 | 0 | 19 | 0 | 0 |
| stability | 135 | 119 | 0 | 16 | 0 | 0 |
| **Total** | **1,336** | **950** | **0** | **384** | **2** | **0** |

"Agree" means the geometry agrees; 250 of these cases still carry a defect finding from section 5 (178 of them the drawing-side conflict). The run therefore exits non-zero. In the workload group the full geometric comparison ran on every fourth part (96 accepted parts); the rest were checked for the correct accept/refuse outcome and finite output.

Largest deviations over all accepted jobs, relative to part size: product vertex to reference 1.4e-10 (cylinder 7e-17, reducer 2.3e-9); rolled seam gap 3e-9; assembly mating 1.9e-7 mm absolute at worst. Scaling by a power of two reproduces the pattern bit for bit; other factors to 4e-16.

Refusals: 145 malformed requests answered HTTP 400; all others stored as a job with no pattern, no DXF (404) and a reason naming the parameter.

## 5. Defects and observations

Severity: **high** = can produce a wrong part without any warning; **medium** = a stated guarantee does not hold in a way that could matter; **low** = wrong but harmless at fabrication sizes.

### D1 — Header and branch patterns are drawn from opposite faces; no marking side is stated (high)

- **Evidence.** In all 312 assemblies compared, the header pattern matches the reference only as seen from outside (or both ways when symmetric), the branch pattern only as seen from inside. In 178 of the 179 assemblies that are both offset (δ ≠ 0) and oblique (β ≠ 90°), folding both patterns from the same face leaves the saddle off the opening; folding header-outside with branch-inside closes to 1e-13 mm. The stand-alone branch stub has the same orientation (43 of 43 asymmetric cases).
- **Reproduce.** `{"family":"header_with_branch","header_radius":150,"header_dev_radius":150,"header_length":1200,"branch_station":500,"branch_radius":100,"branch_dev_radius":100,"beta_deg":60,"delta":20,"s_end":400,"surface_reference":"outside"}` → `mating_3d`: outside/outside 114 mm, inside/inside 57 mm, header outside / branch inside 2.6e-13 mm.
- **Size of the error.** A stub rolled from the mirrored pattern, then turned and slid to its best position, leaves a root gap of ±27 mm for this case; ±6.9 mm at δ = 5, ±1.4 mm at δ = 1, ±57 mm at R 150 / r 100 / β 45° / δ 40 (`validation/analysis_view_side.py`). Zero when δ = 0 or β = 90°.
- **Which side is wrong.** The product. Specification 02 §0.3 requires the pattern normal to point to a *declared* marking side "because mirroring a pattern is a classic fabrication error"; the result and the DXF declare none, and the two parts of one assembly disagree. The branch also departs from §4.4, which gives the pattern as (r·φ, s): the product draws (r·φ, s_end − s), the mirror image.
- **Why internal checks miss it.** The interface check folds each pattern back with the convention it was built with. Same class as Defect 002.
- **Limit of this finding.** The validator can show that the sides differ and that neither is declared; it cannot say which face a workshop marks on.

### D2 — Absurd lengths are accepted and return non-finite numbers (low)

- `{"family":"cylinder_shell","r_dev":500,"length":1e308}` → state REVIEW_REQUIRED, DXF served, `area: null`, `eps_num` 8.9e292, check `area` INCONCLUSIVE. Same for `header_length: 1e308`, and for a shell scaled by 2^505 or more. Expected: refusal. These are the two "wrongly accepted" cases.
- Nine further 1e308 requests are not refused at input: a pattern with null numbers is computed and stored, then rejected by gate G11 and its files withheld. Fail-safe, but an input limit is missing, and each such case is recorded as an engine failure.

### D3 — `eps_rep` is not a bound near tangency, and the representation error grows to 0.2 mm (medium)

- The stated value is the chord deviation at the middle parameter of each segment; the largest deviation lies elsewhere when the curve has a sharp knee. Measured against the exact curve with 60,001 samples: R 150, β 45°, r = R(1 − 1e-5): true 0.196 mm, stated 0.189; r = R(1 − 1.1e-6): true 0.143, stated 0.119 (×1.21). Everywhere else in the run the ratio is 0.91…1.00.
- The fixed 720 segments are not refined toward the tangent points, so the error itself rises from 0.003 mm at r/R = 0.9 to 0.03 mm at 0.999 and 0.1–0.2 mm at 0.99999 (R = 150). Specification §4.4 asks for vertices placed at these points. The value is reported, but no gate acts on it.

### D4 — `eps_num` is exceeded in ill-conditioned cases (low)

- Near tangency (gap 2e-6 R): true error up to 1.4e-11 mm against a stated 2.9e-12…3.5e-12 (×3.4…6.1), 11 parts in 6 cases. Cause: cancellation in R² − y² and the arcsine near ±90°, not included in the estimate.
- Very flat, strongly oblique reducer `{"family":"eccentric_reducer","r1":5.90942068969116,"r2":2.061326520132905,"h":0.024581416189856813,"e":12.239738052541922}`: developed angle 1.2792885609 against 1.2792885578 (mpmath); true error 1.26e-7 mm against a stated 6.5e-8 (×1.9). The fixed quadrature does not resolve the peak of the integrand.
- In all 22 ordinary high-precision cases and the other 56 stability cases tested the stated value holds (ratio 0.00…0.92). Absolute sizes are far below fabrication relevance; the product already labels the value "estimated".

### D5 — DXF `$EXTMIN` / `$EXTMAX` are wrong for cones (low)

The header gives the box of the full circles (±L1 around the apex), not of the sector: CONE-01 says −632…632 in both axes; the true extents are −255.5…632.5 by 0…632.5. All 61 cone DXFs in the run. Software that reads the header for nesting or zoom sees a part up to twice its size.

### D6 — Right cones are accepted when the pattern closes on itself (low)

A cone's sector may come arbitrarily close to a full turn: r1 300, r2 50, h 0.01 → seam edges 2.5e-7 mm apart; h 1e-8 → 1e-14 mm. The eccentric reducer refuses at 99.9 % of a turn ("would overlap"). Nine cases, including tiny small-end radii. The two families should apply the same rule.

### D7 — JSON `points` of arcs are a 96-chord approximation while `eps_rep` is 0 (low)

The DXF arc is exact. The JSON polyline deviates by 0.034 mm for CONE-01 and by up to 26.8 mm for the largest cones in the sweep (relative 3e-5). Anyone drawing or measuring from the JSON gets an unlabelled approximation.

### D8 — Absolute tolerances make acceptance depend on scale (low)

The same shape is accepted or rejected by the product's own gates depending on size: all families fail the profile gate below about 2^-40 of the test sizes (≈ 1e-10 mm); the cone and the assembly fail the verification gate above about 2^24 (≈ 3e9 mm). Inside 0.01 mm…100 m, the range the stress campaign claims, scaling is exact.

### Observations (no defect claimed)

1. **Station is the work point.** `branch_station` is where the branch axis meets the header axis. The opening centre lies at station + R/tan β (1050 for station 50, R 1000, β 45°). Consistent with the input sketch, but not stated in the result, and the DXF does not label which plate end is station 0.
2. **Two frames for the same plate.** `cylinder_shell` puts the circumference along x; the header plate puts the axis along x.
3. **Development radii are unconstrained.** `header_dev_radius = 10 × header_radius` with `branch_dev_radius = 0.1 × branch_radius` is accepted with all checks PASS. Without a thickness model the product cannot test plausibility; a warning would be cheap.
4. **Refusals carry state REVIEW_REQUIRED**, like a result awaiting review, although nothing was produced (222 of 239 engineering refusals).
5. **Units** exist only in a DXF comment (R12 has no unit variable); `s_min` / `s_max` are extremes of the sampled points, not of the curve (difference below 1e-5 R).

## 6. Numerical stability

| Region | Finding |
|---|---|
| Near-tangent, r → R and \|δ\| → R − r, β 90° and 45°, gaps 1e-1…1.1e-6 | all accepted cases agree with the reference; gaps ≤ 1e-6 R refused as declared; D3 and D4 apply in the last two decades |
| Shallow angles | accepted from 9.8° to 170.2° (limit sin β > 0.17); agree; true error ≤ 0.03 of `eps_num` |
| Near-cylindrical cones | accepted down to sin α = 1.0001e-3, refused at 0.9999e-3; agree; true error ≤ 0.15 of `eps_num` |
| Nearly flat cones and reducers | cone accepted to h = 1e-8 (D6); reducer refused from h ≈ 1 |
| Very oblique reducers, e up to 1e6 | agree; `eps_num` holds (0.41 at e = 1e6, true error 1e-5 mm on a 1e6 mm part) |
| Extreme aspect ratio (r 0.01 × L 1e6, r 1e6 × L 0.01, stub 1.5e8 long, header 1.5e9 long) | agree |
| Magnitude 2^-1040…2^1010 | exact scaling where accepted; D8 and D2 outside |

## 7. Not implemented — nothing to validate

| Capability asked for | Status |
|---|---|
| Structural frames | not implemented — nothing to validate |
| Trusses | not implemented — nothing to validate |
| Beam / column connections | not implemented — nothing to validate |
| Structural analysis | not implemented — nothing to validate |
| Load paths | not implemented — nothing to validate |
| External drawing / DXF import | not implemented — nothing to validate (a file upload is stored; no geometry is read from it) |
| Multiple openings per plate | not implemented — nothing to validate (one branch per header job) |

Also declared unsupported and refused: equal-diameter and tangent tees, partial intersections, oblique cylinders, near-cylindrical cones, grazing branches, thickness, bevels, kerf and weld gaps.

Any future structural capability must be classified before it is validated, because each class needs a different kind of evidence:

| Class | What it claims | Evidence needed |
|---|---|---|
| Geometry generation | member lengths, cut angles, hole positions | independent geometric reference, as in this report |
| Structural modelling | idealisation into nodes, members, supports, releases, loads | review of modelling assumptions by a structural engineer; no numerical test settles it |
| Numerical analysis | forces, displacements, stability | benchmark problems with known solutions, equilibrium and energy checks, comparison with an independent solver |
| Code-compliant verification | a member or connection satisfies a named design code | clause-by-clause worked examples from the code body, version control of the code edition, sign-off by a licensed engineer |

A result in one class must never be presented as evidence for another.

## 8. What remains unverified

- **Physical correctness.** Thickness, neutral surface, bevel, kerf, weld gap and rolling behaviour are not modelled by the product and not tested here. Reference and product both treat the entered radii as exact.
- **The specification itself.** The reference takes the meaning of the inputs (which side δ is positive, where φ starts, what the station is) from Specification 02 and the input sketch. If the specification misdescribes workshop practice, both sides share the error.
- **Seam placement** is the product's stated assumption (opposite the branch; θ = 0 for reducers), adopted as given.
- **Which marking side is correct** (D1).
- **CAM acceptance** of the DXF; only ezdxf and a minimal parser were used.
- **Certified bounds.** The mpmath tier is high precision, not interval arithmetic.
- **Float reference resolution** is about 2e-9 of part size for reducers; smaller product errors there are judged only by the mpmath test, which ran on 22 corpus and 62 stability cases.
- **Workload group**: full comparison on one part in four.
- **Defect 002 itself** was re-created by altering a correct result inside the validator (`seeded_faults.py`, 7 planted faults, all reported); the pre-fix binary was not available to run.
- **Other layers**: storage, replay, corrections, obligations, permissions and the web client were not examined.
- **Later builds**: the source tree was being changed during this work; the run must be repeated on the fixed build.

## 9. Quick run on engine `nf-engine 0.4.0` (20 Nov 2026)

Command: `python3 validation/run_all.py --spawn target/release/nf-server --quick --port 8853` (the `--port` option and `TMPDIR` only; no harness change). Result file: `validation/results/latest.json`.

**Result: 155 cases: 89 agree, 4 disagree, 62 refused correctly, 0 wrongly accepted, 0 wrongly refused, 0 defect findings. Exit status 1.** The run is not green. Before this pass the same quick run stopped on D4 alone; in this run the D4 case (`N-tangent-d-b90-g2e-06`) is one of the disagreements below, so its `eps_num` check did not run and **D4 is not re-measured.**

Two intermediate builds of this pass gave false refusals of valid joints (a mating-length check comparing two polylines of different density, and a fold-back check with a chord tolerance that did not cover one part sampled more finely than the other). Both are fixed in the final build: the two parts of a joint now share one segment count, and the mating-length check was removed (the two exact point checks remain).

The four disagreements:

1. **`S-SCALE-asm-25.4` and `N-magnitude-ecc-2^8` (scale invariance).** The harness expects the vertex count to be the same for a scaled part. Since review RB-004 the chord-error limit is absolute (0.01 mm), so a part 25.4 times larger needs more segments (the smallest 720·2^k that meets the limit; the chord error of a segment is proportional to size and to 1/n², so n grows with the square root of size). The geometry is not in question: the frame mapping of these parts agrees. This is a change of the product's contract, and the harness's expectation of exact vertex counts under scaling is out of date. **Open:** the harness check needs a derivation and a written decision on whether the density limit should be relative. Not changed in this pass.

2. **`N-tangent-d-b90-g2e-06` and `N-tangent-r-b45-g1.1e-06` (header frame mapping).** The harness reports vertex deviations of 8.46e-2 mm and 4.69e-1 mm from the exact curve. An independent evaluation of the product's opening vertices gives the opposite: the header opening was evaluated in plate coordinates from Spec 02 eq. (4.3) and the header chart, using 2·10⁶ samples of the exact curve and the two neighbouring chords for each vertex. The largest distance is 2.3e-8 mm (5 760 vertices, first case) and 1.2e-7 mm (11 520 vertices, second case), both below the harness tolerance of 1.9e-6 mm. The harness computes each vertex distance with `validation/reference/ruled.py` `dist_pts_to_param_curve`: the nearest of 2 049 coarse samples, then a golden-section search on the neighbouring interval. Near tangency the curve comes close to itself, so that search can settle on a wrong local minimum. The most likely cause of the 0.08 and 0.47 mm figures is therefore the harness. **Open:** the harness distance routine needs a denser search for these cases, derived and recorded here before it is changed. Not changed in this pass.

3. Both rows above are the same two near-tangent joints that the quick run flagged as D4 in the earlier build, so the D4 status in section 0 stands as written there: `eps_num` is an a-priori bound for the branch and the opening, and the validation's float reference is less accurate than the product near tangency. It has not been re-measured here.

4. The four rows are reported as they are. The quick run exits non-zero until items 1 and 2 are settled.

### 9.1 Lead's exact check of the two near-tangent disagreements (engine 0.4.0)

`validation/exact_onsurface_check.py <nf-server binary>` does not use the harness's nearest-point search. It folds every
vertex of the header opening back onto the header cylinder and measures its distance to the branch axis, which must equal
the branch radius. Result on engine 0.4.0, seam angle fixed at exactly −π, same frame for all three cases:

| case | vertices | max abs(distance to branch axis − r), mm |
|---|---|---|
| N-tangent-d-b90-g2e-06 (harness: disagree, 8.5e-2 mm) | 5761 | 7.1e-14 |
| N-tangent-r-b45-g1.1e-06 (harness: disagree, 4.7e-1 mm) | 11521 | 1.4e-13 |
| N-tangent-d-b45-g0.01 (harness: agree; control) | 721 | 8.5e-14 |

So the product's vertices lie on the true intersection curve in the same frame as an accepted case; the deviation the
harness reports for the two near-tangent cases comes from the harness's comparison near tangency. This check shows that
the vertices are on the curve; it does not measure chord error between vertices, and it does not replace the harness.
Status (superseded by section 10): the harness was corrected in the pass described in section 10; the quick run on the same engine now exits 0 with no disagreements. The text above is the record of the state before that correction.

## 10. Harness correction on engine `nf-engine 0.4.0` (this pass)

Engine: `nf-engine 0.4.0+src.e3c264333b691c19`, built in a clean worktree of commit b5ebd82. The validated product is that committed engine; uncommitted edits present in the main tree (other workers' changes under `crates/`, `web/`, `tests/`, `docs/qa/`, `docs/design/`, and two new files in `docs/backend/`) were neither built nor validated here, and this pass did not touch them. This pass changed only `validation/` and this report.

### 10.1 Diagnoses, confirmed

**A. Scale relation.** `check_relation` required equal entity kinds and equal vertex counts and compared points index by index. The polyline density follows the absolute chord-error limit, so the counts change with size: at k = 25.4 the header opening of `S-SCALE-asm-25.4` grows from 721 to 2881 vertices (the saddle too); at k = 256 the reducer of `N-magnitude-ecc-2^8` grows from 361 to 5761.

**B. Near-tangent header opening.** The harness measured each product vertex to the reference curve with a nearest-point search (2049 uniform samples, then golden-section refinement on the neighbouring interval). Measured instead against the reference sampled until its own chord error is at most tol/10 (tol = max(2e-9 S, 1e-12) = 1.885e-6 mm for both cases):

| case | harness search deviation | exact: product vertices to reference polyline | on-branch residual of folded vertices | reference points at tol/10 |
|---|---|---|---|---|
| N-tangent-d-b90-g2e-06 | 8.460e-2 mm | 1.857e-7 mm | 2.3e-13 mm | 58 483 |
| N-tangent-r-b45-g1.1e-06 | 4.687e-1 mm | 1.874e-7 mm | 4.0e-13 mm | 71 173 |

The 1.9e-7 mm that remains is the reference polyline's own chord error (tol/10), so the product vertices are on the curve within the tolerance; the search was the error. These numbers come from a scratch script on the clean worktree build (port 8857, not in the repository). That run overlapped the disk-full window (13:10 to 13:35 UTC); its server log shows no disk error, and the final run below reproduces the same vertex deviations (1.857e-7 and 1.874e-7 mm).

### 10.2 What changed, and why

- `checks/compare.py`
  - `evaluate()`: the two pairs that have an exact test (`branch_opening` of the header, `saddle_cut` of the branch) are no longer searched. Product vertices are measured exactly (vertex to segment) to an adaptively sampled reference polyline, and the folded vertices must lie on the other surface. The frame hypotheses, the anchor test, the station origin, the seam, the marking side and the mirroring are unchanged. Every other pair keeps the search.
  - `cmp_header()`, `cmp_branch()`: the curve-to-polyline coverage is now an issue (threshold 1.10 eps_rep + tol). Before, the same number only fed the EPS_REP_EXCEEDED defect finding, from a uniform sample of 6 001 or 60 001 points.
  - `compare_scaled()` (new); `scale_request()` and `LENGTH_KEYS` moved here from `run_all.py`.
  - `compare_job(..., dxf=True)`: the DXF round trip can be skipped; only the seeded tests skip it, because they plant geometry, not files.
  - `_dist_pts_polyline3d()`: exact candidate search instead of the dense pairwise loop. Same values (checked against the old loop on random data: difference 0), much faster on the near-tangent cases.
- `run_all.py`: the scale branch of `check_relation()` calls `compare_scaled()`; the old identical-count rule is gone.
- `seeded_faults.py`: `near_tangent_and_scale()` (new) with the near-tangent faults, the scale faults and their controls; `_scale_entities()`; the near-tangent check counts an aborted comparison as a detection, as `compare_job()` does.

Rules, old and new:

| check | old rule | new rule | tolerance and derivation |
|---|---|---|---|
| opening / saddle vertices | nearest-point search to the exact curve, <= tol | exact vertex-to-segment distance to the adaptive reference polyline <= tol; folded vertices on the other surface <= 50 tol / max(sin beta, 1e-3) | tol = rel_tol(S) = max(2e-9 S, 1e-12), S = max(part extent, reference scale) (unchanged). The reference polyline's chord error is <= tol/10, so each distance is exact to +-tol/10. The on-surface tolerance is the existing fold-back tolerance, unchanged |
| opening / saddle coverage | uniform reference sample; defect finding only | curve-to-polyline distance <= 1.10 eps_rep + tol; now an issue | eps_rep is the product's stated representation error; the 10 % margin is the one used by the existing EPS_REP_EXCEEDED rule |
| scale: entity kinds | equal | equal, else an issue | none |
| scale: vertex counts | equal | not required | none; the density follows the absolute chord limit |
| scale: polylines | pointwise, relative 1e-11 | two-sided vertex-to-segment distance d <= tau = k eps_rep(base) + eps_rep(scaled) + 1e-11 S2 | the two stated eps_rep values; S2 = extent of the scaled part; the term k eps_rep(base) is the base chord error carried to scale k |
| scale: lines and arcs | pointwise, relative 1e-11 | d <= 1e-11 S2 (arcs: centre, radius and angles) | exact entities, no chord error |
| scale: exact | none | the scaled pattern passes the exact reference comparison (compare_job) | rel_tol(S2), as for every other case |

### 10.3 Seeded faults (final run)

All 23 lines of the final run are `ok`. Existing faults, unchanged: Defect D1, Defect 002, opening mirrored, opening shifted 0.05 mm, opening from the wrong plate end, opening half a turn, saddle scaled by 1.0005, branch seam a quarter turn, and five DXF tamper faults. Each is detected and its control passes.

New, near-tangent case N-tangent-d-b90-g2e-06 (control passes):
- one opening vertex moved off the curve by 10 tol: detected (no frame mapping);
- opening mirrored about the plate centre line: detected (declared side wrong);
- opening shifted along the station by 10 tol: detected (no frame mapping);
- last 5 % of the opening removed: detected (curve-to-polyline 1.46e+1 mm against 1.10 eps_rep + tol);
- correct vertex count, branch radius 1e-4 relative too small: detected (no frame mapping).

New, scaled case (k = 25.4, header_with_branch; control passes):
- the whole pattern multiplied by k (1 + 1e-6): detected by the line check (3.05e-2 mm against an allowed 3.05e-7 mm);
- only the polylines multiplied by k (1 + 1e-6): detected only by the exact reference comparison (no frame mapping). The chord-error test cannot see this fault; the fault shows that step 3 of the relation is needed.

The first version of the radius fault used 1 + 1e-4. That makes the near-tangent branch miss the header on one side (NaN), so the fault now uses 1 - 1e-4, which keeps the curve valid.

### 10.4 Run table: `run_all.py --quick` on engine 0.4.0 (final)

Command: `python3 validation/run_all.py --spawn <clean worktree>/target/release/nf-server --quick` (port 8802). Result file: `validation/results/latest.json`. Exit status 0; verdict PASS; 384 s (291 s before this pass).

| group | run | agree | disagree | refused correctly | wrongly accepted | wrongly refused | error | defect findings |
|---|---|---|---|---|---|---|---|---|
| closed_form | 9 | 9 | 0 | 0 | 0 | 0 | 0 | 0 |
| limit | 4 | 4 | 0 | 0 | 0 | 0 | 0 | 0 |
| assembly | 21 | 21 | 0 | 0 | 0 | 0 | 0 | 0 |
| refusal | 45 | 0 | 0 | 45 | 0 | 0 | 0 | 0 |
| high_precision | 5 | 5 | 0 | 0 | 0 | 0 | 0 | 0 |
| symmetry | 7 | 7 | 0 | 0 | 0 | 0 | 0 | 0 |
| random_sweep | 25 | 13 | 0 | 12 | 0 | 0 | 0 | 0 |
| workload | 24 | 22 | 0 | 2 | 0 | 0 | 0 | 0 |
| stability | 15 | 12 | 0 | 3 | 0 | 0 | 0 | 0 |
| TOTAL | 155 | 93 | 0 | 62 | 0 | 0 | 0 | 0 |

No findings by code. Before this pass: 89 agree, 4 disagree, 62 refused correctly, exit status 1.

The four former disagreements all agree now:
- S-SCALE-asm-25.4: worst entity deviation 0.94 of its tau; exact reference check of the scaled pattern: 0 issues.
- N-magnitude-ecc-2^8: worst entity deviation 0.996 of its tau; exact reference check: 0 issues.
- N-tangent-d-b90-g2e-06: header vertex deviation 1.857e-7 mm; curve-to-polyline 5.071e-3 mm against 1.10 x 5.069e-3 + tol = 5.58e-3 mm.
- N-tangent-r-b45-g1.1e-06: header vertex deviation 1.874e-7 mm; curve-to-polyline 4.025e-3 mm against 1.10 x 4.024e-3 + tol = 4.43e-3 mm.

D4 (eps_num exceeded): not reproduced in this run. There is no EPS_NUM_EXCEEDED finding. The 12 high-precision cases of the quick run have true error / eps_num <= 0.35 (largest: N-tangent-dneg-b90-g0.0001). The non-quick corpus was not run in this pass, so D4's status over the full corpus is not re-measured here; section 5 records D4 as open for the earlier engine.

### 10.5 What is still open

1. The scale relation's exact step uses rel_tol (2e-9 S). The old pointwise rule was 1e-11 S, but it applied only where vertex counts matched, and that condition is gone. The product's on-surface error is about 1e-13 mm absolute (section 10.1), about six decades below rel_tol at this size, so a scale defect below 2e-9 relative would pass. The 1e-6 fault is caught. Tightening this needs a reference chord error of about 1e-12 S (roughly three times more reference points); not done.
2. Step 2 of the scale relation is near its limit for large k (0.94 and 0.996 of tau). It can fail only when the product's true chord error exceeds its stated eps_rep; step 3 carries the detection.
3. The near-tangent reference curves need 58 000 to 71 000 points at tol/10. The quick run takes 384 s instead of 291 s. Caching the reference polylines across the seeded tests would save time; not done.
4. Near tangency the two surfaces meet at a small angle, so the on-surface residual barely changes when a vertex slides along the surface. For near-tangent joints the planar curve test carries the 10 tol fault.
5. The full corpus, including the tangent gaps 1e-6 and 5e-7 and all N-magnitude cases, has not been run in this pass; D4 is therefore not re-measured for the full corpus.
6. `run_all.py` does not stop its spawned server if an exception escapes the seeded-fault stage. This happened once in this pass: a run that was cut short left its server running after its data directory had been removed, and every job then returned HTTP 500. The server was stopped and the run repeated. Not changed.
7. `validation/exact_onsurface_check.py` still uses port 8856, which is outside the ports allowed for this work. It was not run in this pass and was not changed.
8. `validation/README.md` is unchanged: the command-line usage did not change.
