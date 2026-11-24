# Review B2 — computational review of the pattern engine

Reviewer: independent computational reviewer (applied mathematics, fabrication geometry).
Scope: the frozen copy of commit 09dab50 (git worktree, removed at the end). Engine: `crates/uem` (`develop.rs`, `profile.rs`, `lib.rs`; `verify.rs` and `independent.rs` not used as evidence), gates and JSON in `crates/nf-core/src/job.rs`, DXF output of `nf-dxf`. The main tree's engine was not read. Derivations were written before reading the review documents; those documents were read afterwards for comparison only.

Method in one line: release build of the frozen copy, `nf-server` started from it on 127.0.0.1:8840 and 8841 with fresh data directories, requests submitted through the HTTP API, results compared with my own closed forms and quadratures (numpy, and mpmath at 50 digits for spot checks). Scripts and raw results are listed in section 10.

---

## 0. Verdicts, scores, principal findings

| Family | Verdict | Accepted cases tested | Worst measured error | Reservations |
|---|---|---|---|---|
| Cylinder shell | correct as far as tested | 205 | area and corner error 0 (exact) | none material |
| Cone frustum | correct as far as tested | 225 | L1, L2, sector angle, slant, area: relative 6.9e-14 or less | JSON display arcs (B2-003); over-strict refusals (B2-005) |
| Eccentric reducer | correct with reservations | 200 | developed angle 7.5e-15 relative; vertices 4.7e-10 mm (reference-limited); chord 9.97e-3 mm (limit 0.01) | B2-002, B2-005, B2-006, B2-007 |
| Branch stub (saddle cut) | correct with reservations | 200 + designed | vertices 9.1e-13 mm; chord 8.1e-3 mm random, 8.8e-3 mm near tangency | B2-001 (developed radius accepted unbounded), B2-002, B2-005, B2-006 |
| Header with opening and joint | correct with reservations | 200 | joint closes in 3D to 1.9e-12 mm; hole grid 1.9e-12 mm | B2-001, B2-002, B2-004 |

**Scores.** Mathematical correctness: **8/10**. Every family's development agrees with my independent derivation to 1e-12 mm or better in the closed-form parts, and the joint closes in 3D. Not higher because the radius model accepts developed sizes that no wall can produce (B2-001, high), because polyline deviations are reported as zero (B2-002), and because the offset and tangency conventions can only be confirmed by a fabrication engineer. Numerical robustness: **8/10**. Closed forms are accurate to about 1e-13 mm, quadrature to 5e-10 mm, the 0.01 mm density rule holds in every accepted case, and `eps_num` held in every 50-digit check. Not higher because `eps_rep` is a 15-point estimate rather than a bound (B2-007), two quantities are sampled or quadrature-limited (B2-006), and several refusals are stricter than the arithmetic requires (B2-005).

**Five most serious findings** (full reproductions in section 4):
1. **B2-001 (high):** the surface-reference rule accepts any development radius on the permitted side of the contact radius. A branch with contact radius 50 mm and development radius 0.5 mm is accepted with developed width 3.1416 mm against a contact circumference of 314.16 mm.
2. **B2-002 (medium):** the JSON marks every polyline (eccentric, branch, header hole) as exact with zero deviation, while the pattern's own `eps_rep` is 1.6e-3 mm for the reducer in section 4 and up to 9.97e-3 mm in the random set.
3. **B2-005 (low, but systematic):** refusals stricter than the mathematics requires: cones with sin α below 1e-3 (closed forms exact to 0 at 1e-4), shallow cones within 0.1 % of a turn, branch angles below 9.79°, and a chord-error cap that refuses radii above about 4 km while the stated magnitude limit is 1e8 mm.
4. **B2-003 (low, display only):** arcs are shown with 96 chords whose deviation grows with radius; for a 2.2e8 mm arc the displayed chords deviate by 23,946 mm. The DXF arcs are exact (verified).
5. **B2-004 (low, label):** the header's `area` is the gross plate `L·2πR`, with the opening not deducted (7.5 % above the net in the example).

---

## 1. Derivations (first principles, written before reading the documents)

Conventions are listed at the end of each family. "Declared" means the code states the choice in its conventions text.

### 1.1 Cylinder shell
Surface S(θ,z) = (R cos θ, R sin θ, z), θ ∈ [0,2π), z ∈ [0,L]. Metric R²dθ² + dz². Chart (u,v) = (Rθ, z) is an isometry onto the rectangle [0,2πR]×[0,L]. Orientation: ∂_θS × ∂_zS = R·n_out, so the chart seen from outside is right-handed with the plate normal. Area 2πRL. Seam: u = 0 and u = 2πR.
Conventions: outside view (declared, `MARKING_SIDE`, develop.rs:126); origin at a corner; seam along v.

### 1.2 Cone frustum
r1 > r2 > 0, h > 0, s = √(h² + (r1−r2)²), sin α = (r1−r2)/s. Generator lengths from the apex: L1 = r1/sin α = r1·s/(r1−r2), L2 = r2/sin α; L1 − L2 = s. Developed sector angle Φ = 2π sin α. Chart (ℓ, θ) ↦ (ℓ cos ψ, ℓ sin ψ), ψ = θ sin α, is an isometry (the circumference at generator distance ℓ is 2πℓ sin α = ℓΦ). Orientation: ∂_ℓS × ∂_θS = ℓ sin α·n_out, so seen from outside, ψ increases counter-clockwise. Area = (Φ/2)(L1² − L2²) = π(r1+r2)s.
Conventions: angle datum on the +X axis, counter-clockwise, outside view (declared, develop.rs:209–210).

### 1.3 Eccentric reducer (oblique cone frustum)
Big circle C1: centre (0,0,0), radius r1, plane z=0. Small circle C2: centre (e,0,h), radius r2, plane z=h. Points P1(θ) = r1(cos θ, sin θ, 0), P2(θ) = (e + r2 cos θ, r2 sin θ, h). The homothety with ratio k = r2/r1 about V = (a, 0, hh), a = e·r1/(r1−r2), hh = h·r1/(r1−r2), maps P1(θ) to P2(θ) for every θ. So the same-θ generators all pass through V: the surface is an oblique cone with apex V.
Parametrisation X(t,θ) = V + t(P1(θ) − V), t ∈ [k,1]. Generator length ρ(θ) = |P1(θ) − V|, ρ² = (r1−a)² + hh² + 4a·r1·sin²(θ/2) (no cancellation). Developed angle element dψ/dθ = |P1′ × (P1−V)|/ρ² = r1·√(hh² + (r1 − a cos θ)²)/ρ², and the developed polar chart is (tρ(θ) cos ψ(θ), tρ(θ) sin ψ(θ)), ψ(θ) = ∫₀^θ dψ. Total angle Ψ = ψ(2π), not closed form. Developed length of the big edge = ∫ r1 dθ = 2πr1 (exact); generator seam length = |P1(0) − P2(0)| = √((r1 − e − r2)² + h²). Area = ((1−k²)/2) ∫₀^{2π} r1√(hh² + (r1 − a cos θ)²) dθ. Orientation: at θ = 0 the normal ∂_tX × ∂_θX has a positive x-component (outward; exactly +x for the flat generator e = r1 − r2); ψ increases with θ, so the outside view is counter-clockwise. The big edge is a non-circular polar curve unless e = 0.
Conventions: seam at θ = 0 on the side toward which the small end is offset (declared, develop.rs:370, 459–472); e ≥ 0 only (develop.rs:375).

### 1.4 Branch stub (saddle cut)
Header axis = x, header surface y² + z² = R². Branch axis through (0, δ, 0) with direction a = (cos β, 0, sin β). Branch surface (code's frame, Branch::point develop.rs:515–522): B(φ,s) = (s cos β − r sin φ sin β, δ + r cos φ, s sin β + r sin φ cos β). Intersection with the header, upper sheet (z > 0): s(φ) = [√(R² − (δ + r cos φ)²) − r sin φ cos β]/sin β. It exists for every φ iff |δ| + r < R. The stub is the part s ∈ [s(φ), s_end].
Chart (φ, s) ↦ (rφ, s): ∂_φB × ∂_sB = r·n_out, so the outside view is (rφ, s). Developed saddle length = ∫√(r² + s′²) dφ, which equals the 3D length only if the developed radius equals the contact radius. Area = r ∫(s_end − s(φ)) dφ.
Conventions: outside view and the square end at the top (declared, develop.rs:641–644); the sign of δ and β in words (declared, `delta_notes`, develop.rs:136–150, review item RB-002).

### 1.5 Header with branch opening, and the joint
Header chart (X, Y): X = axial coordinate, Y = r_h(ω + π), ω = asin(y/R) on the branch side. In 3D, (X, Y) ↦ (X, R sin a, R cos a) with a = Y/r_h − π. Outward normal (0, sin a, cos a); ∂_X × ∂_Y = outward (checked analytically). Hole boundary: X(φ) = st + s(φ) cos β − r sin φ sin β, Y(φ) = r_h(asin((δ + r cos φ)/R) + π). Joint identity: the header hole, mapped to 3D, equals the branch saddle B(φ, s(φ)) exactly, because (δ + r cos φ, √(R²−y²)) is the same point on both surfaces. The hole is traversed clockwise (φ decreasing), as holes are declared to be clockwise (lib.rs). Plate area L·2πr_h (gross).
Conventions: length along x, circumference along y; ω increases from −y to +y (declared, develop.rs:782–800).

---

## 2. Comparison with the code (file:line)

| Item | Code | Status |
|---|---|---|
| Cylinder rectangle | develop.rs:153–172 (w = 2π r_dev, corners) | agrees with 1.1 |
| Cone: s, sin α, L1, L2, Φ | develop.rs:224–230 | agrees with 1.2 |
| Cone refusals | develop.rs:226 (sin α < 1e−3), 233–236 (Φ ≥ 0.999·2π) | threshold too strict (B2-005); comment at 231–232 says the sector "closes on itself" at 0.1 % of a turn, which is true only at 2π: justification wrong |
| Cone area | develop.rs:303 `0.5·φ·(l1² − l2²)` | agrees |
| Oblique apex | develop.rs:321–324 (a = e·k, hh = h·k, k = r1/(r1−r2)) | agrees with 1.3 |
| ρ² without cancellation | develop.rs:328–332 | agrees (identity checked numerically) |
| dψ/dθ | develop.rs:337–342 `r1·hypot(hh, r1 − a cos θ)/ρ²` | agrees |
| Developed table, compensated sum | develop.rs:349–362 | agrees; compensated sum, error 5e-14 mm at 50 digits |
| Eccentric refusals | develop.rs:375 (e ≥ 0), 381, 384 (sin α < 1e−3), 392–398 (Ψ ≥ 0.999·2π) | 384 and 394 too strict (B2-005); the overlap test is by total angle only |
| Inner edge = k × outer | develop.rs:409–415 | agrees (homothety about the apex, the development origin) |
| Eccentric area quadrature | develop.rs:419–422 (fixed 720 panels) | not converged for near-flat cases (B2-006) |
| Eccentric eps_num | develop.rs:423–432 | holds in all 50-digit checks (section 3) |
| Density | develop.rs:58–73, 79–104 (15 interior samples, doubling to 46 080) | rule met in all tests; returns a sampled estimate, not a bound (B2-007) |
| Branch s(φ) and point | develop.rs:509–522 | agrees with 1.4 |
| Branch refusals | develop.rs:619 (sin β > 0.17), 622–624 (\|δ\| + r < R(1 − 1e−6)), 632–640 (s_end > peak of 2 881 samples) | angle limits too strict (B2-005); peak check on samples (U2) |
| Developed abscissa | develop.rs:650 `r_dev·φ` with s(φ) computed from r | isometric only if r_dev = r; enforced only for surface_reference "neutral" (job.rs:1223–1226); see B2-001 |
| Header hole | develop.rs:802–815: X at 810, Y = r_dev(asin(y/R) + π) at 808–811 | agrees with 1.5 |
| Header opening refusal | develop.rs:816–830 (strictly inside plate) | agrees; strict at the boundary (harmless) |
| Header area | develop.rs:197 (w·length, inherited), opening_area at 891 | gross area labelled "area" (B2-004) |
| JSON arc display | job.rs:297 (96 chords), 319, 323–330 | display only; deviation formula exact; flags wrong for polylines (B2-002, B2-003) |
| Radius rule | job.rs:1199–1231; outside: 1214–1217 (dev > contact refused, nothing else); 1217 `Ok(None)` | no lower bound: B2-001 |
| DXF | verified against JSON (section 6) | agrees |

---

## 3. Numerical tests

All accepted cases were compared with my closed forms or with quadratures (numpy Gauss–Legendre, double; mpmath at 50 digits for spot checks). Chord error was measured as the largest distance from the exact curve to the polyline chord, at 20 interior points per segment and on a dense grid (200 per segment; for the knee cases a refinement of the maximum inside each segment). "Dense" values are the reference for B2-007 and the density rule.

| Family | Batch | Accepted | Quantity | Max | Median |
|---|---|---|---|---|---|
| Cylinder | batch1 | 205 | area, corners (relative) | 0 | 0 |
| Cylinder | batch1 | 205 | eps_num (mm) | 5.6e−7 | 4.0e−13 |
| Cone | batch1 | 225 | L1, L2 (relative) | 2.9e−16 | 0 |
| Cone | batch1 | 225 | sector angle Φ (rad) | 8.9e−16 | 0 |
| Cone | batch1 | 225 | slant s (relative) | 2.1e−16 | 0 |
| Cone | batch1 | 225 | area (relative) | 6.9e−14 | 1.6e−16 |
| Cone | batch1 | 225 | display radius error (mm) | 3.0e−8 | 5.7e−14 |
| Cone | batch1 | 225 | display chord, true (mm) | 2.4e4 | 9.0e−4 |
| Cone | batch1 | 4 DXF files | ARC radius and angles (mm, rad) | 1.4e−17 | — |
| Eccentric | batch1c | 200 | developed angle Ψ (relative) | 7.5e−15 | 1.4e−15 |
| Eccentric | batch1c | 200 | area (relative) | 4.7e−8 | 6.4e−16 |
| Eccentric | batch1c | 200 | large edge vertices vs exact (mm) | 4.7e−10 | 1.0e−12 |
| Eccentric | batch1c | 200 | small edge vertices vs exact (mm) | 4.7e−10 | 3.7e−13 |
| Eccentric | batch1c | 200 | chord, 20 interior points (mm) | 9.95e−3 | 5.5e−4 |
| Eccentric | batch1c | 200 | chord, dense (mm) | 9.97e−3 | 5.5e−4 |
| Eccentric | batch1c | 200 | eps_num (mm) | 8.8e−8 | 2.1e−11 |
| Eccentric | batch1c | 15 × 6 | vertex error, 50 digits (mm) | 4.7e−10 | 8.0e−14 |
| Eccentric | batch1c | 200 | big edge developed length deficit (relative) | 1.3e−5 | 7.1e−7 |
| Eccentric | batch1c | 200 | seam (generator) length (relative) | 4.0e−15 | 1.7e−16 |
| Branch | batch2 | 200 | saddle vertex error (mm) | 9.1e−13 | — |
| Branch | batch2 | 200 | chord, 20 interior / dense (mm) | 8.08e−3 / 8.10e−3 | 8.3e−5 / 8.3e−5 |
| Branch | batch2 | 200 | developed length vs chord sum (relative) | 5.3e−6 | 1.8e−6 |
| Branch | batch2 | 200 | area (relative) | 3.3e−14 | 8.6e−16 |
| Branch | batch2 | 200 | eps_num (mm), 50-digit error max (12 vertices each) | 1.7e−11; 4.5e−12 | 7.3e−14; 7.1e−15 |
| Branch | batch2 | 200 | s_max, s_min quantity error (mm) | 2.6e−4 / 1.8e−4 | 5.9e−7 / 4.8e−7 |
| Branch | batch2 | 80 | full outline self-crossings; minimum height (mm) | 0; 6.5e−4 | — |
| Header | batch2h | 200 | hole vertex grid error (mm) | 1.9e−12 | 8.0e−15 |
| Header | batch2h | 200 | joint 3D closure, hole vs saddle (mm) | 1.9e−12 | 8.0e−15 |
| Header | batch2h | 200 | joint in pattern (mm) | 1.4e−12 | 3.6e−15 |
| Header | batch2h | 200 | opening area, perimeter (relative) | 2.1e−15 | 0 |
| Header | batch2h | 200 | plate gross area (relative) | 2.1e−16 | 0 |
| Header | batch2h | 200 | chord, 20 interior / dense (mm) | 7.7e−3 / 7.7e−3 | 4.7e−5 |
| Header | batch2h | 200 | eps_num (mm), 50-digit error max (12 vertices each) | 2.1e−11; 9.1e−13 | 8.9e−14; 5.0e−15 |

**Near tangency (branch, R = 100, r = 50, β = 60°, δ = R(1−g) − r).** Accepted for g = 1e−3 (n = 1440, dense chord 2.8e−3 mm), 1e−4 (n = 1440, 8.8e−3 mm), 1e−5 (n = 2880, 7.0e−3 mm), 1.01e−6 (n = 5760, 5.3e−3 mm). Refused for g = 9.9e−7, 1e−7, 1e−9 (rule |δ| + r < R(1−1e−6)).
**Knee (validation claim D3).** R = 150, r = R(1 − 1e−5), δ = 0, β = 45°: the code's density is n = 5760, its `eps_rep` 5.292e−3 mm and my refined true chord error 5.292e−3 mm. The 0.196 mm error claimed in COMPUTATIONAL_VALIDATION_REPORT.md does not reproduce in this build. The header hole with the same joint: 5.292e−3 mm. Other knee cases (β = 20°, 60°, 90°, r = R(1 − 1e−4 … 1e−3), offset near the limit) all agree within 2e−4 mm of `eps_rep`, and all are ≤ 0.01 mm.
**Convergence (synthetic, exact curve).** Saddle chord error at g = 1e−5: 7.0e−2 (n = 720), 2.6e−2 (1440), 7.0e−3 (2880), 1.7e−3 (5760), 4.4e−4 (11 520) mm: ratio 2.7, 3.7, 4.0, 4.0 per doubling, i.e. O(n⁻²) after the knee is resolved. The code chose 2880 for this case, which is the first doubling above 0.01 mm.
**Scaling.** Multiplying every length by a power of two reproduces the vertices bit for bit (cone, eccentric, branch, header, cylinder); other factors agree to 3e−16 relative. Vertex counts change at k = 512 because the chord limit is absolute (expected; RB-004).
**Reproducibility.** The same request submitted twice gave identical vertices, DXF SHA-256 and closure hash. Replay (`POST /api/calculations/{id}/replay`) returned `matches: true`; the replay runs in a separate process of the same build, which is not an independent build.
**Closure and orientation.** Every accepted curve chain closes (LINE and POLYLINE endpoints each used by exactly two curves in the DXF, 4 corners for the header plate). No NaN, no zero-length or duplicated segments among 40 stubs and 35 reducers (smallest segment 6.3e−5 mm and 6.7e−4 mm). Eccentric developed angle monotone in θ in all 200 accepted cases (sector simple by construction). Branch and header outlines: no proper self-crossing (80 stubs, 200 hole polylines).
**Joint.** The header hole, folded with the outward map of 1.5, coincides with the branch saddle in 3D to 1.9e−12 mm in all 200 random joints. The header and branch developed abscissae (Y = r_h·(ω+π) and x = r·φ) agree with the closed forms to 1.4e−12.

**Reference errors resolved (for transparency).** My first eccentric reference used `mp.quad` without breakpoints at the sharp peak of the integrand (near cos θ = r1/a) and reported 1e−2 mm vertex errors; corrected (breakpoints, 32 subintervals) it agrees with double precision to 1e−14. One accepted reducer (r1 = 100, r2 = 50, h = 150, e = 0) appeared to violate `eps_num` (2.1e−11 vs 1.0e−11 mm) in my uncompensated double reference; against the closed form at 50 digits its worst vertex error is 4.8e−14 mm, so there is no violation. Remaining double-precision checks cannot resolve errors below about 2e−11 mm, so the `eps_num` claim is verified at 50 digits only for the 15 + 12 + 12 sampled cases above.

---

## 4. Findings

### B2-001 — High: the radius rule accepts an unbounded implied wall thickness
**Where.** nf-core/src/job.rs:1199–1231 (`radius_consistency`); the `outside` arm at 1214–1217 refuses only `dev > contact`; 1217 `"outside" => Ok(None)` accepts any smaller positive development radius with no advisory. `inside` (1219–1222) is treated the same way. `unknown` gives no advisory either (RB-010 says the same).
**Reproduction 1 (branch).** `{"family":"branch_stub","header_radius":100,"branch_radius":50,"branch_dev_radius":0.5,"beta_deg":60,"delta":0,"s_end":200,"surface_reference":"outside"}` → HTTP 200, accepted. Measured developed width (max x of the saddle polyline) 3.14159 = 2π·0.5. Contact circumference 2π·50 = 314.159. Gates G05, G10, G11, G15 PASS, the rest INCONCLUSIVE, no refusal.
**Reproduction 2 (header).** `{"family":"header_with_branch","header_radius":100,"header_dev_radius":0.5,"header_length":400,"branch_station":150,"branch_radius":50,"branch_dev_radius":50,"beta_deg":60,"delta":10,"s_end":200,"surface_reference":"outside"}` → accepted; plate Y extent 3.14159 against 628.3185 for the contact circumference.
**Expected.** For an outside contact radius r_o and neutral radius r_n = r_o − k·t with 0 < k ≤ 0.5 and inner radius r_i = r_o − t > 0 (Spec 02 §6.1 as quoted in the code): t < r_o, hence r_o/2 < r_n < r_o. Here r_n = 0.5 is impossible for any wall.
**Why it matters.** The developed pattern is then a ring 100× narrower than the part, with the cut developed on a radius the part cannot have. The engine's own check then reports "consistent". The job remains NOT RELEASED because gates are inconclusive, but the numbers are accepted.
**Fix.** For `outside`, refuse `dev ≤ contact/2` as well as `dev > contact`. For `inside`, the bound needs the outer radius or a declared thickness, since t is unbounded from r_i alone; require one or the other. Until then, report the implied thickness and refuse when it exceeds the contact radius.

### B2-002 — Medium: polylines are reported as exact with zero deviation
**Where.** nf-core/src/job.rs:323 `points_exact: !matches!(c.geom, Geom::Arc { .. })` (true for polylines) and 324–330 (`_ => 0.0` for polylines). The pattern's `eps_rep` for the same curve is not in the curve record.
**Reproduction.** `{"family":"eccentric_reducer","r1":100,"r2":50,"h":150,"e":20}`: pattern `eps_rep` = 1.61e−3 mm; `large_end` has `points_exact: true`, `points_max_deviation: 0.0`. In the random set the reported deviation is 0 while the measured chord error reaches 9.97e−3 mm (200 cases). The same applies to the branch saddle (`saddle_cut`, deviation 0) and the header hole (`branch_opening`, deviation 0).
**Expected.** `points_exact: false` and `points_max_deviation` = the measured chord error (or the pattern's `eps_rep`) for polylines; `points_exact: true` only for vertices of lines.
**Fix.** Set `points_exact` to false for every curve that is an approximation, and carry the curve's `eps_rep` into `points_max_deviation`.

### B2-003 — Low (display only): arc display chords grow with radius
**Where.** nf-core/src/job.rs:297 (`ARC_DISPLAY_CHORDS = 96`), 319 (display points), 324–330 (sagitta formula, which is correct).
**Reproduction.** `{"family":"cone_frustum","r1":1e8,"r2":5e7,"h":1e8}`: display `points_max_deviation` = 23 946 mm (equal to my chord measurement). The DXF arc is exact: radius 2.236e8 mm, sweep Φ (4 DXF files checked, error ≤ 1.4e−17).
**Expected.** A display that is not used for manufacture may still not claim to be a curve: use a chord count fixed by a display tolerance, or omit the points and send the arc parameters.
**Fix.** Choose the display chord count from the sagitta limit: r(1 − cos(Φ/2n)) ≤ 0.01 mm gives n ≥ (Φ/2)·√(r/0.02); or drop the points for arcs.

### B2-004 — Low (label): header `area` is gross
**Where.** develop.rs:197 (`("area", w * length)` inherited by the header through `cylinder_shell`); develop.rs:891 (`opening_area` pushed separately).
**Reproduction.** Header with R = 100, branch r = 50, β = 60°, δ = 10 mm, s_end = 200 mm, header length 215.6 mm (station chosen so the opening lies inside): `area` = 135 481.357 = 2π·100·215.6; `opening_area` = 9 446.352 (shoelace of the hole, exact to 2e−15); net plate 126 035.005. The reported `area` is 7.5 % above the net.
**Fix.** Rename to `gross_area` and add `net_area = gross_area − opening_area`, or report net only.

### B2-005 — Low: refusals stricter than the mathematics requires
Each item was reproduced; the cited arithmetic shows the formula is accurate there.
- **Cone, sin α < 1e−3** (develop.rs:226–228): closed forms evaluated in binary64 agree with 50-digit values exactly at sin α = 1e−4 (relative error 0) and 1e−5 (1e−20 absolute on Φ). Conditioning in the inputs grows as 1/sin α, but the input rounding stays below 1e−12 relative at 1e−4. The 69 random refusals in this band are cones whose closed forms are accurate in binary64.
- **Cone, Φ ≥ 0.999·2π** (develop.rs:233–236): `{"family":"cone_frustum","r1":100,"r2":50,"h":1}` refused ("developed angle 6.281929 rad would overlap"). Sin α = 0.99980 gives a seam gap of 0.126 mm at the large end and 0.063 mm at the small end, not an overlap. The comment at develop.rs:231–232 ("seam edges touch") is wrong below 2π. 22 random refusals in this band.
- **Eccentric, Ψ ∈ [0.999·2π, 2π)** (develop.rs:392–398): 20 random refusals with Ψ between 6.2802 and 6.2832 rad, where the polar sector cannot overlap. No case with Ψ ≥ 2π was found in the random set (Ψ decreases with e for the tested family: 1.9869 at e = 0, 1.5735 at e = 100, maximum at e = 0), so the overlap branch itself is untested in the overlapping regime.
- **Branch angle** (develop.rs:619): sin β > 0.17 refuses β < 9.79° and β > 170.21°. The closed form s(φ) at β = 5° has binary64 error 4.6e−13 mm on s ≈ 980 mm (50-digit check).
- **Chord-error cap** (develop.rs:94–101): `{"family":"eccentric_reducer","r1":1e8,"r2":5e7,"h":1e8,"e":1e7}` refused ("cannot be represented within 0.01 mm with 46 080 segments"). The required segment count is about 2π·√(R/(8·0.01)); the cap refuses developed radii above about 4 km, while the magnitude check (job.rs:1237) advertises up to 1e8 mm. Branch and header have the same cap.
**Fix.** Use the closed-form conditioning bound instead of a flat limit; state the magnitude that the chord cap actually allows; correct the comment at develop.rs:231–232.

### B2-006 — Low: two quantities are less accurate than the rest
- Eccentric `area`: relative error up to 4.7e−8 (case r1 = 13.352847, r2 = 6.1638566, h = 0.0136016, e = 16.244922) from a fixed 720-panel rule (develop.rs:419–422). The integrand is peaked for near-flat cones.
- Branch `s_max`, `s_min`: sampled extremes (2 880 samples), error up to 2.6e−4 mm and 1.8e−4 mm (80–200 random stubs). Not used in the cut geometry.
**Fix.** Use the adaptive rule already in the file for `area` (develop.rs:349 `gauss_adaptive`); locate the extrema with a golden-section search.

### B2-007 — Low: `eps_rep` is an estimate, not a bound
**Where.** develop.rs:79–104 (`choose_density` returns the value measured at 15 interior points) and 58–73 (`chord_deviation`).
**Measured.** The 200-point maximum exceeds the 15-point value in 72 of 203 eccentric cases (by at most 2.3e−10 mm), 37 of 200 branch stubs (2.8e−14 mm), 27 of 200 header holes (4.7e−13 mm). The density rule itself is met in every case (dense maximum 9.97e−3 mm eccentric, 8.1e−3 branch, 7.7e−3 header).
**Fix.** Either refine the maximum inside each segment (a few golden-section steps) before returning, or describe `eps_rep` as an estimate in the JSON. The defect is in the word "bound", not in the size of the error.

---

## 5. Refusals: necessity

| Rule | Location | Verdict | Evidence |
|---|---|---|---|
| Magnitude 1e−6 … 1e8 mm | job.rs:1237–1238 | necessary as a statement of tested range; the chord cap in the eccentric and branch families is stricter (B2-005) | 3 refusals in cylinder batch (1e8+1, 1e−7) |
| Cone r1 > r2 | develop.rs:218 | necessary | — |
| Cone sin α < 1e−3 | develop.rs:226 | too strict (B2-005) | closed forms exact at 1e−4 |
| Cone Φ ≥ 0.999·2π | develop.rs:233 | too strict; the 0.1 % margin has no mathematical basis (B2-005) | cone (100, 50, 1) refused, seam gap 0.126 mm |
| Eccentric e ≥ 0; r1 > r2 | develop.rs:375, 381 | necessary as declared (mirror handled by sign convention) | — |
| Eccentric sin α < 1e−3 | develop.rs:384 | too strict (B2-005) | — |
| Eccentric Ψ ≥ 0.999·2π | develop.rs:394 | too strict below 2π; no overlap test (U7) | 20 refusals, Ψ ∈ [6.2802, 6.2832] |
| Branch sin β > 0.17 | develop.rs:619 | too strict (B2-005) | β = 5°: error 4.6e−13 mm |
| Branch \|δ\| + r < R(1 − 1e−6) | develop.rs:622–624 | necessary (partial intersection) with a tight margin; near tangency is resolved down to g = 1e−6 | g = 1.01e−6 accepted (n = 5760), 9.9e−7 refused |
| Branch s_end > peak of 2 881 samples | develop.rs:632–640 | correct in every test; sampling-based (U2) | exact threshold tested to 1e−9 relative |
| Header opening strictly inside plate | develop.rs:816–830 | necessary; the boundary refuses exact touching (harmless) | L = X_max refused, L = X_max + 1e−9 accepted |
| Chord error ≤ 0.01 mm at ≤ 46 080 segments | develop.rs:94–101 | too strict in magnitude (B2-005) | large reducer refused |
| Radius consistency (`outside`, `inside`) | job.rs:1212–1222 | too loose (B2-001) | dev = 0.5 mm accepted |

---

## 6. DXF

Checked for the branch stub, the header with branch (both parts), the eccentric reducer (two sizes), and the cone (four ARC files):
- Coordinates equal the JSON vertices exactly (maximum difference 0): R12 `POLYLINE` / `VERTEX` entities, shortest round-trip number format, so the printed precision is not a limit (17 significant digits).
- Closed contours: the header hole is a closed `CUT_INNER` polyline (720 vertices, closed flag set); the branch saddle is an open polyline that closes through its three LINE seams; each outline corner is used by exactly two curves.
- Layers: cut geometry only on `CUT_OUTER` and `CUT_INNER`; the marking-side and kerf notes are TEXT on `NF_NOT_RELEASED`. Nothing else on the cut layers.
- Arcs (cone): exact radius and sweep, error ≤ 1.4e−17.
- Kerf is not compensated, as declared in the file.

---

## 7. Unverified concerns (not reproduced; listed separately)

- **U1.** `profile::crossings` (profile.rs:68–85) counts proper crossings only (`o ≠ 0` required). Touching and collinear contacts are not detected. My own O(m²) crossing count found none in the accepted outputs, so the gap is latent.
- **U2.** The branch `s_end` threshold uses a uniform sampling of 2 881 points (develop.rs:632–640). A narrower peak could exceed the sampled maximum, letting `s_end` lie below the true saddle peak by O(Δφ²). Not found: the sampled and true peaks agreed to 0 in the tested configuration.
- **U3.** The density rule measures 15 interior points per segment (develop.rs:89). A knee narrower than a segment could be missed. The R = 150 knee was resolved (ratio 1.0), but that is not a proof for all knees.
- **U4.** Isometry was verified through lengths, areas, joint closure and closed forms, not by pointwise geodesic distances on the 3D surfaces. For the oblique cone a local metric check would be the next step.
- **U5.** Marking side (outside), seam placement and δ sign are internally consistent and declared; whether a fabrication shop marks and rolls on these conventions is not a numerical question and was not reviewed.
- **U6.** Thickness, bend allowance, kerf and weld gap are not modelled (stated limitation). The developed radius for the surface references depends on them, which is the content of B2-001.
- **U7.** The overlap refusal for Ψ ≥ 2π was never triggered in the tested space; its correctness is unverified.
- **U8.** Replay runs the same build in a separate process; it does not check the build.

---

## 8. Not tested, and why

- Pointwise geodesic distances on 3D surfaces (U4): time; the length, area and joint checks cover the same content for these charts.
- Interval arithmetic: not implemented in the build, so not tested.
- Cases beyond 2 880 segments for the eccentric family and 5 760 for the branch: the density rule was exercised up to these counts, and the convergence table extends it to 11 520 synthetically.
- Full 46 080-segment cases: none needed in the accepted range; the cap was exercised by the refusal at 1e8 mm.
- DXF of cylinder shells: rectangles only; checked by the JSON.
- Several builds or machines: only the frozen build (replay is same-build, U8).
- Header or branch with a development radius different from the contact radius and a concrete thickness: not modelled by the engine.
- Physical fabrication conventions (U5) and kerf (declared, not compensated).

---

## 9. Scores (repeated, with reasons)

- **Mathematical correctness, 8/10.** All five developments agree with independent derivations to 1e−12 mm in the closed-form parts; the joint closes in 3D; the density rule and the eps_num claims hold in the tested space. Not 9 or 10 because: the radius model accepts sizes that no wall can produce (B2-001); polylines are labelled exact with zero deviation (B2-002); the oblique cone's overlap test is untested in the overlapping regime (U7); and two conventions (marking side, δ sign) need a fabrication engineer.
- **Numerical robustness, 8/10.** Closed forms are accurate to 1e−13 mm, the oblique quadrature to 5e−10 mm, the scaling laws hold, and the results are reproducible and replayable. Not higher because `eps_rep` is an estimate (B2-007), two quantities are less accurate than the rest (B2-006), and the refusals are stricter than the arithmetic requires (B2-005).

---

## 10. Scripts, results, environment

Scripts (scratch directory `S/scripts`, all run against the frozen build):
- `nfapi.py` — API client (setup, login, calculations, results).
- `ref.py` — reference geometry: cone and eccentric (numpy and mpmath), branch saddle, Gauss panels, segment distance, crossing count.
- `batch1.py` — cylinder, cone and first eccentric run (eccentric part superseded by batch1c; the mpmath reference in it was corrected later). Results: `results/batch1.json`.
- `batch1b.py` — stopped (too slow at 256 breakpoints); superseded by batch1c.
- `batch1c.py` — eccentric random set (200), 50-digit checks, overlap threshold scan, refusal classification, DXF. Results: `results/batch1c.json`, `results/batch1c.out`.
- `batch2.py` — branch random set (200) and designed cases; its header section is superseded by batch2h. Results: `results/batch2.json`, `results/batch2.out`.
- `batch2h.py` — header with branch (200), joint, branch outline crossings. Results: `results/batch2h.json`, `results/batch2h.out`.
- `batch3.py` — near-tangent knee (branch and header), refined chord error. Output: `results/batch3.out`.
- `batch4.py`, `batch4b.py`, `dxfparse.py` — radius rule, DXF fidelity, reproducibility, replay, plate edges, shallow-cone refusals. Output: `results/batch4.out`.
- `batch5.py` — scaling laws. `batch6.py` — mismatched header radius and closed-form accuracy beyond refusal limits. `batch7.py` — zero-length segments, DXF chaining, convergence table.

Environment notes: the root filesystem was at 98–99 % during the first build attempt (`build.log` ends with "No space left on device"); the release build of the frozen copy (`cargo build --release`, exit 0) completed inside the full-disk window and the binary served every result above. Servers: `nf-server` from `S/src/target/release`, `NF_LISTEN=127.0.0.1:8840` and `:8841`, fresh data directories `S/data/8840` and `S/data/8841`, both removed after the run. I stopped only the processes I started (PIDs in `S/server8840.pid` and `S/server8841.pid`, and my batch jobs).
