# Tolerance policy (as implemented)

19 November 2026 · engine `nf-engine 0.3.0` · source of truth: the code named in each row. This document describes what the software does; it sets no engineering tolerance. None of these values has been reviewed by a fabrication engineer (evidence level EV-1). Fabrication tolerances, weld gaps and kerf are not modelled at all.

Written in answer to validation finding D8 (`docs/backend/COMPUTATIONAL_VALIDATION_REPORT.md`): acceptance depended on the size of the part because every threshold was absolute.

## 1. Kinds of threshold

| Kind | Meaning here |
|---|---|
| Absolute | A fixed length in millimetres, whatever the size of the part |
| Relative | A fraction of a stated reference length of the part |
| Implementation limit | A range outside which the engine refuses to compute. Not a judgement about fabrication |

## 2. Agreement thresholds of the verification checks (gate G10)

Two binary64 evaluations of the same quantity by different formulations are compared. Their difference is rounding error, of the order 1e-16 × size, so the threshold only has to be far above rounding error and far below any dimension of the part.

| Threshold | Value | Kind | Where | Why |
|---|---|---|---|---|
| `AGREE_ABS` | 1e-6 mm | Absolute, for parts whose largest dimension is 1 mm or more | `uem/src/verify.rs`, `agree_tol` | One nanometre: ten orders of magnitude above rounding error at 100 mm, three or more below any fabrication tolerance. Unchanged from earlier builds |
| `agree_tol(size)` below 1 mm | 1e-6 × size | Relative | same | **New (D8).** An absolute 1e-6 mm is as large as a part of 1e-6 mm; below 1 mm the threshold now shrinks with the part so that it stays one millionth of it. `size` is the largest input dimension of the part (for example max(r_dev, length)) |
| Area checks | `agree_tol` × (r + L), or × slant length + eps_num × perimeter (right cone); eps_rep × perimeter + 1e-9 × area (oblique cone) | Relative to a length of the part | same | An area error is a boundary displacement times a length of the boundary (8.4, 11) |
| Oblique cone outline | max(`agree_tol`, eps_num + 10 × verifier quadrature estimate), on the directed (Kabsch) residual | Absolute or estimate-driven | `verify_eccentric` | Both sides are numerical integrals; the threshold may not be below their own estimated error. The comparison is directed since RC-10 (11) |
| Saddle handedness | `agree_tol` + 2 × eps_rep | Absolute plus representation estimate | `verify_branch` | The two heights are read off a polyline |
| Yes/no checks (profile orientation, same marking side, opening inside the shell) | 0.5 on a value that is exactly 0, 1 or -1 | n/a | same | Encoded booleans |

**Reference case, before and after (CYL-02, r 500, L 1000):** thresholds 1e-6, 1e-6, 1e-6 mm and 1.5e-3 mm² before; identical now (asserted by `d8_agreement_tolerance_is_relative_below_one_millimetre_and_unchanged_above`). No check is looser at any size; checks are tighter only for parts smaller than 1 mm. Example of the effect: a shell of r 1e-4 mm developed with a radius 5 % too large passed every check before and fails now.

Not changed: at very large sizes the absolute 1e-6 mm approaches rounding error (1e-16 × 1e8 mm × condition number). The validation report measured false rejections above about 3e9 mm; the engine refuses dimensions above 1e8 mm (section 5), where the stress campaign still verifies every case. A threshold that grows with size would loosen the checks at normal sizes for nothing, so it was not introduced.

## 3. Profile topology (gate G11) and file round trip (gate G15)

| Threshold | Value | Kind | Where | Why |
|---|---|---|---|---|
| Profile closure gap | 1e-9 mm | Absolute | `nf-core/src/job.rs` → `uem::profile::check` | Consecutive curves share end points computed by the same expression; the gap is 0 or rounding error. Absolute: it fails falsely only below about 1e-10 mm part size, which is refused (section 5) |
| Self-intersection, orientation | none | Exact predicates on binary64 (not exact arithmetic) | `uem/src/profile.rs` | Stated in gate G11 |
| DXF re-import: point and length deviation | 1e-8 mm | Absolute | `nf_dxf::reimport_check_annotated` | Numbers are written in shortest round-trip form, so the true deviation is 0; arcs are compared at end points computed from degrees |
| DXF header extents | 1e-8 mm + arc sampling error + 1e-9 × drawing size | Absolute plus relative | same | The check recomputes the extents by dense sampling (4,096 points per arc); its own sampling error is added, so the threshold is no tighter than the method |

## 4. Input consistency and matching

| Threshold | Value | Kind | Where | Why |
|---|---|---|---|---|
| "Same radius" | 1e-12 | Relative | `job.rs` `SAME_RADIUS_REL` | Decides whether a contact radius equals a development radius (BE-001). Covers binary64 noise only; it is not an engineering tolerance |
| Lesson match | 1e-9 | Relative (absolute below 1) | `nf-server` `raise_advisories` | "The same value was corrected before" |

## 5. Implementation limits (refusals, not tolerances)

| Limit | Value | Where | Why |
|---|---|---|---|
| Largest magnitude of any length, position or offset | 1e8 mm | `job.rs` `MAX_MAGNITUDE_MM` | Upper end of what the stress campaign exercises; beyond it results lose digits and, far beyond, become non-finite (validation D2, stress S-9). Checked for every field before anything is computed |
| Smallest positive dimension | 1e-6 mm | `job.rs` `MIN_DIMENSION_MM` | Lower end of what the stress campaign exercises |
| Apex radius of a cone | r1 / sin α > 1e8 mm refused (eccentric: largest apex-to-large-end distance) | `develop.rs` `MAX_DEVELOPED_MM` | Largest size this build is exercised with, equal to `MAX_MAGNITUDE_MM`. Until 0.4.1: sin α < 1e-3 refused, which the arithmetic never required (r1 − r2 is exact; the closed forms are correct to a few ulp for every sin α > 0) (B2-005) |
| Seam gap of a cone | l2·(2π − Φ) ≤ 1e-9 mm refused, right cone and oblique cone alike | `develop.rs` `SEAM_RESOLUTION_MM` | A cone sector never overlaps (right cone Φ = 2π sin α < 2π; oblique cone Ψ < 2π by the angle defect at the apex); the two seam edges cannot be told apart below the 1e-9 mm closure resolution of the profile check (G11). No fabrication seam-clearance rule exists in the specification and none is added. Until 0.4.1: 99.9 % of a turn (B2-005) |
| Branch near tangency | \|δ\| + r ≥ R (1 − 1e-6) refused | `develop.rs` | Tangent and partial intersections not implemented |
| Branch angle and size | β outside (0, 180) deg, or s_max / s_max − s_min > 1e8 mm, refused | `develop.rs` | s(φ) is exact for every 0 < β < 180 (the error bound carries 1/sin β); only the developed size grows. Until 0.4.1: sin β ≤ 0.17 refused, which had no basis (B2-005) |
| Branch angle the verification cannot confirm | refused with `branch_angle_not_verifiable` when the verifier's own rounding bound is not inside its tolerance budget AND its 3D check fails (section 7) | `verify.rs` `branch_angle_refusal`, `job.rs` | Engine 0.4.3, task 1.1: replaces jobs that G10 rejected at shallow angles; derived from the verifier's tolerance, no angle constant |
| Chord-error cap | 0.01 mm with at most 46 080 segments | `develop.rs` `MAX_CHORD_ERROR_MM`, `MAX_SEGMENTS` | Kept (B2-005 point e). Implied bound: the sagitta R(1 − cos(π/n)) ≤ 0.01 mm gives R ≤ 0.02·n²/π² ≈ 4.3e6 mm for a full-turn polyline |
| Seam clearance reported (not a refusal) | every cone and reducer states `seam_gap` (the flat-pattern gap at the small end) and `seam_gap_resolution`; the profile gate G11 is INCONCLUSIVE, never PASS, when the gap is below the resolution | `develop.rs`, `job.rs` | Q2(b), engine 0.4.2. Resolution (derived, not tuned): right cone `max(1e-9 mm, 2·eps_num)` (its arcs are exact entities and its seam edges straight lines, but the written vertices carry the numerical error `eps_num` = l1·U·(16 + 8Φ), which grows with the size of the part, so two seam edges closer than twice that cannot be told from a closed seam nor drawn apart in a DXF; the arc display sagitta plays no part); eccentric reducer `max(1e-9 mm, 2·eps_rep, 2·eps_num)` (its ends are polylines, so a clearance below twice their representation error cannot be told from a closed seam). Below the resolution G11 is INCONCLUSIVE, never PASS, and G15 may also reject the job (the DXF cannot represent the seam; REJECTED, fail-closed). The refusal at 1e-9 mm is kept; for a small cone 2·eps_num is far below 1e-9 mm, so the resolution is 1e-9 mm and the band is empty, for a large cone (apex radius 1e7 mm: 2·eps_num about 1.5e-7 mm) the band 1e-9 mm to 2·eps_num is a result with G11 INCONCLUSIVE. Until the 0.4.2 review fix the cone resolution was the bare 1e-9 mm, which gave G11 PASS on seams the DXF cannot represent (caught by G15 only). Corpus cases `B-cone-seam-vs-eps-num-in-band` / `-above-band`, seeded faults in `validation/seeded_faults.py` |

## 6. Reported errors: which are bounds and which are estimates

| Value | What it is | What it is not |
|---|---|---|
| `eps_num` | An a-priori bound of the rounding error of the pattern coordinates (first-order analysis, checked against 50-digit arithmetic in the earlier validation) for the branch saddle (`saddle_error_bound`), the header opening (`opening_error_bound`) and the right cone. For the eccentric reducer it includes an ESTIMATE of the quadrature error (section 8.2) | Not interval-certified. For the eccentric reducer it is not a bound, because its quadrature term is an estimate. Absolute sizes: below 2e-7 mm. (The earlier form of this section called it an estimate with measured ratios of 3.4 to 6.1 times; those ratios describe the earlier estimate, not the a-priori bounds of 0.4.4.) |
| `eps_rep` | **A certified upper bound** of the distance from the exact curve to the drawn polyline (section 8.1) for the branch saddle and the header opening: `eps_rep_certified` is true. For the eccentric reducer the chord part is certified but the vertex term is an estimate, so the value is NOT certified (`eps_rep_certified` false). Where no bound can be proved the value is the measured estimate (15 interior points per segment), also NOT certified | Before 0.4.4 it was a measured estimate: the true maximum can lie between samples (0.0095979 mm against a true 0.0096085 mm, task 0.3). The certified value of the near-tangent case is 0.0096380 mm. Checks that would pass with an uncertified value are INCONCLUSIVE (section 8.1) |
| `points_max_deviation` of an arc in the JSON result | The exact sagitta of the 96 display chords | The JSON points of an arc are for display; the DXF carries the exact arc (D7) |

The result, gate G08 and the interface state, for each value, which of the three it is: a certified bound, an a-priori
bound that is not interval-certified, or an estimate that is not a bound. The G08 and assumption wording of the job
follows this table (work plan task 1.2 review).

## 7. Shallow and near-180° branch angles: what the verification can confirm (engine 0.4.3, work plan task 1.1)

**Rule.** A branch angle `beta_deg` is refused with `branch_angle_not_verifiable` when, on the final patterns and before
the job is accepted, (a) the a-priori bound F on the verifier's own rounding in at least one 3D-intersection check is
not below the part of that check's tolerance left after the engine's own error, F ≥ agree + 3·bound, and (b) at least
one check of gate G10 fails, and EVERY failing check is one of the four 3D-intersection point checks that carry an F
(`verify::CONDITIONED_CHECKS`: `saddle_matches_3d_intersection`, `opening_matches_3d_intersection` and the two checks of
`verify_joint`), has its OWN F at or above its own budget, and differs by no more than a correct pattern can, difference
≤ F + bound. (b) is the verifier's criterion (the same checks, on the same patterns, that G10 reads); (a) and the last
two conditions restrict the refusal to failures the verifier's own conditioning can explain. Any other failure (another
check, a check whose own F is inside its budget, or a difference beyond F + bound) is a wrong pattern and the job is
REJECTED by G10 as before; a pattern the verifier confirms is accepted. Faults planted inside the regime (stub 0.003°,
header with branch 179.99°) are rejected, not refused (test `task_1_1_a_wrong_pattern_inside_the_regime_is_rejected_not_refused`)
and caught by the validator (`validation/seeded_faults.py` `regime_faults`). No angle is hard-coded: the regime follows from the inputs. Code: `uem::verify` (`*_rounding`,
`*_conditioning`, `branch_angle_refusal`), applied in `nf-core/src/job.rs` for the branch stub and the header with
branch. The validation oracle (`validation/checks/oracle.py` `branch_conditioning`) MIRRORS this tolerance definition,
the engine's bounds and the constants, as its other limits mirror the engine's (review C, RC-28): it is not independent
evidence; the geometry of every accepted case is still compared with the independent reference.

**Why the verifier is conditioned.** The independent formulation (`independent::Joint3`) builds the joint as a solid in a
tilted frame and finds each saddle point as the root of g(s) = |b + dp·s| − R along a branch ruling, where dp is the branch
direction with its component along the header axis removed. The slope at the root is g′ = sin β · cos ω ≥ sin β · c,
c = sqrt(R² − (|δ| + r)²)/R. The direction dp is formed as d − (d·ex)·ex from vectors of length 1, so its part across the
header axis carries an absolute error of a few u (u = 2⁻⁵³), while its length is sin β: relative error u/sin β. Times
|s| ≤ S = (R + r|cos β|)/sin β and divided by the slope, the root errs by about u·S/(sin β · c) ∝ u(R + r)/sin²β, at
both ends of the angle range. The tolerance of the check is agree_tol(size) + 4·bound, `bound` being the engine's own
a-priori bound of the compared vertex (section 6 and `develop::saddle_error_bound`, `opening_error_bound`), which near
0° grows only as 1/sin β.

**The bound F (first-order rounding analysis; terms in `uem::verify`).** With S as above, D = r(1 + (|δ|+r)/root)/sin β
(≥ |ds/dφ|), root = sqrt(R² − (|δ|+r)²), frame origin |o| ≤ 0.87·scale, station t, frame defect ν = u (exact rational
evaluation of the stored frame constants: |ex·ex − 1| = 1.0 u, others ≤ 0.47 u, asserted by a test):

| Quantity | Bound |
|---|---|
| root of g (`intersection_root_bound`) | F_s = [3u·S + u(2·0.87·scale + 6|t| + 6|δ| + 15r) + u(5R + |δ| + r)] / (sin β · root/R) + 8u·max(S, R) (direction across the axis; the base point after removing its axial part; evaluation of g at the root; stopping rule 4u·max(S, R) and the relative frame defect) |
| a point of `branch_point` | P = u(3·0.87·scale + 5|t| + 5|δ| + 13r + 4S) |
| `saddle_matches_3d_intersection` | F_B = F_s(scale R, t = 0) + 2u·2π·D (the fold of the vertex parameter φ = x/r_dev) |
| `opening_matches_3d_intersection` | F_H = δx + R_dev·(√2·δt/R + 2u·2π + 3u·2π + 1.3e-16) + 3u·2π·(D + r + R_dev·r/root), with e = F_s(R, t) + P, q = |t| + S + |δ| + r, δx = e + 4u·q, δt = e + (4u + ν)·q (header chart: x from a dot product, ω by atan2 of two components at radius R, v = R_dev(ω + π), and the rounding of the vertex parameter) |
| the two checks of `verify_joint` | F_J = e_p + F_s(R, t) + P + (r + D)·δφ, δφ = hypot(δa, δb)/r + 2u·2π, δa = e_p + e_o + (S + r)(7u + 2ν), δb = e_p + e_o + (S + r)(4u + 1.5ν), e_p = max(u(0.87R + 3(|t| + S + r) + 5R) + R(3πu + 4u), P + 2u·2π·r), e_o = u(0.87R + 3|t| + 3|δ|) (the vertex placed on its surface, its branch azimuth recovered by atan2 from a vector of length up to S + r, moved along the curve by |C′| ≤ r + D, and the computed curve point) |

Budgets: saddle agree_tol(max(R, s_end)) + 3·saddle bound; opening agree_tol(max(R, L)) + 3·opening bound; joint
agree_tol(max(R, s_end)) + 3·(opening bound + saddle bound). A correct pattern is guaranteed to pass a check when
engine error + verifier error ≤ agree + 4·bound, i.e. (engine error ≤ bound) when F ≤ agree + 3·bound.

**Measured tightness.** Largest measured check difference / F over a sweep of R ∈ {1e-3, 150, 1e6} mm, r/R ∈ {1e-3, 0.3,
0.667, 0.9}, δ ∈ {0, 0.3(R − r), ±(R − r − 2e-6 R)} and 28 angles at both ends: 0.19 (saddle), 0.08 (opening), 0.15
(joint); no point check failed with F inside its budget (unit test `task_1_1_verifier_rounding_bound_covers_the_measured_differences`,
0.18 on its own smaller sweep). F is a worst-case sum of roughly twenty roundings, so it is 5 to 30 times the error the
verifier actually commits. **That is why F is the gate of the regime and not the refusal itself:** as the refusal it would
turn away angles the verifier confirms by a factor of 20 (for R 150, r 100: every header with branch from about 179.97°
and between about 0.02° and 0.06°). Inside the regime the verifier's own verdict decides, so no case it confirms is
refused and none it cannot confirm is accepted.

**Same logic at both ends, different outcome.** The engine's bound carries the rounding of the angle itself,
2u·β·|ds/dβ| ≤ 2u·β·(r + (R + r)|cos β|/sin²β) (section 6). Near 0° (β ≈ sin β) that term grows as 1/sin β; near 180°
(β ≈ π) it grows as 1/sin²β, like F. So at the obtuse end the budget keeps pace with F:

| R = 150, r = 100, δ = 0 | Acute end | Obtuse end |
|---|---|---|
| Branch stub: ratio F/budget = 0.9 (just inside) | β = 0.02016° (accepted; vertex error 3.1e-10 mm vs eps_num 1.8e-9 mm against the 31-digit reference) | never reached: the ratio stays ≤ 0.21 up to the 1e8 mm size limit (179.99986°); inside boundary = the size limit |
| Branch stub, in the regime | 0.003° (ratio 39): the verifier confirms, accepted; 0.002° (ratio 87) and 0.001°: the 3D check fails, refused | — |
| Header with branch: ratio 0.9 (just inside; the joint checks govern) | β = 0.06385° (accepted, agrees with the reference) | β = 179.96378° (accepted, agrees with the reference) |
| Header with branch, in the regime | 0.009° (ratio 44): the joint checks fail (about 2× their tolerance), refused | 179.99° (ratio 1.11): the verifier confirms (differences about 1/20 of the tolerances), accepted |

In the sweep the regime was entered at the obtuse end only for headers with branch (ratio up to 2.1 for r/R 0.667–0.9,
up to 178 for r/R = 1e-3 next to tangency), and no 3D-intersection check failed there before the size limit: at the
obtuse end no request is refused by this rule. Measured on 0.4.2, the 3D checks of a stub begin to fail below about
0.0023° (not every angle: 0.00116° passed), those of a header with branch below about 0.0105°.

**eps_num shown prominently.** Near 180° eps_num itself becomes large (R 150, r 100: 6.4e-3 mm at 179.9997°, 2.7e-2 mm at
the size limit, where the true vertex error against the exact angle in degrees is 1.05e-2 mm), because β = fl(beta_deg·π/180)
is rounded at the size of π. A part is flagged `eps_num_governs` (result JSON) and the interface shows eps_num first, in
full, with the words "estimate, not a bound" (English and Japanese), when 8·eps_num ≥ the tolerance of its 3D point check
(`saddle_matches_3d_intersection` for the branch, `opening_matches_3d_intersection` for the header). For the branch,
eps_num IS the bound in that tolerance (agree + 4·bound), so the condition is exactly 4·eps_num ≥ agree_tol(size). For the
header, eps_num is the larger of the plate's own estimate 8u·max(2πR_dev, L) and the opening bound, while the tolerance
holds the opening bound only; so for a long plate the flag can fire earlier than the opening bound alone would make it.
For R 150, r 100 it fires near 180° only (from about 179.95°), never at the acute end (eps_num stays small there) nor at
ordinary angles.

**Not covered (open).** The developed-length checks (`*_developed_length_equals_3d_arc_length`, `cmp_resolved`) can fail
outside this regime for reasons that are not the 1/sin²β rounding: a header with a very small branch (r/R = 1e-3) at
179.996° and 179.999°, and near-tangent parts of 1e-3 mm at every angle (also at 90°). Those jobs stay REJECTED by G10
(fail-closed, no file); they belong to tasks 1.2 and 1.6 (bounds and sampling), not to this rule.

## Changed validation rules (engine 0.4.2 review; old -> new -> reason)

| Rule | Old | New | Reason |
|---|---|---|---|
| `validation/run_all.py` `check_relation` `stub_equals_branch` | the whole branch DXF text of the stub equalled the whole branch DXF text of the assembly part | only the cut geometry is compared: the ENTITIES section up to the first TEXT | since engine 0.4.1 the annotation of an assembly part also states the joint, so the TEXT notes legitimately differ. The slice is complete: `nf_dxf::write_stated` writes every geometry entity first and every annotation after (test `cut geometry must be written unchanged and before any annotation` in `nf-dxf`), so no cut entity can follow the first TEXT. The cut geometry itself is compared exactly (byte-identical), nothing was loosened |
| `validation/stability.py` `N-magnitude-*` `inside` bound | -26 <= n <= 16 for every family | -26 <= n <= 16 for cone/cylinder/branch, -26 <= n <= 14 for the eccentric reducer and the assembly | polyline families are limited by the chord-error cap (R(1 - cos(pi/46080)) <= 0.01 mm gives a developed radius of at most about 4.3e6 mm, kept in 0.4.2); the base parts (200 mm) exceed it from a scale of 2^15 (6.6e6 mm), so those scales are correctly refused, not a defect. Outside-range cases still must be refused or correct |
| `validation/checks/compare.py` `seam_resolution_check`, cone floor | constant 1e-9 mm | `max(1e-9, 2·eps_num)` from the product's reported `eps_num`, with `eps_num` required to be at least extent·2^-53 (computed from the DXF) | the constant mirrored the engine and was too small for large cones (see the seam row above) |

## Changed validation rules (engine 0.4.3, work plan task 1.1; old -> new -> reason)

| Rule | Old | New | Reason |
|---|---|---|---|
| `validation/checks/oracle.py` `classify`, branch stub and header with branch | `valid` (must be accepted) for every angle inside (0, 180) deg within the size limit | `edge` where the ratio F / (agree + 3·bound) of section 7 is at least 1 − 1e-12; `valid` below | the product now refuses an angle its verification cannot confirm, which can only happen in that regime. The ratio MIRRORS the engine's tolerance definition (RC-28 precedent): it is not independent evidence. The band 1e-12 covers the different evaluation order of the same closed form in Rust and Python (about 60 operations of relative error ≤ u and sin/cos/radians within 2 ulp: at most about 100u = 1.1e-14 relative, first order) |
| `validation/run_all.py`, a refused case with expectation `either` | any refusal, including a pattern REJECTED by the product's own gates, counted as refused correctly | in the task 1.1 regime (oracle reason) only a refusal with code `branch_angle_not_verifiable` counts; a gate REJECT (finding REFUSED_ONLY_BY_GATE) or another code is a wrong refusal | without this, the `edge` class would have hidden a G10 REJECT inside the regime: a looser rule than before |
| corpus `B-br-verified-floor-beta-0.003` / `-180-0.003` | expect accept | unchanged (expect accept) | 0.003 deg is inside the regime (ratio 39) and the verification confirms it, so the product must still accept it; 179.997 deg is outside the regime (ratio 0.21). Only the comment in `build_corpus.py` changed (the range below 0.003 deg is now refused, not rejected) |

## 8. Certified error bounds (engine 0.4.4, work plan task 1.2)

**Scope.** `eps_rep` (the chord error of every polyline) is a certified upper bound (8.1) for the branch saddle and the
header opening. For the eccentric reducer it is NOT certified, because its vertex term contains an estimate (8.2). `eps_num`
(the displacement of the vertices) keeps its a-priori bounds for the branch saddle, the header opening and the right cone
(8.2); for the reducer it is partly an estimate. The developed-length checks of a branch saddle and of a header opening compare a certified
enclosure of the exact length (8.3). The area check of a header opening is computed relative to the opening and scaled
to its perimeter (8.4). Arithmetic: `crates/uem/src/interval.rs`, outward rounding by `next_up`/`next_down` on every
operation; sin, cos and asin come from the platform library, which the engine ASSUMES accurate to 2 ulp (as `develop::TRIG`),
widened here by 4 ulp of relative rounding. Square roots accept a lower bound that is negative only by rounding (a sum of
squares). No external crate.

### 8.1 Chord error of a polyline: `eps_rep` (all polyline families)

Let c(t) be the exact curve, t the uniform parameter (phi for the saddle and the opening, theta for the reducer). The
drawn polyline has n segments with vertices c(t_{i·16}); each segment is split into 16 sub-intervals [t_k, t_{k+1}] with
width h_k, and M_k ≥ sup |c''| over each sub-interval.

*Claim.* For t in [t_k, t_{k+1}] and the chord segment S of its segment,

    dist(c(t), S) ≤ max(D_k, D_{k+1}) + M_k·h_k²/8·(1 + 8u),   D_k = dist(c(t_k), S),

*Proof.* (1) Let L(t) be the point of the sub-chord c(t_k)c(t_{k+1}) at the same fraction of the interval. The error of
linear interpolation of a C² vector function is e(t) = c(t) − L(t), with e = 0 at both ends and e'' = c'': by the Green's
function of the interval, e(t) = ∫ G(t,s)c''(s)ds with G ≥ 0 and ∫G ds = (t − a)(b − t)/2, so |e(t)| ≤ M_k·h_k²/8.
(2) dist(·, S) is a convex function (distance to a convex set), so dist(L(t), S) ≤ max(D_k, D_{k+1}).
(3) Triangle inequality: dist(c(t), S) ≤ |e(t)| + dist(L(t), S). No condition on the speed or the angle of the curve is
needed (an earlier argument needed M·H < speed and was dropped: it failed for the shallow saddles, 8.7).

*Why it is an upper bound of the drawn polyline.* The brackets are measured on the RE-SAMPLED vertices `sample(n, 16)`,
i.e. the exact curve at the vertex parameters of the verification grid, and NOT on the drawn vertices. The drawn vertex of
each point lies within the vertex displacement bound `eps_num` of the exact curve (8.2). Two chords whose endpoints each
move by at most e lie within Hausdorff distance e of each other, so every point of the drawn chord is within e of the
re-sampled chord. That gap is covered by ADDING `eps_num` to the bound (`develop::certified_rep`, argument `vertex_eps`).
The 16 units of roundoff at the largest coordinate (`chord_error_bound`, `margin`) cover only the binary64 evaluation of
the sampled distances D_k; they do not cover the Hausdorff gap. One point is NOT covered by this argument and is stated
as open: the drawn and the re-sampled vertices use parameter grids that differ by rounding (the drawn vertex i is at
phi = i·(2π/n), the re-sampled one at 16i·(2π/(16n))), so they are not bit-identical, and this rounding is not separately
bounded. The reducer's vertex term is an estimate (8.2), so its bound is not certified (below).

*M_k by interval arithmetic on the closed forms* (`curve_bound.rs`):
- Branch saddle, c = (r_dev·phi, s(phi)): |c''| = |s''|, with y = delta + r cos phi, Q = sqrt(R² − y²),
  s = (Q − r sin phi cos beta)/sin beta, s'' = (Q'' + r sin phi cos beta)/sin beta,
  Q' = −y y'/Q, Q'' = −(y'² + y y'' + Q'²)/Q, y' = −r sin phi, y'' = −r cos phi. Code `saddle_derivatives`, `SaddleCurve`.
- Header opening, c = (x, y): x = station + s cos beta − r sin phi sin beta, y = R_dev (omega + pi), omega = asin(y_b/R).
  x'' = s'' cos beta + r sin phi sin beta; omega' = −r sin phi / Q; omega'' = −r cos phi / Q + r² y_b sin² phi / Q³;
  y'' = R_dev omega''. |c''| = sqrt(x''² + y''²). Code `OpeningCurve`.
- Eccentric reducer, polar about the apex (a, h') of the binary64 apex of the code: rho = sqrt(G),
  G = (r1 − a)² + h'² + 4 a r1 sin²(theta/2), rho' = G'/(2 rho), rho'' = G''/(2 rho) − G'²/(4 rho³),
  G' = 2 a r1 sin theta, G'' = 2 a r1 cos theta; psi' = D = r1 H/G, H = sqrt(h'² + X²), X = r1 − a cos theta,
  D' = r1 (H' G − H G')/G², H' = X a sin theta / H. The components of c'' are E_rho = rho'' − rho D² and
  E_psi = 2 rho' D + rho D', and |c''| = sqrt(E_rho² + E_psi²). Code `ConeCurve`. Only the rotation of the frame
  depends on psi, and it drops out of |c''|.
- Where a quotient or square root cannot be certified (a denominator interval containing zero), `chord_error_bound`
  returns no bound: `eps_rep_certified` is false, `eps_rep` is the measured estimate, and the checks that use `eps_rep`
  return INCONCLUSIVE (`verify::rep_check`). No case of the accepted corpus or of the property tests of 8.6 needed this.

*Certification.* `eps_rep_certified` is true only where the vertex term is a proved bound (branch saddle and header
opening). For the eccentric reducer the value is still reported as the chord bound plus `eps_num` (the best available
value, never below the measured one), with `eps_rep_certified` false and basis `EPS_REP_VERTEX_ESTIMATE`. Where no bound
can be proved, the value is the measured estimate with `eps_rep_certified` false (`EPS_REP_UNCERTIFIED`).
A check whose tolerance uses an uncertified `eps_rep` (`verify::rep_check`): a would-be PASS becomes INCONCLUSIVE; a
measured difference beyond the tolerance stays FAIL, because an uncertified tolerance can only make a failure look more
doubtful than it is, and the conservative direction is kept (work plan task 1.2 review; before it, both became
INCONCLUSIVE).

**Precision rule (review after task 1.2; "certified bound > required precision => INCONCLUSIVE").** The tolerance of
every check that uses `eps_rep` contains 1 or 2 times that value. A certified bound coarser than the precision the product
requires, `MAX_CHORD_ERROR_MM` = 0.01 mm (`develop.rs:55`, the chord limit of section 5), would let almost any pattern
pass. Such a check is therefore INCONCLUSIVE, never PASS, whenever `eps_rep > MAX_CHORD_ERROR_MM` (`verify::rep_check`,
`verify.rs:348`; NaN is treated as coarse). A measured difference beyond the tolerance stays FAIL. Applies to the
circumference checks and the drawn-area check of the reducer (the area check since RC-10, section 11), the 3D checks of
the branch saddle and the header opening, and the interface checks of the header with a branch. Cone and cylinder have
`eps_rep = 0` (exact arcs) and no check that uses it. The check verdicts change, not the state, the outcome or the cut
geometry. Counts: section 8.8.

*Density unchanged.* `choose_density` still decides the number of segments from the MEASURED value (as before), so the
cut geometry is byte-identical. The reported value is the certified bound (about 1 % above the measured one at the
near-tangent case: 0.0096380 mm against a measured 0.0095979 mm; true 0.0096085 mm). It stays below the 0.01 mm limit.

### 8.2 Vertex displacement: `eps_num`

- Branch saddle: `develop::saddle_error_bound`, unchanged a-priori bound (section 6 and 7): covers s, the rounding of
  r_dev·phi and of the closing vertex.
- Header opening: `develop::opening_error_bound` and the plate's own 8u·max(2π R_dev, L), the larger of the two is
  reported. Unchanged, a-priori.
- Eccentric reducer: NOT a bound. Its quadrature term (`qerr`, the difference of the last two refinement levels of the
  adaptive rule) is an estimate (`EPS_NUM_BASIS_CONE`). The reducer's `eps_rep` adds this estimate, so it is NOT certified
  (`eps_rep_certified` false, 8.1). Open item for a later task: proving the quadrature error.
- Right cone and cylinder shell: exact arcs and straight edges, eps_rep = 0. The cone's eps_num (a-priori bound,
  `l1·U·(16 + 8Φ)`) feeds its drawn-area tolerance since RC-10: eps_num × perimeter (section 11).

### 8.3 Developed length of a branch saddle and of a header opening: `length_enclosure`

The exact length is the sum of the arc lengths of the curve. For one segment with drawn chord direction u (any unit u),

    arc − |chord| ≤ ∫ (|c'| − c'·u) dt = ∫ |c'_⊥|² / (|c'| + c'·u) dt,

because |chord| ≥ chord·u = ∫ c'·u dt, and c'_⊥ is the component of c' across u. The segment is split into 64
sub-intervals of width h. On each, with the midpoint m, M ≥ sup|c''| (from `sup_c2`) and c'(m) enclosed by interval
arithmetic (`curve_bound::length_excess_bound`):

- Taylor about m: |c'(t) − c'(m)| ≤ M·h/2, so sup|c'_⊥| ≤ |c'(m)_⊥| + M·h/2 =: P;
- each of |c'| and c'·u moves by at most M·h/2, so inf(|c'| + c'·u) ≥ (|c'(m)| + c'(m)·u) − M·h;
- the integrand is at most P² / inf(|c'| + c'·u) where that denominator is positive, and at most the crude bound
  sup|c'| − inf(c'·u) ≤ |c'(m)| − c'(m)·u + M·h everywhere; the smaller of the two is used. The crude bound is needed
  where the curve turns against the chord inside one sub-interval (the tip of a saddle near 180 degrees).

The sum over the sub-intervals bounds the excess over the drawn chords, which carry the displacement of 8.2. Hence the exact length lies in
[L − 2n·eps_num, L + E + 2n·eps_num], L the drawn length; the check uses centre L + E/2 and half-width E/2 + 2n·eps_num
(`verify::length_enclosure`). This replaces the Richardson limit of the stride lengths, which had no bound (task 0.3 measured
its resolution at 1.29e-2 mm against a need of 1.15e-2 mm). For the near-tangent case the resolution is now 3.8e-3 mm
(measured, test `near_tangent_developed_length_checks_resolve_and_pass`), below the need of 1.15e-2 mm, and the 3D
reference lies inside the enclosure. A first form with one interval per 16th of a segment gave 4.8e-3 mm; a box
enclosure of the derivative (instead of the midpoint) gave INCONCLUSIVE for a small obtuse stub (regression test
`small_obtuse_stub_developed_length_is_pass`) and was replaced.

### 8.4 Area of a header opening: `opening_area_equals_surface_area`

- The shoelace of a closed loop is unchanged by translation. The area is computed on coordinates relative to the first
  vertex (`develop` `area2`, `verify::verify_header_opening` `shoelace`), and the 3D area integral on ∮(x − x0) dω, which
  equals ∮ x dω because ∮ dω = 0. The absolute coordinates (x ~ station, y ~ circumference) had cancelled to a fraction of
  the opening's area (task 0.3: rounding steps 1.16e-9 mm² against an opening of 3.6e-6 mm²).
- Tolerance: `agree_tol(max(R, L)) × (extent of the opening in x + extent in y)`. The area error of a polygon is a
  boundary position error times a length of the boundary, and the length of a convex loop is at most twice the sum of its
  two extents; the scale is therefore that of the opening. The opening lies inside the plate, so the extents are at most L
  and 2π R_dev: the new tolerance is never larger than the 0.4.3 tolerance agree × (2π R_dev + L). (The first version used
  the perimeter, which for the near-tangent header (about 4 700 mm) exceeded the plate scale and would have loosened the
  check three-fold; it was replaced.) For the tiny case the plate tolerance of 0.4.3 (8.3e-3 mm²) is replaced by 4.3e-9 mm² (measured).

### 8.5 Right cone and cylinder shell

Exact entities. eps_rep = 0. eps_num is the a-priori first-order bound of 0.4.3 (unchanged).

### 8.6 Property tests (`crates/uem/tests/property_bounds.rs`) and the 40-digit tier

Seeded (xorshift), per family: radius 1 to 1 000 mm, near-tangent (r/R = 1 − 10^−k, k in 2 to 5.5), generic r/R, angles
beta from 10^−2.5 to 10^0 deg, from 180 minus that, and uniform 5 to 175 deg, offsets up to 0.999 of the admissible room.
Two tiers (work plan task 1.2 review, item 3):

- **Fast tier** (`*_fast`, not ignored): 40 parameter sets per family. It runs in every `cargo test -p uem`. Measured in
  debug on this host (2026-11-22, the full `cargo test -p uem` run): 23.8 s for the 3 fast tests, 3 passed, 3 ignored.
- **Full tier** (2 000 sets per family, `#[ignore]`): run in release, as
  `nf-sandbox cargo test --release -p uem --test property_bounds -- --include-ignored --nocapture`.

The drawn polyline is compared with the exact curve evaluated by independently written closed forms (the reducer's psi by
a 4-point Gauss rule), at 31 samples in each of about 150 chosen segments (both ends, the knees at phi = 0 and pi, and
random segments). **Samples are off the bracket grid**: 15 irrational fractions frac(1/2 + j·(√5 − 1)/2) and the 16
midpoints (k + 1/2)/16 of the sixteenths, which is where the M·h²/8 term of 8.1 is attained (the earlier version sampled
the bracket grid itself, where that term is zero). The reducer's psi is evaluated at the off-grid parameters by a partial
Gauss panel from the nearest integration node. Assertion: measured ≤ the reported `eps_rep` for every sample of every
case that has a bound. The reducer's reported value is not certified (8.2): its test checks that value empirically; only a
value with no chord bound at all (`EPS_REP_UNCERTIFIED`) is skipped.

Results of the full tier on 2026-11-22 (release, `--include-ignored`, 6 passed, 0 failed):

| Family | Sets | Accepted (refused) | Largest measured / bound |
|---|---|---|---|
| branch stub | 2 000 | 1 976 (24) | 0.9996 |
| header opening | 2 000 | 1 964 (36) | 0.9993 |
| eccentric reducer | 2 000 | 2 000 (0) | 0.9922 |

The fast tier gives 0.9938 (stub, 39 accepted), 0.9967 (opening, 38 accepted), 0.9922 (reducer, 40 accepted).

**Open finding (the precision rule above now makes such checks INCONCLUSIVE; the bound itself is not tightened).** The same full-tier run prints the reported value of every polyline
it engineers (5 781 evaluations, fast and full tiers). The certified value is far above the measured one in some random
sets: the largest is 0.5641 mm against a measured 0.0096 mm (a factor of 59), the median factor is 1.004, and 212 of the
5 781 values exceed the chord limit of 0.01 mm. Such jobs are accepted and nothing flags them: the density is chosen from
the MEASURED value (8.1), and no rule compares the certified value with the limit. The bound is valid (no sample exceeds
it) but loose in those sets, and a loose `eps_rep` loosens every check that uses it (2·eps_rep). Candidate causes, not
yet tested: the interval enclosure of M near the saddle's sharpest turn, and the crude branch of 8.1 where the curve turns
against the chord. Until this is decided, a result with `eps_rep` above 0.01 mm must not be read as meeting the limit.

**40-digit tier: NOT RUN on this host.** `tests/bound_hp_check.py` re-evaluates the dumped vertices and samples with mpmath
at 40 digits (`NF_BOUND_DUMP=<file>` writes them). mpmath is not installed on this host and was not installed (no system
packages were added for this task). The tier has therefore NOT been run here, and no result of it is claimed. The
50-digit and 40-digit checks cited in the `eps_num` basis texts of `develop.rs` are earlier validation work and were not
repeated by this task.

### 8.7 Shallow angles: what changed

The first version of 8.1 used the line distance and needed M·H < inf|c'| on each segment (a projection argument). At 0.003°
that condition fails where the saddle bends sharply and slowly; the bound was then not certified and the two
3D-consistency checks of the stub became INCONCLUSIVE. The segment-distance form above has no such condition and certifies
these cases (test `task_1_1_a_wrong_pattern_inside_the_regime_is_rejected_not_refused`).

### 8.8 Changed expectations (engine 0.4.3 -> 0.4.4, work plan task 1.2)

| Expectation | Old (0.4.3) | New (0.4.4) | Reason |
|---|---|---|---|
| `eps_rep` of a polyline | measured estimate at 15 points | certified bound (8.1) + vertex displacement (8.2) | items A and D |
| `eps_rep` near tangency, stub and opening (R 150, r R(1−2e-6), beta 90) | 0.0095979 mm | 0.0096380 mm (expected 0.0096401: the interval bound is 2e-6 tighter) | item A; covers the true 0.0096085 mm |
| developed-length checks of a stub and of an opening, near tangency | INCONCLUSIVE (resolution 1.29e-2 mm, need 1.15e-2 mm) | PASS, resolution 3.8e-3 mm (need 1.15e-2 mm) | item C |
| developed-length check of a small obtuse stub (R 5.29, r 5.28, beta 140.6 deg; stress_branch) | PASS (Richardson resolution) | PASS, certified resolution below the need (regression test `small_obtuse_stub_developed_length_is_pass`) | item C; a box enclosure had made it INCONCLUSIVE during the work (fixed, not a verdict change at the end) |
| opening-area tolerance, N-aspect-branch-tiny | agree × (2π R_dev + L) = 8.3e-3 mm² | agree × (extent x + extent y) = 4.3e-9 mm² | item B |
| opening-area tolerance, near-tangent header (N-tangent-r-b90-g2e-06) | agree × (2π R_dev + L) = 1.5e-3 mm² | agree × (extent x + extent y) ≈ 7.7e-4 mm² (never larger than before, by construction) | item B |
| opening-area check, N-aspect-branch-tiny | INCONCLUSIVE (rounding 1.16e-9 mm² against 3.6e-11 mm²) | PASS (difference 2e-17 mm²) | item B |
| `opening_area` quantity, N-aspect-branch-tiny | 3.6275e-6 mm² (0.053 % low, cancellation) | 3.6275527e-6 mm² (relative shoelace; the 720-gon is 1.3e-5 below the limit area) | item B |
| `eps_rep`-dependent tolerances (handedness, delta side, interface, circumferences of a reducer) | tolerance agree + 2·measured | agree + 2·certified (larger by about 4e-5 mm at the near-tangent case) | items A/D: tolerance follows the bound; a loosening of 8e-5 mm, reported |
| verdicts of checks using `eps_rep` when no bound is proved | PASS possible | INCONCLUSIVE | rule 1 (no case of the corpus needed it) |
| engine version | nf-engine 0.4.3 | nf-engine 0.4.4 | item F; results of 0.4.3 are REJECTED with G21 |
| cut geometry | — | byte-identical (density decided by the measured value, unchanged) | rule 8 |
| `assumptions` and G08 wording for eps_rep | "estimates, not bounds" | eps_num an estimate (reducer: its quadrature term); eps_rep a certified bound where stated | item D |
| `eps_rep_certified` of the eccentric reducer | true (first form of task 1.2) | false: its vertex term carries the quadrature estimate (8.2); the value is unchanged | task 1.2 review, item 1 |
| circumference checks of the eccentric reducer (`large_edge_equals_circumference`, `small_edge_equals_circumference`) | PASS | INCONCLUSIVE in every accepted reducer (each is uncertified; the stress judge, which accepts only these two as non-PASS, found no other one among 140 verified reducers). G10 INCONCLUSIVE; state REVIEW_REQUIRED unchanged | task 1.2 review, item 1 (rule 1: fail closed) |
| `verify::rep_check` for an uncertified `eps_rep` | every such check INCONCLUSIVE, a FAIL included | a would-be PASS becomes INCONCLUSIVE; a measured difference beyond the tolerance stays FAIL | task 1.2 review, item 4 (conservative direction) |
| `interval::library` (sin, cos, asin enclosure) | four units of relative rounding plus the smallest normal, on each side | one further unit outward (`next_down`/`next_up`) | task 1.2 review, item 6; no pattern of the compat corpus changed |
| G08 and assumption text on `eps_num` and `eps_rep` | "estimate, not a bound" for both | the status of each value (section 6): certified bound, a-priori bound, or estimate | task 1.2 review, item 1; compat `patterns_sha256` changed for 148 accepted cases (text only, section 8.8 note below) |
| growth of `eps_rep` over the measured value (tolerances use it) | the measured value (0.4.3) | certified bound plus `eps_num` | corpus-wide maximum, compat corpus (335 cases, 150 polyline evaluations): `eps_rep` grows by at most 3.8e-5 mm, so the tolerances that use 2·eps_rep by at most 7.5e-5 mm. The full property tier (random sets) reaches far more: see the open finding in 8.6 |

| precision rule: a check using `eps_rep` with `eps_rep > MAX_CHORD_ERROR_MM` (certified) | PASS | INCONCLUSIVE (8.1, `verify::rep_check`) | review after task 1.2. Counts over the 1 733 requests of the compat fixture (335) and of `validation/results/latest.json` (1 398): 0 checks changed PASS to INCONCLUSIVE, no state, eps_rep or outcome changed. The only corpus cases with `eps_rep` above 0.01 mm (D-ASM-106 and N-tangent-r-b90-g0.01, header 0.010039 mm) do not feed it into any tolerance (their branch values are below the limit and the header's is not used when the polyline counts match). Engine-level case with a tolerance that uses the coarse value: branch stub R 150, r 148.5, beta 75 deg (eps_rep 0.010032 mm): saddle handedness and declared-delta-side checks are INCONCLUSIVE (test `task_1_2_review_a_coarse_certified_bound_is_inconclusive_never_pass`; it fails without the rule). Changed expectation: `refusal_codes.rs` `accepted_and_verified` accepts G10 INCONCLUSIVE only where a pattern has eps_rep above the limit (one case: beta 179.9997, s_end 4.7e7) |

Note on the compat fixture (regenerated for this review; `crates/nf-core/tests/compat.rs` header): no state, outcome,
refusal, parse result or cut geometry changed; `patterns_sha256` changed for 148 accepted cases (40 eccentric_reducer,
55 cone_frustum, 34 cylinder_shell, 11 branch_stub, 8 header_with_branch), all from the assumption text. Regenerating
with the previous `interval::library` gives the same 148, so the outward margin changed no pattern of the corpus.
The check verdicts are not held by the fixture; the reducer verdicts above follow from the flag (every reducer is uncertified) and from the stress judge.

**Changed test expectations (work plan task 1.2, fix-up after the 2100a22 verification; no engine change, no validation
threshold changed, no planted fault removed).** Each old value and its reason:

| Test | Old expectation | New expectation | Reason |
|---|---|---|---|
| `validation/checks/compare.py`, product verdicts | any verdict other than PASS is defect `PRODUCT_CHECK_NOT_PASS` | FAIL (or an unknown verdict) stays `PRODUCT_CHECK_NOT_PASS` (defect); INCONCLUSIVE is `PRODUCT_CHECK_INCONCLUSIVE` (observation, not in DEFECT_CODES) | the reducer's circumference checks are INCONCLUSIVE by 8.2 (eps_rep not certified): fail closed, not a wrong result. Derivation in the code comment (plan rule A9) |
| `tests/ui_smoke.js`, every family: "all checks PASS" | every verdict is PASS | eccentric reducer: exactly `large_edge_equals_circumference` and `small_edge_equals_circumference` INCONCLUSIVE, all others PASS, none FAIL | 8.2 (above); the names are read from the job, so the list is checked, not counted |
| `tests/ui_smoke.js`, every family: "errors labelled as estimates" (`/estimate, not a bound/` in `#quant`) | that phrase in `#quant` | per-family sentences in `#assume` (job.rs `eps_assumption`): cylinder and cone: the eps_num sentence only; branch and header: eps_num plus "eps_rep is a certified upper bound of the chord error"; reducer: eps_num, "ESTIMATE, not a bound", "eps_rep is NOT certified" | 0.4.4 states stub and opening errors as certified bounds and the cone as an a-priori bound; only the reducer stays an estimate (8.1, 8.2). `#quant` keeps "a-priori bound, not interval-certified" |
| `tests/ui_smoke.js`, task 1.1 notice (EN) | `/estimate, not a bound/` | `/a-priori rounding bound, not interval-certified/` | the notice text was changed in 1.2 (`web/i18n.js` q.epsgov); the old phrase is no longer emitted |
| `tests/ui_smoke.js`, task 1.1 notice (JA) | `/推定値/` | `/先験的な上界であり、区間演算による保証はありません/` | the same change in Japanese (`web/i18n.js` q.epsgov) |
| `tests/e2e_smoke.sh`, job of the eccentric reducer | all checks PASS | the two circumference checks INCONCLUSIVE, all others PASS, none FAIL | 8.2 (above) |
| ja "no English word" walk and `docs/design/tools/i18n_check.mjs` allow-list | `eps_num`, `eps_rep` not allowed (q.hint, a new tooltip) | allowed as whole identifiers only; no other word added. The ui walk removes the two identifiers before its word rule, which splits at `_` | they are deliberate technical names, shown identically in EN and JA |
| `validation/seeded_faults.py` | 33 planted faults | 36: the 33 unchanged, plus three planted product verdicts (FAIL, INCONCLUSIVE, unknown) | the verdict split had no planted fault; the new ones test it |

### 8.9 L11 certified cut geometry for the NFJ-1 new shapes (engine 0.6.0)

Sections 8.1–8.8 certify the five `job::run` families and keep the eccentric reducer withheld. L11 adds a second,
independent route for the NFJ-1 plate shapes reached through `order_compute`, which do not go through `job::run`.

**Proof.** `uem::certify` encloses the exact curve on every parameter cell covering a drawn chord with outward-rounded
interval arithmetic (`uem::interval`). The maximum distance from the four corners of each cell's enclosure box to the
drawn segment bounds the whole cell, because the distance to a segment is convex. Every written vertex is enclosed at
its parameter to cover vertex displacement. Cells are subdivided until each is within `tol + margin`; the reported
value is the largest cell/vertex distance plus a binary64 rounding margin, capped at `tol`. If any cell, vertex, quotient
or square root cannot be enclosed, or the bound exceeds the cap, the shape is not certified.

**What `certified` means.** `certified: true` certifies the cut **outline** only: every drawn edge of the outline and of
each opening lies within the 0.05 mm bound of the exact curve. It does not certify the enclosed area or the extents.
Both are estimates in every family at this engine version and are reported with `area_certified: false` and
`extents_certified: false`.

**Scope.** The 0.05 mm cap is decision D1's implementation limit for these new shapes, not an engineering tolerance;
the 0.01 mm cap of `develop.rs` and 8.1 remains unchanged for the five old families.

| Family | L11 status | Basis |
|---|---|---|
| Tee main, perpendicular thin branches | certified when every hole is certified | exact hole enclosure `(dir + asin((e + r sin θ)/ri), x0 + r cos θ)`; azimuth scaled to plate millimetres; thick and lateral holes withheld |
| Perpendicular tee branch wrap | certified except at wall tangency | exact cut enclosure `(w(θ−π/2)/(2π), h − tee_y(θ))`; a tangent wall gives a square-root singularity and no bound |
| Reinforcing pad, thin wall | certified when both the hole and outline bounds prove | exact enclosed hole and circle outline; a constant-width ribbon adds the hole's chord error to each corner arc's chord error and is withheld when their sum exceeds the cap; thick holes withheld |
| Bend, eccentric cone, cone branch, wye, lateral tee cut, every quadrature-dependent extent or area | withheld (`certified: false`) | no rigorous bound of the exact map and of every drawn edge is available in this lane |

A withheld shape still serializes `eps_rep` as an estimate, `eps_rep_certified: false`, its method, named rules, holds,
questions and taper flags.

**DXF.** `order_compute` writes a cut file only when `dxf_allowed` holds: a certified outline, no hold, question or
refusal, and a single pattern piece (`several_pieces` otherwise). The file has the outline as a closed polyline on
`CUT_OUTER`, each opening as a closed polyline or full circle on `CUT_INNER`, and the not-released marking. The server
digests every such file against its own pattern piece before it is served (`amoeba::gate_order_dxf`); a file that is
not a PASS is withheld and its part gets `digest_failed`. A measured defect is a FAIL, never an INCONCLUSIVE.

**Digest limits.** The digest compares the drawn file with the reference built from the engine's own pattern output.
That is a consistency check, not an independent recomputation from the drawing parameters: a re-derivation of the
exact curves in `nf-digest` is not implemented in this lane. The digest also holds a file whose declared bound is
absent, uncertified or coarser than the 0.05 mm cap.

The engine id is `nf-engine 0.6.0+src.<fingerprint>`; results made by older engines are recomputed.

## 9. Surface declaration of the entered radius (work plan task 1.4, engine 0.4.6)

The entered radius of every family with radii may be declared to refer to a wall surface: `surface_reference` =
`outside`, `inside`, `neutral` or `unknown`. A cylinder shell, a cone and an eccentric reducer take the optional field
(task 1.4); a branch stub and a header with branch already had it. Engine 0.4.6 makes `unknown` act on the verdicts.

- **`unknown`**: every radius-dependent check that would PASS becomes INCONCLUSIVE with the reason
  `uem::verify::SURFACE_UNKNOWN_REASON` ("surface unknown: cannot tell which surface the radius is on, so the radius check
  cannot be certified. / 基準面が不明です: 半径がどの面を指すか判別できないため、半径に関する検査は確定できません。"). A FAIL stays
  FAIL. G10 is INCONCLUSIVE and carries the reason. Cut geometry, patterns and state do not change.
- **`outside`, `inside`, `neutral`**: recorded in G02. Verdicts and geometry are those of the request without the field
  (tested for the cylinder, cone and eccentric reducer; for a branch stub a known surface keeps its verdicts, see the table
  of changed cases below).
- **Absent**: for the three new families, not declared, and the engine behaves exactly as before (the entered radius is
  the reference-surface radius, G06). For a branch stub, absent means `unknown` (its existing default), so it flips like
  an explicit `unknown`. Absent is never `unknown` for the three new families, because that would change every existing
  request of those families.

**Radius-dependent checks.** A check is radius-dependent when its REFERENCE value is computed from an entered radius
(`r_dev`, `r1`, `r2` of the cylinder, cone and reducer; `R`, `r`, `r_dev` of the branch and the header). The lists are in
`uem::verify` (`*_RADIUS_CHECKS`, `BRANCH_PART_*`, `HEADER_OPENING_*`, `INTERFACE_JOINT_CHECKS`); the tests
`tests/surface_declaration.rs` assert that every check the verifier produces is in exactly one list, so a new or renamed
check cannot escape the rule.

| family | radius-dependent (INCONCLUSIVE with `unknown` when PASS) | not radius-dependent |
|---|---|---|
| cylinder shell | `bottom_edge_equals_circumference`, `area` | `seam_edges_equal_length`, `seam_length_equals_input` |
| cone frustum | `large_arc_equals_circumference`, `small_arc_equals_circumference`, `seam_equals_slant_length`, `area`, `large_end_3d_arc_length`, `small_end_3d_arc_length`, `fold_back_radius_residual` | `seam_edges_equal_length`, `outline_counter_clockwise`, `seam_a_on_positive_x` (RC-10) |
| eccentric reducer | `apex_free_development_agrees`, `large_edge_equals_circumference`, `small_edge_equals_circumference`, `seam_equals_3d_ruling`, `area`, `seam_b_at_development_angle` (RC-10: Psi depends on the entered radii) | `seam_edges_equal_length`, `outline_counter_clockwise` (0.4.7), `seam_a_on_positive_x` (RC-10: a fixed direction) |
| branch stub (one declaration for both contact radii) | `cut_points_on_header_surface`, `cut_points_on_branch_surface`, `saddle_handedness_matches_declared_side`, `square_end_equals_circumference`, `saddle_matches_3d_intersection`, `saddle_on_the_declared_delta_side`, `saddle_developed_length_equals_3d_arc_length`, `pattern_area_equals_surface_area`, `square_end_3d_arc_length` | `outline_counter_clockwise`, `seam_edges_equal_length` |
| header with branch, branch part | header radius only: `cut_points_on_header_surface` (header surface); branch radius only: `cut_points_on_branch_surface`, `saddle_handedness_matches_declared_side`, `square_end_equals_circumference`, `square_end_3d_arc_length` (branch surface); both radii: `saddle_matches_3d_intersection`, `saddle_on_the_declared_delta_side`, `saddle_developed_length_equals_3d_arc_length`, `pattern_area_equals_surface_area` (either surface) | `outline_counter_clockwise`, `seam_edges_equal_length` |
| header with branch, header part | header radius only: `outline_extent_around_header`, `shell_edge_equals_circumference`, `opening_closed_simple_inside_shell`, `header_circle_3d_arc_length` (header surface); both: `opening_points_on_branch_surface`, `opening_centre_on_the_declared_delta_side`, `opening_matches_3d_intersection`, `opening_developed_length_equals_3d_arc_length`, `opening_area_equals_surface_area` (either surface) | `outline_extent_along_header_axis`, `seam_edges_equal_length`, `outline_counter_clockwise` |
| header with branch, interface | `branch_cut_lies_on_header_opening`, `header_opening_lies_on_branch_cut`, `opening_on_intersection_of_branch_geometry`, `saddle_on_intersection_of_header_geometry` (either surface) | `parts_declare_the_same_marking_side` |

**Changed expectations of engine 0.4.6 (A2, measured).** The verdict snapshot `tests/fixtures/compat_verdicts.json`
holds the verdicts of all 335 corpus cases; before this change it was recorded and diffed against the 0.4.5 build, and
it differs in exactly six cases, all with `unknown` (explicit or the branch default). No case with a known surface
changed, and no FAIL changed. Each flipped check went PASS -> INCONCLUSIVE with the reason:

| case | request | checks flipped (PASS -> INCONCLUSIVE) |
|---|---|---|
| 7 | branch_stub, field absent (default `unknown`) | the 9 branch radius checks |
| 12 | branch_stub, `unknown` | the 9 branch radius checks |
| 157 | branch_stub, field absent | the 9 branch radius checks |
| 191 | branch_stub, field absent | the 9 branch radius checks |
| 196 | branch_stub, field absent | the 9 branch radius checks |
| 14 | header_with_branch, both parts `unknown` | 22 checks: header part 9 (4 header-radius, 5 joint), branch part 9 (1 header-radius, 4 branch-radius, 4 joint), interface 4 |

The five absent/explicit branch cases are the first flips that matter to users: every branch stub without the field
reads `unknown` (the existing default), so its radius checks are now INCONCLUSIVE. That follows the RB-010 rule. The
owner may instead want a default other than `unknown` for branch stubs; that is a decision, not made here.

**Changed test expectations (A2, old -> new).** (a) `crates/nf-core/src/job.rs` `jobs_run_verify_and_never_approve`: a
branch stub with `unknown` had every check PASS and G10 PASS; now its radius checks are INCONCLUSIVE with the reason and G10
is INCONCLUSIVE. Only checks with the reason may do this. (b) `crates/nf-core/tests/refusal_codes.rs`
`accepted_and_verified`: G10 must be PASS for an accepted branch unless a certified bound is coarse (8.8) or the surface is
unknown (then G10 is INCONCLUSIVE and every reason-bearing check carries the reason). (c) Version expectations
`0.4.5+src.` -> `0.4.6+src.` in `crates/nf-core/tests/plate_conventions.rs` and `crates/nf-server/src/tests.rs:2498`
(the second is outside the nf-core ownership; the same one-line change was made by the 1.3 integration).
(d) `tests/e2e_smoke.sh`: the branch_stub happy-path job (C3) sent no `surface_reference`, which defaulted to `unknown` and
PASSed before 0.4.6; it now declares `"outside"` so the all-PASS smoke stands (the job always implicitly used the
reference-surface radius, G06). This is an input change, not a loosened assertion. The `unknown` path is added, not
removed: new job C6 (branch_stub, `surface_reference` `unknown`) asserts REVIEW_REQUIRED, the 9 radius checks INCONCLUSIVE
each with a reason, the other 2 PASS, none FAIL. C6 is one more REVIEW_REQUIRED job, so the dashboard count assertion
moved `review_required` 4 -> 5 (refused unchanged; the later relative assertion `review_required == calculations - 4` is
unaffected). (e) `tests/ui_smoke.js`: the family loop selected `surface_reference`
`outside` only for header_with_branch; it now does so for branch_stub as well, for the same reason, so the branch form
smoke stays all-PASS. The `unknown` rendering is already exercised (the pre-fix run drew its 9 INCONCLUSIVE rows).

**Engine bump.** 0.4.5 -> 0.4.6: verdicts change for requests with `unknown` on a branch or a header with branch (and for
any branch stub without the field). Results of 0.4.5 are marked for recomputation (G21). The cut geometry and the patterns
of the corpus are unchanged (`tests/fixtures/compat_0.4.0.json` differs only in case 70, see the compat header).

**Seeded faults and the validation corpus.** `validation/run_all.py --spawn target/release/nf-server --quick` on 0.4.6:
PASS; seeded faults 37 of 37 detected; corpus 116 agree, 0 disagree; the 14 INCONCLUSIVE product notes are existing
eccentric-reducer results of task 1.2 (none is a branch or assembly case).

**Not changed.** The branch's `radius_consistency` advisory for `unknown` (G02) stays; the INCONCLUSIVE checks are added
to it, not in place of it. The surface selector and the reason rendering are web work (docs/backend/UI_CHANGES_NEEDED.md,
row 18).

## 10. Orientation guard for cylinder and eccentric (work plan task 1.5a, engine 0.4.7)

Review C RC-09/RC-10 (RB-006): the cylinder and eccentric verifiers held only length and
unsigned-magnitude area checks, every one invariant to a MIRRORED flat pattern, so a mirrored
(wrong-handed) plate passed. The branch and header verifiers already guarded this with
`outline_counter_clockwise` (the outer profile's signed area must have sign +1, counter-clockwise,
Spec 02 §0.3). Engine 0.4.7 adds the same guard to `verify_cylinder`, `verify_eccentric` and
`verify_cylinder_solid`. A genuine development is counter-clockwise and PASSes; its mirror is
clockwise and FAILs. The check is not radius-dependent, so task 1.4's `surface_unknown` never demotes
it (it is in neither `CYLINDER_RADIUS_CHECKS` nor `ECCENTRIC_RADIUS_CHECKS`).

**Changed expectations of engine 0.4.7 (A2, measured).**
- `verify_cylinder`: 4 checks -> 5 (added `outline_counter_clockwise`, sign test, tolerance 0.5).
- `verify_eccentric`: 5 checks -> 6 (added `outline_counter_clockwise`, sign test, tolerance 0.5).
- `verify_cylinder_solid`: 3 checks -> 4 (same guard; only its tests call it, so no corpus case is affected).
- `tests/fixtures/compat_verdicts.json`: regenerated; 75 of 335 cases gained `outline_counter_clockwise` = PASS.
  No existing check's verdict or reason changed; no case flipped PASS -> anything; patterns and cut geometry
  are byte-identical (develop.rs unchanged).
- Test expectations: `verify.rs` `d8_agreement_tolerance...` tolerance vector for CYL-02 gains a fifth slot
  `0.5` for the appended check (the first four are unchanged). The RB-006 wrong-radius solid test now asserts
  the three radius checks (`end_circle_3d_arc_length`, `end_circles_3d_arc_length_equal`,
  `area_equals_sampled_surface_area`) FAIL and `outline_counter_clockwise` PASSes (a wrong radius is not a
  mirror, so the guard correctly does not fire); the fault is still fully detected. A new test
  `outline_guard_rejects_a_mirrored_cylinder_and_eccentric` proves genuine -> PASS, mirror -> FAIL.
- Version strings `0.4.6+src.` -> `0.4.7+src.` in `crates/nf-core/tests/plate_conventions.rs` and
  `crates/nf-server/src/tests.rs:2498` (the second is outside nf-core ownership; the same one-line change
  as tasks 1.3/1.4).

**Engine bump.** 0.4.6 -> 0.4.7: the check list of the cylinder and eccentric families changed; their verdicts
are unchanged for every genuine development. Results of 0.4.6 are marked for recomputation (G21). Patterns and
cut geometry are byte-identical.

## 11. Directed outline, drawn area against the surface, seam datums (work plan task 1.5, RC-10, engine 0.4.8)

Review C RC-10 named three gaps in the cone and eccentric verifiers, and engine 0.4.8 closes all three.
`develop.rs` is unchanged: the verifier re-derives every reference itself (the independence guard
`crates/uem/tests/verifier_independence.rs` still passes), and the cut geometry and patterns are byte-identical.

**(1) Directed congruence.** `apex_free_development_agrees` compared distances from the seam point, which are
invariant to a mirror and leave n-2 shape degrees of freedom free (a bulge that preserves those distances passes).
It now compares DIRECTED coordinates after the best proper rotation and translation (Kabsch, `kabsch_residual`):
a mirror or a non-rigid bulge is named. The name and the tolerance are unchanged,
max(`agree_tol`, eps_num + 10 x verifier quadrature estimate); the quadrature estimate stays the fine/coarse
distance difference (a scalar, frame-independent). Measured: a genuine development's residual ~1e-12 mm, its
mirror ~1e2 mm. A length-mismatched or empty point list gives NaN -> INCONCLUSIVE (fail closed).

**(2) The `area` check compares the DRAWN outline against the 3D surface.** The old primary was the pattern's
own `area` quantity — a self-comparison that can never fail. The primary is now `exact_signed_area` of the drawn
outline: Dekker error-free products (`split27`, `two_product`) and hi/lo Neumaier compensated accumulation remove
the arithmetic noise entirely, so what remains is geometry, not rounding. The sign matters: a mirrored outline has
negative area and FAILs. The reference stays the surface integral (the cone's closed form pi(r1+r2)s, the
reducer's surface integral); develop's `area` QUANTITY keeps its surface-integral value, unchanged.

**Cone area tolerance (old -> new):** `agree_tol x s` -> `agree_tol x s + eps_num x perimeter`.
Derivation (plan rule A9): the written vertices are binary64 roundings of the exact development; the cone's
eps_num = l1 U (16 + 8 Phi) is the A-PRIORI bound of that displacement (`develop.rs`, `EPS_NUM_BASIS_EXACT` —
a bound, not an estimate, so no INCONCLUSIVE demotion). The first-order shape derivative of area under a boundary
displacement gives |dA| <= displacement x perimeter, so eps_num x perimeter bounds the area effect of the written
rounding. Near the apex limit this term dominates: test B2-005 (r1 = 1e5, r2 = 99999 mm, apex radius ~1e8 mm)
has l1 ~ 9.99e7 mm, Phi ~ 6.283e-3, eps_num ~ 1.8e-7 mm, perimeter ~ 1.258e6 mm, allowance ~ 0.22 mm^2. The
measured exact-minus-closed-form residual there is 0.0021646 mm^2 (old tolerance 0.000999 mm^2 FAILed it); with
the arithmetic noise removed by `exact_signed_area` this residual is the rounding of the WRITTEN radii, an input
effect, not an arithmetic error. For ordinary cones the allowance is ~1e-9 mm^2 and the residual <= 2.2e-11 mm^2.
A wrong-radius fault (residual ~200 mm^2) still FAILs (`seeded_fault_wrong_radius_reference`).

**Eccentric area tolerance (old -> new):** `1e-6 x max(area, 1)` -> `eps_rep x perimeter + 1e-9 x max(area, 1)`,
wrapped in `rep_check`. Derivation: the drawn polyline is inscribed in the exact curve; each segment's sagitta is
<= eps_rep (8.1), so the chord deficit of the area is <= eps_rep x perimeter by the same first-order boundary
argument. Measured deficits on the corpus reducers: 1.418 / 0.829 / 0.323 mm^2, against allowances of ~10.4 mm^2
and smaller (eps_rep 4.24e-3 / 2.76e-3 / 1.21e-3 mm, uncertified). Because this family's eps_rep is NOT certified
(8.2), `rep_check` turns a genuine PASS into INCONCLUSIVE by design — the same treatment as its two circumference
checks since task 1.2. A measured difference beyond the tolerance stays FAIL.

**(3) Handedness and seam datums.** New checks (all with `agree_tol`, the sign test with 0.5):
- cone: `outline_counter_clockwise` (the eccentric and cylinder got it in 0.4.7, section 10) and
  `seam_a_on_positive_x`;
- eccentric: `seam_a_on_positive_x` and `seam_b_at_development_angle`.
`ray_run` measures the largest distance of any point of the named seam edge from the stated ray from the apex:
seam_a on +X (a fixed direction, NOT radius-dependent), seam_b at Psi = integral of dpsi over a full turn
(radius-dependent, so it joins `ECCENTRIC_RADIUS_CHECKS` and `surface_unknown` demotes it). A missing seam edge
gives NaN -> INCONCLUSIVE. The CCW and seam_a checks are not radius-dependent and are never demoted.

**Changed test expectations (A2, old -> new; no validation threshold weakened, no planted fault removed).**

| Test | Old expectation | New expectation | Reason |
|---|---|---|---|
| `verify.rs`, cone `area` | primary = pattern's own `area` quantity; tol `agree_tol x s` | primary = exact signed area of the drawn outline; tol `agree_tol x s + eps_num x perimeter` | self-comparison replaced by the surface integral; written-vertex rounding allowance (above). B2-005 now PASSes with state ReviewRequired and the quantity within 1e-3 of pi(r1+r2)s |
| `verify.rs`, eccentric `area` | primary = pattern's own `area` quantity; tol `1e-6 x max(area,1)` | primary = exact signed area; tol `eps_rep x perimeter + 1e-9 x max(area,1)`, wrapped in `rep_check` | chord-deficit allowance (above); genuine reducers are INCONCLUSIVE by design |
| `verify.rs`, `apex_free_development_agrees` | seam-point distances | directed Kabsch residual (name and tolerance unchanged) | (1) above |
| `nf-core/src/job.rs`, `jobs_run_verify_and_never_approve` | only the two circumference checks may be INCONCLUSIVE on a reducer | the reducer's `area` joins them | `rep_check` demotes it by design (8.2) |
| `nf-core/tests/stress.rs`, `by_design` | INCONCLUSIVE allowed only for the two circumference checks (+ unknown-surface radius checks) | the reducer's `area` joins the list | same; FAIL stays a defect everywhere |
| `nf-core/tests/fixtures/compat_verdicts.json` | 0.4.7 verdicts | regenerated (`NF_WRITE_COMPAT_FIXTURE=1`): 55 cone cases gain `outline_counter_clockwise` and `seam_a_on_positive_x` PASS; 95 cases gain `seam_a_on_positive_x` PASS; 40 reducer cases gain `seam_b_at_development_angle` PASS; the reducer `area` flips PASS -> INCONCLUSIVE in all 40 of its cases. No removal, no reason change, no cone flip, no state flip | enumerated against the pre-RC-10 snapshot; header note in `compat.rs` |
| `tests/e2e_smoke.sh`, reducer job C2 | exactly the two circumference checks INCONCLUSIVE | exactly `area`, `large_edge_equals_circumference`, `small_edge_equals_circumference` INCONCLUSIVE, all others PASS, none FAIL | `rep_check` demotes the area check by design (8.2); the list is asserted by name, not counted |
| `tests/ui_smoke.js`, `INCONCLUSIVE_CHECKS` | reducer: the two circumference names | the same two plus `area` | same; the walk compares sorted names and counts, so nothing else moves |

New tests: a distance-preserving bulge (eps = 1e-4 sin(pi i/n), old seam-distance residual < 1e-9) now FAILs the
directed check; a turned plate (0.01 rad) FAILs the datums while lengths, CCW and the directed check PASS;
mirrored and wrong-radius faults still FAIL. The cylinder verifier and its pinned d8 tolerance vector are
untouched. The verifier stays free of `develop.rs` calls (independence guard).

**Engine bump.** 0.4.7 -> 0.4.8 (recorded once, with the other task 1.5 results, in the version history of
`crates/nf-core/src/lib.rs`): check lists and verdicts change as enumerated above; patterns and cut geometry are
byte-identical. Results of 0.4.7 are marked for recomputation (G21).

## 12. Cylinder solid verifier, self-intersection contact, verifier independence (work plan task 1.5, RC-09/RC-13/RC-15, engine 0.4.8)

**RC-09: the cylinder pipeline runs the solid verifier and plate-axis datums.** `verify_cylinder_solid` was
test-only; the pipeline called just `verify_cylinder`, whose length/area checks pass a rectangle turned 90 degrees
with the edge names swapped. The pipeline now calls both and keeps ONE copy of the shared `outline_counter_clockwise`
guard. New checks and tolerances (all `agree = agree_tol(max(r_dev, length))`, section 2):

| Check | Measured | Reference | Tolerance |
|---|---|---|---|
| `end_circle_3d_arc_length` | developed length of `bottom` | 3D chord sums of the end circle in a tilted frame, taken to the limit (no 2*pi*r) | `agree + circle_res` |
| `end_circles_3d_arc_length_equal` | developed length of `top` | the same 3D circle | `agree + circle_res` |
| `area_equals_sampled_surface_area` | signed area of the plate outline | 3D circle length x shell length | `agree x (r_dev + length) + circle_res x length` |
| `bottom_runs_along_x` | largest distance of any point of `bottom` from the X-parallel line through its start | 0 | `agree` |
| `seam_a_runs_along_y` | the same for `seam_a` and the Y-parallel line | 0 | `agree` |

`circle_res` is the chord-sum limit's own residual estimate. An absent or degenerate named edge gives NaN ->
INCONCLUSIVE, never a pass. The three solid radius checks join `CYLINDER_RADIUS_CHECKS`, so an `unknown` surface
makes them INCONCLUSIVE like the rest (section 9). Reproducer: a proper 90-degree turn still passes
`verify_cylinder` but FAILs the datums.

**RC-13: self-intersection checks flag touching and small overlaps.** `profile.rs` and `quality.rs` used strict
crossing tests: an endpoint lying on a non-adjacent segment, or an overlap shorter than the old length gate,
passed. Both directions of endpoint-to-segment distance <= tol now count as self-intersection (profile: fixed
`TOUCH_TOL` 1e-9, matching the flatten dedup tolerance, grid boxes grown by tol; quality: the existing tol,
`candidate_pairs` tol-aware). Grid and brute-force references use the same predicate. This only ever turns a
would-be PASS into a FAIL (fail closed); reproducer tests pin the old blind spots (G11 check path, DXF gate).

**RC-15: the verifier re-derives the engine-error bounds itself.** The 3D-intersection tolerances called
`develop::{s,saddle,opening}_error_bound`. `verify.rs` now owns `engine_*_error_bound`: the same first-order
analysis of the Spec 02 evaluation sequence, re-derived from the inputs with local U/TRIG constants, matching the
engine's op order exactly. `develop` keeps its bounds for the reported eps_num. `Branch`, `ObliqueCone` and
`MAX_CHORD_ERROR_MM` move to a neutral geometry module, re-exported from `develop`.
`crates/uem/tests/verifier_independence.rs` guards both halves: non-test verifier code free of `develop`
imports/calls, and the two bound copies bit-equal on 2000 seeded parameter sets (near-tangent, shallow, obtuse).

**Changed test expectations (A2, old -> new; no validation threshold weakened, no planted fault removed).**

| Test | Old expectation | New expectation | Reason |
|---|---|---|---|
| `nf-core/tests/fixtures/compat_verdicts.json` | 0.4.7 verdicts (after RC-10) | 35 cylinder cases gain the five solid/datum checks, all PASS; no existing verdict, reason or state changed, patterns and cut geometry byte-identical | RC-09; enumerated against the pre-RC-09 snapshot; header note in `compat.rs` |
| `uem/src/verify.rs`, unknown-surface cylinder | the radius checks of `verify_cylinder` demoted | the three solid radius checks demoted too (`CYLINDER_RADIUS_CHECKS`) | RC-09; section 9 treatment extended to the new checks |
| self-intersection (G11, DXF quality gate) | a T-touch or an overlap shorter than the length gate passes | both FAIL | RC-13; strengthening only, new reproducer tests, no existing expectation moved |
| 3D-intersection tolerances | `develop::*_error_bound` | `verify::engine_*_error_bound`, bit-equal | RC-15; no expectation change (2000 seeded sets) |

## 13. The DXF-only mating test (work plan task 1.5, validation)

`validation/checks/mating.py`, wired into `run_all.py` stage 0 beside the seeded-fault self-test: the validator
validates itself. Two plates that the product says form one joint must, from the two DXF files ALONE, fold back
into 3D and meet cut-edge to cut-edge. The verifier calls no development code and reads no result geometry: the
fold-back is re-derived from Spec 02 section 4.4 and the conventions the files state (the full derivation, plan
rule A9, is the module docstring), and the vertex correspondence is SEARCHED (both directions x all n cyclic
shifts), not assumed. This also proves the task 1.3 convention statements suffice to build the part.

**Scope stated by the files.** The test requires `NF-HEADER-SURFACE` = `NF-BRANCH-SURFACE` = `neutral` (then the
stated developed radius IS the contact radius, BE-001) and `NF-JOINT` = `not declared` (then both cuts lie on ONE
curve of the entered radii, the zero-thickness idealisation the file states). A set-on joint cuts the opening at
the branch bore, a different curve, and is outside this test. Every load-bearing statement is parsed and
cross-checked (the circumference in two keys, the half-circumference datum, R_dev against the branch
circumference, delta and beta equal in both files); a missing, unreadable or contradictory statement raises
(fail closed).

**The one unstated placement.** The station is a translation along the header axis that the files do not state.
It is fitted per alignment (t = mean(x_H - x_B); y and z must coincide with no fit) and then checked against the
SUBMITTED `branch_station` — a test input, not an engine output.

**Tolerance (a sensitivity bound, not a fitted number).** The convention sentences write lengths and angles with
at most three decimals (DXF_METADATA.md), so each true value lies within +/-0.0005 mm / +/-0.0005 deg of the
stated one; the drawn vertices are exact round-trip decimals. Perturb each stated scalar the fold-back uses
(header circumference, branch R_dev, delta, beta) by +/- half a unit of the last place, re-fold, take the largest
vertex displacement per channel and sum the four -> D. Each coordinate of either fold differs from the true-value
fold by at most D, a paired vertex by at most 2D per coordinate; the mean-t fit subtracts a mean within +/-2D, so
the x residual is at most 4D and y, z at most 2D: residual <= sqrt(24) D < 5D, fitted station error <= 2D.
The bound is `5D + 1e-9 x size` (the floor covers binary64 arithmetic and the perturbation's second-order terms,
<= ~1e-7 mm at these sizes).

**Measured (engine 0.4.8, REQ R 150 / r 100 / beta 60 / delta 20, station 500, s_end 400, neutral, joint not
declared; 720 vertices, 1440 alignments searched).** Correct joint: residual 1.36e-4 mm <= tol 2.64e-2 mm, fitted
station 500.000000 mm. A second job whose inputs are NOT exact at three decimals (beta 60.0004, delta 20.0004,
r 100.0004) exercises all four rounding channels at once: residual 2.67e-3 mm <= the same bound, fitted station
499.999095 mm. Six planted faults must FAIL and do, each by >= 65x the bound: mirrored branch saddle
(residual 114 mm), header opening scaled x1.02 (2.55 mm), saddle shifted +2 mm along the branch axis (1.73 mm),
stated beta +2 deg (6.97 mm), stated delta +3 mm (3.00 mm), stated header circumference +1 % (6.26 mm). The
stated-value faults are planted by rewriting the 999 lines of the DXF text and re-reading them (the file itself
carries the wrong statement, coherently: the circumference fault rewrites the datum too); the geometry faults are
planted in the DXF-derived geometry, the seeded_faults idiom. A fault that cannot be planted fails the self-test.
