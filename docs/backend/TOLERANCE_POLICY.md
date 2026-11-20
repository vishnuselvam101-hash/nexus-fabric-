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
| Chord-error cap | 0.01 mm with at most 46 080 segments | `develop.rs` `MAX_CHORD_ERROR_MM`, `MAX_SEGMENTS` | Kept (B2-005 point e). Implied bound: the sagitta R(1 − cos(π/n)) ≤ 0.01 mm gives R ≤ 0.02·n²/π² ≈ 4.3e6 mm for a full-turn polyline |
| Seam clearance reported (not a refusal) | every cone and reducer states `seam_gap` (the flat-pattern gap at the small end) and `seam_gap_resolution`; the profile gate G11 is INCONCLUSIVE, never PASS, when the gap is below the resolution | `develop.rs`, `job.rs` | Q2(b), engine 0.4.2. Resolution (derived, not tuned): right cone `max(1e-9 mm, 2·eps_num)` (its arcs are exact entities and its seam edges straight lines, but the written vertices carry the numerical error `eps_num` = l1·U·(16 + 8Φ), which grows with the size of the part, so two seam edges closer than twice that cannot be told from a closed seam nor drawn apart in a DXF; the arc display sagitta plays no part); eccentric reducer `max(1e-9 mm, 2·eps_rep, 2·eps_num)` (its ends are polylines, so a clearance below twice their representation error cannot be told from a closed seam). Below the resolution G11 is INCONCLUSIVE, never PASS, and G15 may also reject the job (the DXF cannot represent the seam; REJECTED, fail-closed). The refusal at 1e-9 mm is kept; for a small cone 2·eps_num is far below 1e-9 mm, so the resolution is 1e-9 mm and the band is empty, for a large cone (apex radius 1e7 mm: 2·eps_num about 1.5e-7 mm) the band 1e-9 mm to 2·eps_num is a result with G11 INCONCLUSIVE. Until the 0.4.2 review fix the cone resolution was the bare 1e-9 mm, which gave G11 PASS on seams the DXF cannot represent (caught by G15 only). Corpus cases `B-cone-seam-vs-eps-num-in-band` / `-above-band`, seeded faults in `validation/seeded_faults.py` |

## 6. Reported errors are estimates, not bounds

| Value | What it is | What it is not |
|---|---|---|
| `eps_num` | An estimate of rounding and quadrature error of the pattern coordinates | Not a bound. Validation measured true errors of 3.4 to 6.1 times the value within 2e-6 R of tangency and 1.9 times for one very flat oblique reducer (D4). Absolute sizes: below 2e-7 mm |
| `eps_rep` | The largest distance from the exact curve to the polyline at three interior samples of each of the 720 segments | Not a bound: the true maximum can lie between samples. With three samples the value is within 8 % of a 64-sample measurement for the two near-tangent cases of D3 (the midpoint-only estimate of earlier builds was low by 21 %). The representation error itself still grows toward tangency (0.1 to 0.2 mm at r/R = 0.99999, R = 150) because vertices are not refined there; no gate acts on it |
| `points_max_deviation` of an arc in the JSON result | The exact sagitta of the 96 display chords | The JSON points of an arc are for display; the DXF carries the exact arc (D7) |

The result, gate G08 and the interface say "estimate, not a bound" for both values.

## Changed validation rules (engine 0.4.2 review; old -> new -> reason)

| Rule | Old | New | Reason |
|---|---|---|---|
| `validation/run_all.py` `check_relation` `stub_equals_branch` | the whole branch DXF text of the stub equalled the whole branch DXF text of the assembly part | only the cut geometry is compared: the ENTITIES section up to the first TEXT | since engine 0.4.1 the annotation of an assembly part also states the joint, so the TEXT notes legitimately differ. The slice is complete: `nf_dxf::write_stated` writes every geometry entity first and every annotation after (test `cut geometry must be written unchanged and before any annotation` in `nf-dxf`), so no cut entity can follow the first TEXT. The cut geometry itself is compared exactly (byte-identical), nothing was loosened |
| `validation/stability.py` `N-magnitude-*` `inside` bound | -26 <= n <= 16 for every family | -26 <= n <= 16 for cone/cylinder/branch, -26 <= n <= 14 for the eccentric reducer and the assembly | polyline families are limited by the chord-error cap (R(1 - cos(pi/46080)) <= 0.01 mm gives a developed radius of at most about 4.3e6 mm, kept in 0.4.2); the base parts (200 mm) exceed it from a scale of 2^15 (6.6e6 mm), so those scales are correctly refused, not a defect. Outside-range cases still must be refused or correct |
| `validation/checks/compare.py` `seam_resolution_check`, cone floor | constant 1e-9 mm | `max(1e-9, 2·eps_num)` from the product's reported `eps_num`, with `eps_num` required to be at least extent·2^-53 (computed from the DXF) | the constant mirrored the engine and was too small for large cones (see the seam row above) |
