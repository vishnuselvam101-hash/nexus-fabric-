# Mitered (lobster-back) bend: specification and independent values (NFJ-1 lane S1-L6a)

Status: written from first principles before any zumen-kirizu (ZK) bend code was read. The independent values are produced by
`tests/golden/zumen/verify_miterbend.py` (`python3 -I`, standard library only). The comparison with ZK (section 8) was done afterwards, treating ZK as a black box.
Nothing below is a confirmed factory rule. Numbers that are conventions rather than geometry are marked UNVERIFIED and are parameters.

## 1. Scope

A bend of centre-line radius `R` and total angle `A` made of `n` straight cylindrical pieces cut by planes (miter joints). `J = n - 1` joints.
Geometry only: no plate bevel, kerf, weld gap, spring-back or finishing allowance except the one explicit `allow` parameter. Reducing bends (different end diameters) are out of scope here.

## 2. Parameters

| name | meaning | notes |
|---|---|---|
| D, t | outer diameter, wall thickness (mm) | 0 < t < D/2 |
| R | radius of the bend centre line (mm) | > 0 |
| A | total bend angle (deg) | 0 < A < 180 for the geometry; the piece-count rule below stops at 90 |
| n | number of pieces; J = n - 1 joints | from the rule below, or given |
| basis | surface on which the plate is unrolled: inner, mid, outer | radius r = D/2 - t, (D - t)/2, D/2 |
| s1, s2 | straight length added to the first / last piece | measured from the tangent point (arc start / end) |
| allow | extra length added to the centre length of every piece | UNVERIFIED factory number (ZK uses 4 mm per piece, no public source); default 0 |
| seam | azimuth of the longitudinal weld seam, measured from the outer (extrados) generatrix | UNVERIFIED; default 180 deg = inner (shortest) generatrix |

### Piece-count rule (parameter, UNVERIFIED)
From /root/nf-logs/tasks/NFJ-1/FACTORY-RULES-PUBLIC.md (MAFF pipeline design 9.7.1(1)a, medium confidence; a fabricator snippet gives 3/4/5 pieces with cut-offs 45 and 67.5 deg: conflict, low confidence):
table `[(30, 2), (60, 3), (90, 4)]` = (largest angle of the band, inclusive; number of pieces). Angle <= 0, non-finite, or above the last band: refused (no piece count).
The table is an argument (`pieces_for_angle(angle, rule)`), not a constant of the geometry.

## 3. Geometry

Half cut angle: `theta = A / (2 J)`. Each joint deflects the axis by `2 theta`; the cut plane makes the angle `theta` with the cross-section of both neighbouring pieces.
The centre line is the polygon circumscribed about the arc: every leg is tangent to the arc, joint vertices lie at distance `R / cos(theta)` from the arc centre.
Tangent length of a joint: `T = R tan(theta)`.

Centre length of piece k (distance between the centres of its start and end cut planes, along its axis):
- middle piece (two joints): `c = 2 R tan(theta)`
- end piece (one joint, one free end): `c = R tan(theta)` (half of a middle piece) `+ s1` (first) or `+ s2` (last)
- every piece: `+ allow`.

Cut curve of a piece on a cylinder of radius r. Let phi = azimuth around the axis, measured from the outer generatrix (phi = 0 outer, pi inner). With z measured along the axis from the centre of the start plane:
- start edge: `z_s(phi) = - r cos(phi) tan(theta_s)`; end edge: `z_e(phi) = c + r cos(phi) tan(theta_e)`; `theta_s = 0` for the first piece (square end), `theta_e = 0` for the last.
- generatrix length: `g(phi) = c + r cos(phi) (tan theta_s + tan theta_e)`. Outer (phi=0) and inner (phi=pi) of a middle piece: `2 (R +/- r) tan(theta)`.
- swing (amplitude of one cut curve): `r tan(theta)`.

Basis (mid / inner / outer): the same formulas with `r = basis radius`, for both the circumference and the swing.

## 4. Developed sheet

x = r phi, y = z, over one circumference `C = 2 pi r`, starting at the seam: `phi = seam + x / r`. The sheet is bounded by the start curve `y_s(x)`, the end curve `y_e(x)`, and two vertical seam edges of length `g(seam)`.
Area `= C c` (the cosine terms integrate to zero); mass `= C c t rho` with rho = 7.85e-6 kg/mm3 (rho is NF's default, DESIGN D10).
The curves close: `y(2 pi r) = y(0)`.

## 5. Checks implemented (verify_miterbend.py)

One pass/fail counter, two independent routes:
- Route A, closed forms: tan(theta), tangent lengths, generatrix lengths at 0 and pi, area, polygon length versus the arc.
- Route B, 3D geometry: the polygon is built from the plane geometry (vertices at R / cos(theta)); for each piece the cylinder is intersected with its two cut planes by a line-plane solve per azimuth (no tan formula), and the generatrix lengths and centre-to-centre lengths are compared with route A.
Groups: (1) t -> 0 limit: the three bases coincide and equal the plain oblique cut of a cylinder (z = -x tan(theta)), and A -> 0 gives flat cuts; (2) closed forms at 90 deg with 2 joints (3 pieces, tan 22.5 = sqrt2 - 1), 1 joint (2 pieces, tan 45 = 1) and 3 joints (tan 15 = 2 - sqrt3), plus polygon-to-arc convergence; (3) neighbouring pieces share their cut ellipse in 3D, developed area equals C c by quadrature, seam edges, additivity of `allow`, and the piece-count rule including its refusals.
Mutation test: changing the middle centre length to `2.1 R tan(theta)` gives 510 NG; letting the last piece have an end cut gives 432 NG.

## 6. Not specified / open

Bevel and gap at the joints, which surface the cut plane meets for thick plates, seam position and stagger (public sources: none), `allow`, the piece-count rule, angles above 90 deg, straights shorter than the weld gap, and stock-plate fit are factory questions.

## 7. Port notes

The values in section 5 are the acceptance values for `MiterBend`: same `theta`, centre lengths, generatrix lengths, developed width and area for each basis; the seam azimuth and `allow` are named rules printed in the DXF conventions block.

## 8. Comparison with ZK (not fixed on either side)

Method: `tests/golden/zumen/tools/compare_zk_bend.py /root/zumen-kirizu` calls ZK's `build()` on 54 cases (9 geometries x 3 bases x allow 0/4) as a black box and compares piece width, tallest generatrix and mass with this spec. One more case compared the full cut curve (723 points) of a middle piece.

Result: `compared 126 pieces in 54 cases (width, max generatrix, mass)`; width differences 0; mass differences 0 (including allow 4 mm, so ZK also adds `allow` to the centre length); piece counts equal (ZK: `joints` = cuts, pieces = joints + 1, same as here).

Differences:
1. Swing radius. ZK's rule `amp=outer` makes the cut curve swing use the OUTER radius D/2 even when the basis is mid or inner, while the width uses the basis radius. This spec uses the basis radius for both. Tallest generatrix differs by `(D/2 - r) tan(theta)` per cut: e.g. D 1016, t 12, R 1500, 90 deg, 3 pieces, mid: spec 829.3 / 1658.5 mm, ZK 831.7 / 1663.5 (diff 2.4 / 5.0); inner basis: diff 4.9 / 10.0; outer basis: 0. All 84 height differences vanish when the swing radius is D/2 (84 of 84), and the 723-point cut curve of a middle piece matches `-k cos(phi)` with `k = (D/2) tan(theta)` to 4e-13. Which one is right for a rolled thick plate is a factory question.
2. Seam default. ZK default: seam 45 deg from 地 (the inner side), i.e. azimuth 135/225 deg from the outer generatrix (fitted: 225 deg, x running the other way, which is the mirror image). Spec default 180 deg (inner generatrix). Both are parameters.
3. Piece count. ZK reads `joints` from the drawing; there is no angle rule. The spec takes the public rule as a parameter and refuses above 90 deg; ZK builds 120 deg (its case 4 joints) without refusing (ZK STATUS item L1: upper bound open).
4. Refusals. ZK raises `stock_fit` for D 2641.6 x t 16 (plate does not fit 2438 x 6096); stock fit is not part of this spec.
5. In the points returned by ZK's `Z.unfold` the lowest y is -136.1 (not 0) for the checked piece, i.e. z is measured from the start plane centre exactly as in this spec; any shift to 0 happens later (not examined).
