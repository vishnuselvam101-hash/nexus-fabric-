# Computational validation report

## 0. Re-run on engine `nf-engine 0.3.0` (commit 469c6d9), 19 November 2026

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
