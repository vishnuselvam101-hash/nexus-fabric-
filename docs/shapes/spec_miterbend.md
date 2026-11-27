# Mitered (lobster-back) bend: specification and independent values (NFJ-1 lane S1-L6a)

Status: written from first principles before any zumen-kirizu (ZK) bend code was read. The independent values are produced by
`tests/golden/zumen/verify_miterbend.py` (`python3 -I`, standard library only; currently 4007 OK, 0 NG). The comparison with ZK (section 8) was done afterwards, treating ZK as a black box.
Round 1 (review REVIEW-1, 2026-11-27) changed the joint count source, the swing rule, the seam and allow defaults and the refusals, after the ZK comparison; section 8 records the parity that was re-measured.
Nothing below is a confirmed factory rule. Numbers that are conventions rather than geometry are named rules and are marked UNVERIFIED.

## 1. Scope

A bend of centre-line radius `R` and total angle `A` made of `n` straight cylindrical pieces cut by planes (miter joints). `J = n - 1` joints.
Geometry only: no plate bevel, kerf, weld gap, spring-back or finishing allowance except the one explicit `allow` parameter. Reducing bends (different end diameters) are out of scope here.

## 2. Parameters

| name | meaning | notes |
|---|---|---|
| D, t | outer diameter, wall thickness (mm) | 0 < t < D/2 |
| R | radius of the bend centre line (mm) | > 0 |
| A | total bend angle (deg) | 0 < A < 180 (geometry). No upper limit at 90 deg: a bend above 90 deg is allowed when the drawing gives its joints (flagged if the drawing gives none) |
| J (joints) | number of joints; n = J + 1 pieces | read from the drawing (source box). This is the input. |
| basis | surface on which the plate is unrolled: inner, mid, outer | radius r = D/2 - t, (D - t)/2, D/2 |
| bend_swing_radius | named rule: radius of the cut-curve swing | `basis` (default, r) or `outer` (D/2, ZK behaviour). Used only in the cut-curve heights. The circumference and the developed width always use r. Factory to confirm (decision D2). |
| s1, s2 | straight length added to the first / last piece | measured from the tangent point (arc start / end) |
| allow | extra length added to the centre length of every piece | named rule `ALLOW_PER_PIECE_MM` = 4 mm, UNVERIFIED (ZK parity, DESIGN 2.4; no public source). Geometry-only checks pass 0 explicitly. |
| seam | azimuth of the longitudinal weld seam, measured from the outer (extrados) generatrix | named rule `SEAM_DEG` = 135 deg = 45 deg from the inner generatrix (180), UNVERIFIED (ZK parity). Fitted to ZK's curve, section 8. |

### Joint count: drawing first, table as fallback (UNVERIFIED)
The joint count J is read from the drawing. If the drawing has none, the fallback table `[(30, 2), (60, 3), (90, 4)]` (MAFF 9.7.1(1)a, medium confidence; a fabricator snippet gives 3/4/5 pieces with cut-offs 45 and 67.5 deg: conflict, low confidence) gives a count, and the result carries a question to the engineer: "joints not on the drawing: confirm". Above the table, `pieces_for_angle` refuses (no count is invented). The table is an argument (`pieces_for_angle(angle, rule)`), not a constant of the geometry.

## 3. Geometry

Half cut angle: `theta = A / (2 J)`. Each joint deflects the axis by `2 theta`; the cut plane makes the angle `theta` with the cross-section of both neighbouring pieces.
The centre line is the polygon circumscribed about the arc: every leg is tangent to the arc, joint vertices lie at distance `R / cos(theta)` from the arc centre.
Tangent length of a joint: `T = R tan(theta)`.

Centre length of piece k (distance between the centres of its start and end cut planes, along its axis):
- middle piece (two joints): `c = 2 R tan(theta)`
- end piece (one joint, one free end): `c = R tan(theta)` (half of a middle piece) `+ s1` (first) or `+ s2` (last)
- every piece: `+ allow`.

Cut curve of a piece. Let phi = azimuth around the axis, measured from the outer generatrix (phi = 0 outer, pi inner). With z measured along the axis from the centre of the start plane, and `rs` the swing radius of the rule `bend_swing_radius` (rs = r for `basis`, D/2 for `outer`):
- start edge: `z_s(phi) = - rs cos(phi) tan(theta_s)`; end edge: `z_e(phi) = c + rs cos(phi) tan(theta_e)`; `theta_s = 0` for the first piece (square end), `theta_e = 0` for the last.
- generatrix length: `g(phi) = c + rs cos(phi) (tan theta_s + tan theta_e)`. Outer (phi=0) and inner (phi=pi) of a middle piece: `2 (R +/- rs) tan(theta)`.
- swing (amplitude of one cut curve): `rs tan(theta)`.

## 4. Developed sheet

x = r phi, y = z, over one circumference `C = 2 pi r`, starting at the seam: `phi = seam + x / r` (x increases with phi, no mirror image). The sheet is bounded by the start curve `y_s(x)`, the end curve `y_e(x)`, and two vertical seam edges of length `g(seam)`.
Area `= C c` (the cosine terms integrate to zero); mass `= C c t rho` with rho = 7.85e-6 kg/mm3 (rho is NF's default, DESIGN D10).
The curves close: `y(2 pi r) = y(0)`.

## 5. Checks implemented (verify_miterbend.py)

One pass/fail counter, two independent routes:
- Route A, closed forms: tan(theta), tangent lengths, generatrix lengths at 0 and pi for both swing rules, area, polygon length versus the arc.
- Route B, 3D geometry: the polygon is built from the plane geometry (vertices at R / cos(theta)); for each piece the cylinder of the basis radius is intersected with its two cut planes by a line-plane solve per azimuth (no tan formula), and the generatrix lengths and centre-to-centre lengths are compared with route A. Route B is one cylinder of radius r, so it checks the swing rule `basis` only. The `outer` rule is checked in route A only.
Groups: (1) t -> 0 limit: the three bases coincide and equal the plain oblique cut of a cylinder (z = -x tan(theta)), and A -> 0 gives flat cuts; (2) closed forms at 90 deg with 2 joints (3 pieces, tan 22.5 = sqrt2 - 1), 1 joint (2 pieces, tan 45 = 1) and 3 joints (tan 15 = 2 - sqrt3), the swing = outer closed forms (2(R+D/2)(sqrt2-1) etc.), plus polygon-to-arc convergence; (3) neighbouring pieces share their cut ellipse in 3D, developed area equals C c by quadrature, seam edges, additivity of `allow`, and the joint-count fallback with its refusal above the table.
Result of this round: `verify_miterbend: 4007 OK, 0 NG`.
Mutation check (round 1, the one re-run): centre length of the middle piece changed to `2.1 R tan(theta)` (in a copy of the file) gives `verify_miterbend: 3494 OK, 513 NG`. The round-0 second mutation (end piece with an end cut, 432 NG) was not re-run in round 1.

## 6. Not specified / open

Bevel and gap at the joints, which surface the cut plane meets for thick plates, seam position and stagger (public sources: none; the seam default is ZK's, not a source), `allow` (no public source), the joint count when the drawing has none, `bend_swing_radius` (which radius a rolled thick plate needs), straights shorter than the weld gap, and stock-plate fit are factory questions.

## 7. Port notes

The values in section 5 are the acceptance values for `MiterBend`: same `theta`, centre lengths, generatrix lengths, developed width and area for each basis and each swing rule. The three named rules (`bend_swing_radius`, `allow`, `seam`) are printed in the DXF conventions block and on the cut sheet, with their status (confirmed / UNVERIFIED).

## 8. Comparison with ZK (not fixed on either side)

Methods (all black box through node, ZK read only):
- `tests/golden/zumen/tools/compare_zk_bend.py /root/zumen-kirizu <basis|outer>` calls ZK's `build()` on 54 cases (9 geometries x 3 bases x allow 0/4) and compares piece width, tallest generatrix and mass with this spec.
- The same script's curve part calls `Z.unfold` (`tools/zk_bend_curve.mjs`, middle piece, 723 points) and compares the lower and upper edges with this spec, point by point.

Results (ZK commit f718c34):

| spec swing rule | width / mass | heights (max generatrix) | cut-curve points (723, middle piece, default seam) |
|---|---|---|---|
| `outer` | 0 / 0 differences of 126 pieces | 0 differences | max 7e-13 mm for mid, inner and outer bases |
| `basis` (default) | 0 / 0 | 84 differences, e.g. D 1016 t 12 R 1500 90 deg 3 pieces mid: spec 829.3 / 1658.5 mm, ZK 831.7 / 1663.5 (diff 2.4 / 5.0) | max 2.49 mm (mid), 4.97 mm (inner); about 7e-13 mm for the outer basis |

So ZK's bend is exactly the spec with `bend_swing_radius = outer`, default seam 135 deg, +x direction (the fit: ZK's lower edge equals `-(D/2) cos(135 deg + x/r) tan(theta)` to 5e-13 mm; the equivalent 225 deg with x reversed is the same curve, not a mirror). The default `basis` differs from ZK only in the swing radius, which is the factory question D2.

Other differences:
1. Piece count. ZK reads `joints` from the drawing and has no angle rule. The spec now does the same (section 2). ZK builds a 120 deg bend (its case with 4 joints) without refusing; the spec builds it too (`A` above 90 deg with joints from the drawing).
2. Refusals. ZK raises `stock_fit` for D 2641.6 x t 16 (plate does not fit 2438 x 6096) in 6 of the 54 comparison cases; stock fit is not part of this spec.
3. In the points returned by ZK's `Z.unfold` the lowest y is -136.1 (not 0) for a checked piece, i.e. z is measured from the start plane centre as in this spec; any shift to 0 happens later (not examined).
