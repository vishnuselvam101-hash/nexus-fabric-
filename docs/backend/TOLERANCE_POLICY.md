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
| Area checks | `agree_tol` × (r + L), or × slant length; 1e-6 × area for the oblique cone | Relative to a length of the part | same | An area error is a length error times a length |
| Oblique cone outline | max(`agree_tol`, eps_num + 10 × verifier quadrature estimate) | Absolute or estimate-driven | `verify_eccentric` | Both sides are numerical integrals; the threshold may not be below their own estimated error |
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

## 6. Reported errors are estimates, not bounds

| Value | What it is | What it is not |
|---|---|---|
| `eps_num` | An estimate of rounding and quadrature error of the pattern coordinates | Not a bound. Validation measured true errors of 3.4 to 6.1 times the value within 2e-6 R of tangency and 1.9 times for one very flat oblique reducer (D4). Absolute sizes: below 2e-7 mm |
| `eps_rep` | The largest distance from the exact curve to the polyline at three interior samples of each of the 720 segments | Not a bound: the true maximum can lie between samples. With three samples the value is within 8 % of a 64-sample measurement for the two near-tangent cases of D3 (the midpoint-only estimate of earlier builds was low by 21 %). The representation error itself still grows toward tangency (0.1 to 0.2 mm at r/R = 0.99999, R = 150) because vertices are not refined there; no gate acts on it |
| `points_max_deviation` of an arc in the JSON result | The exact sagitta of the 96 display chords | The JSON points of an arc are for display; the DXF carries the exact arc (D7) |

The result, gate G08 and the interface say "estimate, not a bound" for both values.

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
