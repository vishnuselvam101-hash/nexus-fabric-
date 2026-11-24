# 02 — Mathematical Engineering Specification

Universal Engineering Mathematics Engine (UEM)

Version 0.1 (draft for independent engineering review) · 18 November 2026
Parents: [Master Plan](../MASTER_PLAN.md) · [Spec 01](01_SYSTEM_ARCHITECTURE_SPECIFICATION.md) · [ADR-004](../architecture/ADR-004-mathematical-calculation-architecture.md)
Companions: [Mathematical Reference Cases](../benchmark/MATHEMATICAL_REFERENCE_CASES.md) · [Mathematical Research Register](../research/MATHEMATICAL_RESEARCH_REGISTER.md)

**Status of this document.** Every formula here is a *specification to be reviewed*, not a validated capability. Derivations were checked by the author and, where stated, numerically cross-checked (reference cases); none has yet been reviewed by a second qualified person, which the acceptance gate requires. Standards and textbooks are cited from the author's knowledge and are listed in the research register as **not yet consulted**; nothing here claims compliance with any standard. Factory-dependent values are open requirements (`FR-`), never assumed.

Requirement IDs `MATH-nn` extend `ARCH-nn` of Spec 01. Algorithm IDs `ALG-nn` are defined in §11.

---

## 0. Governing definitions

### 0.1 Four quantities that must never be conflated

| Symbol | Name | Meaning | Source |
|---|---|---|---|
| $\varepsilon_{num}$ | Numerical error bound | Guaranteed or estimated bound on the difference between the computed value and the exact value of the *mathematical model* | Interval arithmetic, a-priori analysis |
| $\varepsilon_{mod}$ | Model / approximation error | Difference between the mathematical model and the intended ideal geometry (distortion of an approximate development, faceting, curve-to-polyline representation) | Computed distortion metrics |
| $u$, $U$ | Measurement / parameter uncertainty (standard, expanded) | Dispersion of values attributable to imperfectly known inputs | Propagation (§8) |
| $T$ | Tolerance | Permitted deviation stated by drawing, customer or standard — a *requirement*, not a property of the calculation | Drawing / contract |

Contractual acceptance limits may differ from $T$ (guard-banding, §8.6). A *discrete interpretation ambiguity* (was that digit a 3 or an 8; is ⌀ inside or outside) is none of the above and is never converted into a variance (MATH-31).

### 0.2 Development classification (four classes)

Every surface region submitted for development receives exactly one class, recorded in the truth graph:

| Class | Name | Definition | Release consequence |
|---|---|---|---|
| **E** | Exactly developable | An isometry to a planar region exists and is established *analytically* (by construction of the surface type), and global validity conditions §3.4 hold. Sub-class **E-C**: closed-form evaluation. **E-N**: exact map evaluated numerically with bound $\varepsilon_{num}$ | Normal gates |
| **A** | Developable under defined assumptions | Geometry of class E applied to a reference surface whose position or length depends on stated manufacturing assumptions (neutral-surface factor, bend model, contact-surface choice, weld gap) | Each assumption must be backed by calibrated ProcessData or an engineer Decision; otherwise REVIEW REQUIRED (ARCH-11) |
| **X** | Approximately developable | No exact isometry exists (or the exact surface is replaced by an approximant, e.g. facets); a flat pattern is produced with quantified distortion $\varepsilon_{mod}$ | Distortion must be within budget **and** explicitly accepted by an engineer Decision; never labelled exact (ARCH-10) |
| **N** | Non-developable — segmentation or forming required | Distortion of any single flat pattern exceeds budget; requires division into pieces (each then E/A/X) or a forming process (pressing, spinning, dishing) whose blank shape depends on empirical process behaviour | UEM proposes segmentation or returns *out-of-scope: forming*; no pattern released without a process-specific, calibrated module |

Orthogonal to the class, every output curve carries a **representation error** $\varepsilon_{rep}$ (part of $\varepsilon_{mod}$): the exact developed curve of a class-E surface is generally transcendental (e.g. a sinusoid) and cannot be represented exactly by DXF lines and arcs.

- **MATH-01** Class is assigned by the surface-type constructor or by ALG-05, never by automated drawing reading and never by a sampled curvature test alone.
- **MATH-02** A result's class can only be weakened downstream (E→A→X), never strengthened.

### 0.3 Conventions

| Item | Convention |
|---|---|
| Internal units | Length: millimetre. Angle: radian. No other internal units. Conversions at the boundary only; inch = 25.4 mm exactly |
| Written values | A dimension read from a drawing is stored as an exact decimal rational (e.g. "1250.5" → 12505/10) with its unit and tolerance, converted to floating point only inside a calculation (MATH-03) |
| Frames | Right-handed Cartesian. World frame per job; local frames per view, part, surface, pattern |
| Vectors | Column vectors. Active transformations: $\mathbf p' = R\mathbf p + \mathbf t$ |
| Frame notation | ${}^{A}T_{B}$ maps coordinates expressed in frame B to frame A. Composition: ${}^{A}T_{C} = {}^{A}T_{B}\,{}^{B}T_{C}$ |
| Rotations | Stored as unit quaternion or orthonormal matrix, never as Euler angles; angle sign by right-hand rule |
| Pattern (2D) frame | $x$ right, $y$ up, counter-clockwise positive; outer profiles CCW, inner profiles CW. Pattern normal $+z$ toward the declared **marking side** (inside or outside of the formed part) — an explicit attribute, because mirroring a pattern is a classic fabrication error (MATH-04) |
| Angular datum | Each shape module declares where $\theta = 0$ lies on the part (seam datum) and its direction of increase as seen from a declared end |

---

## 1. UEM architecture

### 1.1 Subsystems and relation to Spec 01

Spec 01 §3.1 named nine components. This specification refines them into twelve subsystems. This is a refinement, not a change of decision; see §14 for the reconciliation, including the placement of verification.

| # | Subsystem | Crate (proposed) | Spec 01 component |
|---|---|---|---|
| 1 | Coordinate & transformation | `uem-frame` | new (was implicit) |
| 2 | Geometric primitives | `uem-primitive` | new (was implicit) |
| 3 | Constraint solving | `uem-constraint` | same |
| 4 | Parametric geometry (shape modules) | `uem-param` | new |
| 5 | Differential geometry | `uem-diffgeo` | same |
| 6 | Surface development | `uem-develop` | split from `uem-diffgeo` |
| 7 | Computational geometry | `uem-compgeo` + `uem-exact` | same |
| 8 | Numerical computation | `uem-numeric` + `uem-interval` | same |
| 9 | Uncertainty & tolerance | `uem-uncertainty` | same |
| 10 | Manufacturing mathematics | `uem-process` | same |
| 11 | Optimisation | `uem-optim` | new |
| 12 | Independent verification | `uem-verify` (**Z3-owned, separate**) | Spec 01 §3.4 |

Dependency direction (no cycles): 8 → 1,2 → 5 → 6; 7 uses 8; 3 uses 8, 2; 4 uses 2,3,5,6,10; 9 wraps any pure function; 11 uses 4,6,7,10; 12 depends only on `uem-exact`, `uem-interval`, units and its own implementations (ARCH-03).

### 1.2 The result envelope

Every public UEM function returns

```
Outcome<T> = Ok(Quantity<T>) | EngineeringOutcome
Quantity<T> { value: T, unit, regime: Exact|Interval|Float|MultiPrecision,
              eps_num, eps_mod, uncertainty: Option<Cov>, class: Option<E|A|X|N>,
              assumptions: [AssumptionId], inputs: [RecordHash], alg: (id, version) }
EngineeringOutcome = OutOfRange{model, param, range} | Ambiguous{solutions} | UnderDetermined{dof, free}
                   | Contradictory{minimal_conflict_set} | Degenerate{what} | NotDevelopable{metrics}
                   | Undecidable{predicate, precision} | NoConvergence{diagnostics} | Unsupported{reason}
```

- **MATH-05** No function returns a bare number for a physical quantity (ARCH-06/07).
- **MATH-06** `EngineeringOutcome` values are results, not exceptions; they propagate and force a non-APPROVED state (§10).

### 1.3 The automated-reading boundary

The UEM has no input type that an interpretation service can populate directly with geometry. Its only entry points take (a) accepted `Observation` records with provenance class, (b) `Decision` records, (c) `ProcessData` records. Model confidence scores are not a field of any UEM input type and so cannot influence a calculation or a gate (MATH-07). An interpretation service may *propose* which shape module applies; the proposal becomes an Observation of class *inferred* that must be accepted, and the module's own constraints must then be satisfied by the dimensional evidence.

### 1.4 Subsystem specifications

Format: **P**urpose · **F**oundations · **I**nputs · **O**utputs · **A**lgorithms · **Pr**ecision · **E**rror conditions · **V**erification · **D**ependencies/interfaces.

**1. Coordinate & transformation (`uem-frame`)**
P: frames, rigid/similarity/affine transforms, view projections. F: linear algebra, SO(3), homogeneous coordinates (§2.1). I: frame definitions, transform parameters, view declarations. O: typed transforms `Transform<From,To>`; projected coordinates. A: composition, inversion (closed form for rigid: $R^{\mathsf T}, -R^{\mathsf T}\mathbf t$), re-orthonormalisation by polar decomposition, quaternion conversion. Pr: Float with bound; orthonormality defect $\lVert R^{\mathsf T}R - I\rVert$ monitored. E: non-orthonormal input beyond threshold, singular affine map, mismatched frames (compile-time where possible). V: round-trip $T T^{-1} = I$; invariants (distances, angles, handedness $\det R = +1$); independent implementation via quaternion path versus matrix path. D: 8.

**2. Geometric primitives (`uem-primitive`)**
P: canonical representations of curves and surfaces (§2.2). F: analytic geometry. I: defining parameters. O: evaluation, derivatives to 2nd order, implicit residual, bounding boxes, exact/approximate flag. A: evaluation, closest point, conversion to/from kernel-neutral form. Pr: per primitive (§2.2). E: degenerate parameters (zero radius, zero normal, coincident defining points). V: implicit-versus-parametric consistency $F(S(u,v)) = 0$ within bound; derivative checks against interval finite differences. D: 1, 8; consumed by all.

**3. Constraint solving (`uem-constraint`)**
P: reconstruct parameters from dimensions and relations; diagnose under/over-determination and ambiguity (§2.3). F: nonlinear equations, rank analysis, constrained least squares. I: variables, hard constraints (class H), soft observations (class S) with covariance, initial guesses or bounds. O: solution set(s), DOF report, residuals, conditioning, conflict sets. A: ALG-01, ALG-02. Pr: Float solve; hard-constraint satisfaction certified in Interval (existence/uniqueness test) where the system is square. E: `UnderDetermined`, `Contradictory`, `Ambiguous`, `NoConvergence`, ill-conditioning beyond threshold. V: residual re-evaluation by independent code; multi-start reproduction; interval certification; metamorphic tests (rigid motion of all inputs). D: 2, 8, 9.

**4. Parametric geometry (`uem-param`)**
P: shape modules implementing the shape-module contract (master plan §8) for the families in §4. F: per family. I: module parameters with provenance. O: surfaces (as primitives), feature curves, development request, applicability verdict. A: per-family procedures §4. Pr: per family. E: `OutOfRange` (applicability), `Degenerate`. V: per-family independent method (§9.2) and reference cases. D: 2, 3, 5, 6, 10.

**5. Differential geometry (`uem-diffgeo`)**
P: metric and curvature analysis; developability classification (§3). F: classical surface theory. I: parametric surface with derivatives; region. O: fundamental forms, $K$, $H$, principal curvatures, geodesic curvature of curves on surfaces, classification evidence. A: ALG-05. Pr: Interval for classification decisions; Float for reporting. E: singular parametrisation ($EG - F^2 \to 0$), non-differentiable input. V: closed-form checks on quadrics; Gauss–Bonnet consistency on test regions. D: 2, 8.

**6. Surface development (`uem-develop`)**
P: construct the planar image of a surface region and its feature curves (§3, §4). F: isometric maps of developable surfaces; distortion-minimising maps for class X. I: classified surface region, cut (seam) specification, reference-surface assumptions. O: pattern boundary curves with $\varepsilon_{num}$, $\varepsilon_{rep}$, class, mapping function surface↔plane, distortion field (class X). A: ALG-03, ALG-04, ALG-06, ALG-07, ALG-09. Pr: Float + interval bound on every boundary sample; quadrature tolerance derived from tolerance budget. E: `NotDevelopable`, overlap (non-injective development), seam not making region disk-like, apex/edge-of-regression inside region. V: §9 — fold-back, invariants (lengths, area, angles), independent formulation. D: 2, 5, 7, 8, 10.

**7. Computational geometry (`uem-compgeo`, `uem-exact`)**
P: 2D profile operations and topological decisions on final cut geometry. F: exact predicates, arrangements, offsets. I: curves (lines, arcs, polylines, splines with bound). O: closed-profile verdicts, self-intersection reports, offsets, containment, area/length. A: ALG-08, ALG-10. Pr: predicates exact (adaptive); constructions Float with tracked error, snapped only by explicit, recorded rule. E: `Undecidable` for curved predicates beyond available precision, open profile, zero-length element. V: independent implementation in `uem-verify` plus a second library (benchmark-selected). D: 8.

**8. Numerical computation (`uem-numeric`, `uem-interval`)**
P: root finding, quadrature, ODE integration, linear algebra, special functions, interval and multi-precision arithmetic (§7). F: numerical analysis, IEEE 754 arithmetic, interval analysis. I: functions (with derivative where available), tolerances. O: values with error bounds, convergence diagnostics. A: bracketed root finding (interval Newton + bisection fallback), adaptive Gauss–Kronrod with interval-verified remainder where required, QR/SVD, compensated summation. Pr: §7.2. E: `NoConvergence`, bound wider than requested (triggers precision escalation). V: comparison against multi-precision evaluation; known integrals; condition-number-stressed cases. D: none inside UEM.

**9. Uncertainty & tolerance (`uem-uncertainty`)**
P: propagate uncertainty and tolerances through any pure UEM function; sensitivity; conformity decision inputs (§8). F: GUM framework (to be verified against the documents), Monte Carlo, interval analysis. I: function, inputs with covariance / distributions / tolerance intervals. O: $u$, $U$, coverage intervals, worst-case intervals, sensitivity ranking, linearity diagnostic. A: ALG-11. Pr: derivatives by automatic differentiation; seeded deterministic RNG. E: non-linearity diagnostic exceeded (first-order invalid), missing uncertainty for an input (not silently zero — MATH-30). V: first-order vs Monte Carlo agreement on linear cases; analytic propagation cases. D: 8.

**10. Manufacturing mathematics (`uem-process`)**
P: reference-surface, bend, roll, kerf, weld-preparation and compensation models (§6). F: plasticity-based bend models (theoretical) plus empirical calibration. I: geometry, material, thickness, ProcessData. O: adjusted lengths/positions flagged with assumptions and validity. A: §6 models. Pr: Float; dominated by parameter uncertainty, which must be propagated. E: `OutOfRange`, missing calibration (returns value flagged *assumed* → REVIEW REQUIRED). V: closed-form checks; physical calibration evidence (benchmark tier P) — mathematics alone cannot verify these models. D: 2, 9.

**11. Optimisation (`uem-optim`)**
P: plate division and seam placement; fitting (§2.4, §5). F: constrained and multi-objective optimisation. I: developed patterns as functions of seam variables, stock, rules, cost model. O: feasible strategy set, Pareto front, selected strategy with explanation. A: ALG-12. Pr: feasibility decisions use exact/interval comparisons against limits. E: no feasible strategy (with the violated constraints), time budget exceeded (returns best-so-far labelled incomplete). V: independent feasibility re-check of every reported strategy; brute-force comparison on small instances. D: 4, 6, 7, 10.

**12. Independent verification (`uem-verify`, Z3)**
P: challenge primary results with independent methods (§9). F: per check. I: primary result records + original inputs (not intermediate primary state). O: Evidence records (pass / fail / inconclusive, margin, independence class). A: ALG-13 and per-family verifiers. Pr: Interval for comparisons. E: verifier inapplicable → *inconclusive*. V: seeded-fault tests (each verifier must detect the fault class it exists for). D: `uem-exact`, `uem-interval`, units only; optionally an external kernel through the kernel port.

---

## 2. Mathematical foundations

### 2.1 Coordinate systems and transformations

Rigid transform: $\mathbf p' = R\mathbf p + \mathbf t$, $R^{\mathsf T}R = I$, $\det R = +1$. Homogeneous form

$$T = \begin{bmatrix} R & \mathbf t \\ \mathbf 0^{\mathsf T} & 1 \end{bmatrix},\qquad T^{-1} = \begin{bmatrix} R^{\mathsf T} & -R^{\mathsf T}\mathbf t \\ \mathbf 0^{\mathsf T} & 1 \end{bmatrix}.$$

Similarity (drawing scale $s>0$): $\mathbf p' = sR\mathbf p + \mathbf t$. Reflections ($\det = -1$) are a distinct type and are never produced implicitly (MATH-04).

**Orthographic view.** A view $V$ with unit axes $\mathbf a_V, \mathbf b_V$ (drawing right/up, in world coordinates), origin $\mathbf o_V$ and scale $s_V$ maps a world point to sheet coordinates

$$\begin{bmatrix} x_V \\ y_V\end{bmatrix} = s_V \begin{bmatrix}\mathbf a_V^{\mathsf T} \\ \mathbf b_V^{\mathsf T}\end{bmatrix}(\mathbf p - \mathbf o_V) + \mathbf c_V .$$

The viewing direction is $\mathbf a_V \times \mathbf b_V$ (toward the viewer). Two views of one object share the coordinate along their common axis; the arrangement of views on the sheet follows first-angle or third-angle projection, which reverses the placement — this is an explicit job attribute and is never guessed (`FR-03-4`). Section and auxiliary views carry their cutting plane / direction as parameters. Correspondence between views yields *constraints* (§2.3), not direct coordinates.

**Numerical stability.** Compose rotations as quaternions and renormalise; re-orthonormalise matrices via polar decomposition when the defect exceeds a set multiple of unit round-off; avoid subtracting large translations (work in a job-local frame centred on the part so that coordinates are $O(10^{3\text{–}4})$ mm, keeping absolute f64 spacing near $10^{-12}$ mm); never invert a rigid transform with a general matrix inverse.

### 2.2 Geometric primitives

| Primitive | Representation | Exactness notes |
|---|---|---|
| Point | $\mathbf p \in \mathbb R^{2|3}$; rational coordinates where derived only by rational operations from written values | Exact if rational |
| Line | point + unit direction, or implicit $\mathbf n\cdot\mathbf p = d$ | Exact in rational form (direction unnormalised) |
| Segment | two points | Exact if endpoints exact |
| Circle / arc (2D) | centre, radius, start/end angle, orientation | Radius/centre may be exact; points on it are generally irrational — arcs are an *exact curve type* with inexact point coordinates |
| Ellipse | centre, axes, semi-axes | Arc length is an elliptic integral — no elementary closed form (E-N) |
| Spline / NURBS curve | degree, knots, control points, weights | Exact representation of conics when rational; when used to *fit* another curve it is an approximation carrying $\varepsilon_{rep}$ |
| Plane | $\mathbf n\cdot\mathbf p = d$ | Exact in rational form |
| Cylinder | axis frame, radius $r$: $S(\theta,z) = (r\cos\theta, r\sin\theta, z)$; implicit $x^2 + y^2 = r^2$ | Quadric; class E-C |
| Cone (right circular) | apex, axis, half-angle $\alpha$; implicit $x^2+y^2 = z^2\tan^2\alpha$ | Quadric; E-C; apex singular |
| General cone | apex $A$, directrix curve $C(\theta)$ | E-N |
| Sphere | centre, radius | $K = 1/r^2 > 0$: class X or N only |
| Ruled surface | $S(t,\lambda) = C(t) + \lambda\,D(t)$ | Developable iff $\det[C', D, D'] = 0$ (§3.3) |
| Parametric surface | $S(u,v)$, $C^2$ on the region | Classified by ALG-05 |
| NURBS surface | as exchanged with a kernel | Treated as *data*: class determined by analysis, with kernel tolerance recorded; never assumed developable because it "looks like" a cone |
| Quadric (implicit) | $F(\mathbf p) = \mathbf p^{\mathsf T}Q\mathbf p + 2\mathbf q^{\mathsf T}\mathbf p + c = 0$ | Used for intersection (ALG-06) and for independent residual checks |

- **MATH-08** A primitive recognised from kernel or drawing data as "cylinder/cone/plane" is accepted as that analytic type only if its defining data fit within a recorded tolerance, and the fit residual is added to $\varepsilon_{mod}$.

### 2.3 Constraint solving

**Variables** $\mathbf x \in \mathbb R^n$: parameters of primitives and shape modules.

**Two classes of information — the central rule.**

- **Class H (authoritative).** Written dimensions, declared geometric relations accepted by an engineer, module-intrinsic relations. Encoded as equations $g_j(\mathbf x) = 0$. A written dimension with tolerance, e.g. $500 \pm 1$, is the equation $d(\mathbf x) = 500$ for nominal geometry; the tolerance is carried separately as a requirement on the manufactured part (it is not noise on the nominal).
- **Class S (evidential).** Quantities *measured* from linework or images, with standard uncertainty $\sigma_i$. Encoded as residuals $r_i(\mathbf x) = (f_i(\mathbf x) - y_i)/\sigma_i$.

$$\min_{\mathbf x}\ \tfrac12\sum_i \rho\big(r_i(\mathbf x)\big)\quad\text{subject to}\quad g(\mathbf x) = 0 .$$

- **MATH-09** Optimisation never alters a class-H value. If class-H constraints are mutually inconsistent the outcome is `Contradictory` with a minimal conflicting subset; no best-fit compromise is produced.
- **MATH-10** Class-S data may *test* class-H data: if $|f_i(\mathbf x^*) - y_i| > k\,\sigma_i$ at the solution, a discrepancy record is raised (the drawing may be not-to-scale, or a dimension misread). The solution is not adjusted toward the measurement.
- **MATH-11** Robust losses $\rho$ (Huber, Tukey) apply to class S only and are used to *identify* outliers, which are reported, not silently discarded.

**Constraint catalogue** (each with residual form chosen to be well-scaled and non-degenerate):

| Constraint | Residual form (2D unless noted) |
|---|---|
| Distance point–point | $\lVert \mathbf p_1 - \mathbf p_2\rVert^2 - d^2$ (or unsquared when $d \gg 0$) |
| Distance point–line | $\mathbf n\cdot\mathbf p - c - d$ |
| Angle between lines | $\operatorname{atan2}(\mathbf d_1\times\mathbf d_2,\ \mathbf d_1\cdot\mathbf d_2) - \phi$ |
| Coincidence | $\mathbf p_1 - \mathbf p_2$ |
| Parallelism | $\mathbf d_1 \times \mathbf d_2$ |
| Perpendicularity | $\mathbf d_1\cdot\mathbf d_2$ |
| Tangency line–circle | $(\mathbf n\cdot\mathbf c - c_0)^2 - r^2$, branch recorded |
| Tangency circle–circle | $\lVert\mathbf c_1-\mathbf c_2\rVert^2 - (r_1 \pm r_2)^2$, internal/external branch recorded |
| Concentricity | $\mathbf c_1 - \mathbf c_2$ (3D: coaxiality = parallel axes + point on axis) |
| Symmetry about a line | midpoint on line and connecting vector normal to line |
| Equal dimension | $d_a(\mathbf x) - d_b(\mathbf x)$ |
| Fixed | $x_k - \text{const}$ |
| Radius / diameter | $r - R$; inside/outside/mean reference is an attribute (`FR-02-2`) |
| View correspondence (3D) | shared coordinate of the same feature in two views equal after the view maps of §2.1 |

**Degrees of freedom and diagnosis** (ALG-01). With $J_g = \partial g/\partial\mathbf x$ evaluated at a solution, column-scaled:

- $\operatorname{rank}(J_g) = n$ after removing the declared datum (rigid-body gauge): *well-determined by authoritative data*.
- $\operatorname{rank}(J_g) < n$: **under-determined**; the null space identifies which parameters are free. If class-S data fix them, the affected parameters are tagged *measured*, not *written* (provenance, ARCH-14) and inherit measurement uncertainty; otherwise `UnderDetermined`.
- More equations than rank: **redundant**. Consistent redundancy (all residuals within the rounding of the written values) is valuable confirmation and is recorded; inconsistent redundancy is `Contradictory`.
- Rank decisions use the singular-value spectrum with an explicit gap criterion; a spectrum without a clear gap yields "rank uncertain" and forces review, rather than an arbitrary threshold decision. Local rank at one solution can mislead at singular configurations; the test is repeated at perturbed solutions.

**Ambiguity.** Nonlinear systems can have several isolated solutions (mirror images, tangency branches, arc-major/minor, feature on near/far side in a view). ALG-02 enumerates solutions by branch enumeration over discrete choices plus multi-start within bounds, clusters them, and returns:

- one solution, certified locally unique → proceed;
- several distinct solutions → `Ambiguous` with each candidate and the constraints that would discriminate them. The engine never selects one by solver start point (MATH-12). Discriminating evidence may come from class-S data or a Decision.

Multi-start cannot prove that all solutions were found; the residual risk is stated, and for the low-dimensional systems of shape modules a complete method (interval branch-and-bound over the parameter box) is specified instead.

### 2.4 Optimisation and fitting

| Use | Method | Notes |
|---|---|---|
| Soft least squares | Gauss–Newton / Levenberg–Marquardt on QR, never normal equations | Report $\chi^2$, residuals, covariance $(J^{\mathsf T}J)^{-1}$ only when rank is full |
| With hard constraints | Null-space elimination when $g$ is linear; SQP or augmented Lagrangian otherwise; final hard residual certified in interval | MATH-09 |
| Robust estimation | IRLS with Huber/Tukey; RANSAC-type consensus for primitive detection | Outliers reported |
| Geometric fitting (circle, line, arc, ellipse to measured points) | Orthogonal-distance regression (geometric, not algebraic, residuals) | Returns parameter covariance; fit quality gate before a fitted primitive is accepted (MATH-08) |
| Parameter estimation / calibration (K-factor, kerf from trials) | Weighted least squares or Bayesian update with explicit prior; produces ProcessData with uncertainty and validity range | §6 |
| Discrete / combinatorial | §5 | |

A converged optimiser is evidence only that a local stationary point was found. Acceptance of a fit requires: residuals consistent with stated $\sigma$ (goodness-of-fit test), conditioning below threshold, and no competing solution of comparable quality (MATH-13).

---

## 3. Differential geometry and surface development

### 3.1 Surface theory used

For $S(u,v)$ of class $C^2$ with $S_u \times S_v \ne 0$ and unit normal $\mathbf n = (S_u\times S_v)/\lVert S_u\times S_v\rVert$:

$$\mathrm I = E\,du^2 + 2F\,du\,dv + G\,dv^2,\quad E = S_u\!\cdot\! S_u,\ F = S_u\!\cdot\! S_v,\ G = S_v\!\cdot\! S_v$$
$$\mathrm{II} = e\,du^2 + 2f\,du\,dv + g\,dv^2,\quad e = S_{uu}\!\cdot\!\mathbf n,\ f = S_{uv}\!\cdot\!\mathbf n,\ g = S_{vv}\!\cdot\!\mathbf n$$
$$K = \frac{eg - f^2}{EG - F^2},\qquad H = \frac{eG - 2fF + gE}{2(EG - F^2)},\qquad \kappa_{1,2} = H \pm \sqrt{H^2 - K}.$$

Lengths, angles and areas on the surface are determined by $\mathrm I$ alone: $ds^2 = \mathrm I$, $dA = \sqrt{EG-F^2}\,du\,dv$. A map between surfaces is an **isometry** iff it preserves $\mathrm I$. By Gauss's Theorema Egregium $K$ is invariant under isometry; the plane has $K = 0$; hence:

- $K \ne 0$ anywhere on a region ⇒ **no** distortion-free flattening of that region exists.
- $K \equiv 0$ on a region ⇒ every point has a neighbourhood isometric to a planar region (*local* developability).

The geodesic curvature of a curve $C(t)$ on the surface, $\kappa_g = \dfrac{(C'\times C'')\cdot\mathbf n}{\lVert C'\rVert^3}$, is also an isometric invariant; it is the planar curvature of the curve's image in the development and is the basis of ALG-04.

### 3.2 Why $K = 0$ is not sufficient for a valid pattern

$K \equiv 0$ is a local, differential statement. A *manufacturing pattern* is a global object. The following can each invalidate a pattern although $K = 0$ everywhere it is defined:

1. **Topology.** A closed cylinder or cone is not isometric to a planar region as a whole; a cut (seam) is required to make the region simply connected. Where the seam lies is an engineering decision that changes the pattern.
2. **Overlap.** The development of a region may be locally isometric but not injective: a general cone whose total developed angle exceeds $2\pi$, or a tangent developable followed for more than a turn, overlaps itself in the plane. Such a "pattern" cannot be cut from one sheet.
3. **Singularities.** Apex of a cone and the edge of regression of a tangent developable are not regular points; rulings converge there, material cannot follow, and numerical conditioning degrades in a neighbourhood.
4. **Numerical evidence is not proof.** A sampled $|K| < \tau$ shows smallness at the samples at the precision used; it neither proves $K \equiv 0$ nor bounds the distortion between samples.
5. **Physical thickness.** Real plate has thickness; only one surface through the thickness keeps its length in bending. The pattern must be developed for that reference surface (§6.1) — which is an assumption (class A), not geometry.
6. **Smoothness.** A surface that is piecewise developable with tangent-plane discontinuity (a crease) is still developable; one with a curved crease or with $C^0$ joins of non-matching rulings may not be producible by the intended process.
7. **Process.** A mathematically valid pattern may be unformable with available machines (minimum roll diameter, cone-rolling capability, press-brake length). That is a process constraint (§5, §6), not a property of $K$.

### 3.3 Developable surface types (structure theorem used)

A $C^2$ surface with $K \equiv 0$ and no planar points is locally a **ruled surface with constant tangent plane along each ruling**, and is one of: a plane region, a generalised cylinder (parallel rulings), a generalised cone (concurrent rulings), or a tangent developable (rulings tangent to a space curve). For a ruled surface $S(t,\lambda) = C(t) + \lambda D(t)$:

$$\text{developable} \iff \det\big[\,C'(t),\ D(t),\ D'(t)\,\big] = 0\quad\forall t .$$

A developable surface joining two given curves $C_1(s), C_2(t)$ has rulings connecting points where the tangent planes coincide:

$$\det\big[\,C_1'(s),\ C_2'(t),\ C_2(t) - C_1(s)\,\big] = 0 .\tag{3.1}$$

(Surfaces with planar points or mixed regions are handled piecewise; the general statement requires care and is listed in the research register, R-03.)

### 3.4 Conditions under which an exact flat pattern is declared possible (class E)

All of the following must hold, each producing a recorded piece of evidence:

1. **Type by construction.** The region is a plane, generalised cylinder, generalised cone or tangent developable *constructed as such* by a shape module, or a ruled surface for which $\det[C', D, D'] = 0$ is established symbolically or by interval evaluation over the whole parameter range (enclosure contains zero is *not* enough; the determinant must vanish identically by construction, or the surface is treated as class X with measured distortion).
2. **Regularity.** The region excludes singular points with a stated margin (apex distance, edge of regression).
3. **Cut makes it a disk.** With the declared seams the region is simply connected.
4. **Injectivity.** The developed boundary is a simple closed curve (exact 2D self-intersection test, ALG-08) and the development has no fold-over (the orientation of the mapping — sign of the Jacobian — is constant; for cones, total developed angle of any uncut portion $< 2\pi$).
5. **Bounds.** $\varepsilon_{num}$ and $\varepsilon_{rep}$ are computed and within budget.

If 1 fails → class X or N. If 2–4 fail → `NotDevelopable{reason}` with a proposal (move seam, split). Thickness and process assumptions then move the class from E to A (MATH-02).

### 3.5 Approximate development (class X): how distortion is measured, reported and approved

For a flattening map $\phi: S \to \mathbb R^2$, at each point let $\sigma_1 \ge \sigma_2 > 0$ be the singular values of $d\phi$ with respect to orthonormal bases (principal stretches).

| Metric | Definition | Meaning |
|---|---|---|
| Principal strains | $\epsilon_i = \sigma_i - 1$ | Required in-plane stretch/compression of the material |
| Max strain | $\max_S \max(|\epsilon_1|, |\epsilon_2|)$ | Primary acceptance metric |
| Area distortion | $\sigma_1\sigma_2 - 1$ | |
| Angle distortion | $\sigma_1/\sigma_2 - 1$ | |
| Boundary length error | $\ell_{flat}(\gamma)/\ell_{3D}(\gamma) - 1$ per boundary edge | Governs seam fit-up |
| Seam mismatch | Difference of developed lengths of the two edges that will be welded together, and of corresponding station positions along them | Directly manufacturable significance |
| Fold-back deviation | $\max_p d(\Psi(\phi(p)), S)$ where $\Psi$ is the formed-shape model | Only meaningful with a stated forming model |
| Curvature indicator | $\int_S |K|\,dA$ and $\max|K|\,\rho^2$ ($\rho$ = region radius) | A-priori screening indicator, **not a bound** (from $C(\rho) = 2\pi\rho\,(1 - K\rho^2/6 + \dots)$) |

Reporting: metric values with location of maxima, a distortion map, the approximation method and its parameters, and the statement "APPROXIMATE DEVELOPMENT" in every output that carries the pattern. Approval: metrics must be within the job's distortion budget (`FR-05-1`; no default is assumed) **and** an authorised engineer records a Decision accepting that method for that part; absent either, REVIEW REQUIRED or class N.

**Faceted realisation of a class-E surface.** When an exactly developable surface is to be formed by discrete press-brake bends rather than rolling, the *formed* part is polyhedral. The pattern may still be the exact development, but the formed shape deviates from the ideal by the facet sagitta, e.g. for a circular edge of radius $r$ with $N$ equal facets per full circle the radial deviation is $r\,(1 - \cos(\pi/N))$. This is reported as $\varepsilon_{mod}$ of the forming method, not as an error of the pattern.

---

## 4. Industrial fabrication mathematics

Common notation: $t$ thickness; $r_i$ inside radius; $r_{dev}$ development (reference-surface) radius from §6.1; $\theta$ azimuth about the part axis from the seam datum; pattern coordinates $(u,v)$. Every family lists: definition · parameters · equations · procedure · boundary conditions · limits · class · errors · independent verification · reference cases ([reference file](../benchmark/MATHEMATICAL_REFERENCE_CASES.md)).

### 4.1 Cylindrical pipe / shell

- **Definition.** $S(\theta,z) = (r\cos\theta,\ r\sin\theta,\ z)$, $\theta\in[0,2\pi)$, $z \in [z_0(\theta), z_1(\theta)]$.
- **Parameters.** Diameter (with inside/outside/mean attribute), $t$, length, end-cut definitions (planes or intersection with another surface), seam datum.
- **Equations.** Development $u = r_{dev}\,\theta$, $v = z$ (isometry of the reference surface). Developed width $W = 2\pi r_{dev} - g$ where $g$ is any weld-gap deduction (§6.4). End cut by plane $\mathbf n\cdot(\mathbf p - \mathbf p_0) = 0$, $n_z \ne 0$:

$$z(\theta) = z_0 - \frac{r_c\,(n_x\cos\theta + n_y\sin\theta)}{n_z},$$

a sinusoid in $u$, where $r_c$ is the *contact radius* (the surface of the wall that defines the cut — inside, outside or reference; an engineering decision linked to weld preparation, §6.5).
- **Mitred (segmented) bend.** Bend centreline radius $R_b$, total angle $\Theta$, $m$ mitre joints ⇒ half-angle $\gamma = \Theta/(2m)$; cut height measured from the segment's mid-plane, with $\theta = 0$ at the intrados: $v(\theta) = (R_b - r_c\cos\theta)\tan\gamma$.
- **Procedure.** Resolve reference radii → compute boundary functions → sample adaptively (ALG-07) → represent (ALG-09) → profile checks (ALG-08).
- **Boundary conditions.** Pattern edges at $u = 0$ and $u = W$ must have equal $v$ and equal slope (seam closure and tangent continuity for smooth cuts).
- **Limits.** $r_{dev}/t$ within rolling-model validity (§6.3); plane cuts require $n_z \ne 0$ and cut not crossing the opposite end.
- **Class.** E-C for the reference surface; **A** once $r_{dev}$, $r_c$, $g$ are applied.
- **Errors.** $\varepsilon_{num}$ ≈ few ulp of trig evaluation; dominant terms are $\varepsilon_{rep}$ of the sinusoid and uncertainty of $r_{dev}$ ($\partial W/\partial r_{dev} = 2\pi$: 0.1 mm in radius ⇒ 0.63 mm in width).
- **Independent verification.** (i) Fold-back: map pattern samples to 3D by $(\theta,z) = (u/r_{dev}, v)$ and evaluate the implicit plane/quadric residual of the cut; (ii) arc-length invariant: developed length of the cut curve equals the 3D curve length computed from the ellipse (plane ∩ cylinder) by an independent elliptic-integral evaluation; (iii) area of pattern equals $r_{dev}\!\int (z_1 - z_0)\,d\theta$ by independent quadrature.
- **Reference cases.** CYL-01, CYL-02, MITRE-01.

### 4.2 Concentric reducer (right circular cone frustum)

- **Parameters.** Reference radii $r_1 > r_2$ at the two end planes, axial height $h$, $t$, seam datum.
- **Equations.** Half-angle $\tan\alpha = (r_1 - r_2)/h$. Slant length $s = \sqrt{h^2 + (r_1 - r_2)^2}$. Apex distances $L_1 = r_1/\sin\alpha$, $L_2 = r_2/\sin\alpha$ ($L_1 - L_2 = s$). Development: annular sector with radii $L_2, L_1$ and angle

$$\Phi = 2\pi\sin\alpha = \frac{2\pi (r_1 - r_2)}{s};$$

surface point at apex distance $\rho$ and azimuth $\theta$ ↦ polar $(\rho,\ \theta\sin\alpha)$. Lateral area $= \pi (r_1 + r_2)\,s = \tfrac12\Phi (L_1^2 - L_2^2)$. Chord across the large arc $= 2L_1\sin(\Phi/2)$ (stock-size check).
- **Thickness.** With $t$ measured normal to the wall, the radial wall thickness is $t/\cos\alpha$; the reference radius at an end plane perpendicular to the axis is $r_{dev} = r_i + k\,t/\cos\alpha$ with neutral factor $k$ (§6.1).
- **Procedure.** Reference radii → $\alpha, L_1, L_2, \Phi$ → boundary arcs and seam lines (exact DXF arcs and lines: $\varepsilon_{rep} = 0$ for plain frusta) → optional multi-piece division by angle (§5).
- **Boundary conditions.** Seam edges are equal-length straight lines of length $s$; arc lengths equal $2\pi r_1$, $2\pi r_2$.
- **Limits.** Conditioning: as $r_1 \to r_2$, $L_1 \to \infty$ and apex-based formulas lose accuracy and produce impractically large radii. For $L_1$ above a set threshold relative to $s$ the module switches to the apex-free formulation (ALG-04: development from the geodesic curvature of the end circle, $\kappa_g = \sin\alpha / r = 1/L$) and outputs offsets rather than a centre; $r_1 = r_2$ is the cylinder. Cone-rolling capability and minimum small-end radius: `FR-04-1`.
- **Class.** E-C; A with thickness.
- **Errors.** Sensitivity of $\Phi$ to radii and height via ALG-11; near-cylindrical cases are sensitive in $L$ but not in the pattern shape — verification compares pattern points, not $L$.
- **Independent verification.** (i) ALG-04 (apex-free ODE integration) versus closed form; (ii) invariants: arc lengths, seam length, area; (iii) fold-back onto implicit cone $x^2 + y^2 = (z_a - z)^2\tan^2\alpha$; (iv) general-cone integral (4.3) with $e = 0$ must reproduce $\sin\alpha$.
- **Reference cases.** CONE-01, CONE-02 (near-cylindrical), CONE-03 (steep).

### 4.3 Eccentric reducer (oblique circular cone frustum)

- **Definition.** End circles in parallel planes: $C_1(\theta) = (r_1\cos\theta,\ r_1\sin\theta,\ 0)$ and $C_2(\theta) = (e + r_2\cos\theta,\ r_2\sin\theta,\ h)$, joined by straight rulings between equal $\theta$. For $r_1 \ne r_2$ the rulings meet at the apex

$$A = \Big(\frac{e\,r_1}{r_1 - r_2},\ 0,\ \frac{h\,r_1}{r_1 - r_2}\Big) \equiv (a, 0, H),$$

so the surface is a general cone, hence developable (the tangents $C_1'(\theta) \parallel C_2'(\theta)$ confirm a constant tangent plane along each ruling). The flat-sided reducer common in piping is the case $e = r_1 - r_2$.
- **Parameters.** $r_1, r_2, h, e$ (direction of eccentricity relative to the seam datum), $t$.
- **Equations.** With $\mathbf w(\theta) = C_1(\theta) - A$:

$$\rho(\theta) = \lVert\mathbf w\rVert = \sqrt{r_1^2 - 2 a r_1\cos\theta + a^2 + H^2},$$
$$\frac{d\psi}{d\theta} = \frac{\lVert\mathbf w\times\mathbf w'\rVert}{\rho^2} = \frac{r_1\sqrt{H^2 + (r_1 - a\cos\theta)^2}}{r_1^2 - 2ar_1\cos\theta + a^2 + H^2},\qquad \psi(\theta) = \int_0^\theta \frac{d\psi}{d\vartheta}\,d\vartheta .\tag{4.1}$$

Pattern (polar about the developed apex): large end $\big(\rho(\theta),\ \psi(\theta)\big)$; small end $\big(\tfrac{r_2}{r_1}\rho(\theta),\ \psi(\theta)\big)$. $\psi$ has no elementary closed form (elliptic-type integral).
- **Equal radii with offset** ($r_1 = r_2 = r$, $e \ne 0$): oblique cylinder with ruling direction $\mathbf d = (e,0,h)/\ell$, $\ell = \sqrt{e^2 + h^2}$. Development: $u(\theta) = r\!\int_0^\theta\!\sqrt{1 - (e^2/\ell^2)\sin^2\vartheta}\;d\vartheta$ (incomplete elliptic integral of the second kind), lower edge $v(\theta) = (e/\ell)\,r\cos\theta$, upper edge $v + \ell$.
- **Procedure.** ALG-03 (general cone) with adaptive quadrature whose tolerance is allocated from the budget; boundary curves are not arcs → ALG-09.
- **Boundary conditions.** $\psi(2\pi)$ total angle $< 2\pi$ (injectivity); seam rulings at $\theta = 0$ and $2\pi$ have equal length; symmetric about the eccentricity plane when the seam lies in it ($\psi(2\pi - \theta) = \psi(2\pi) - \psi(\theta)$) — used as a check.
- **Limits.** $r_1 \to r_2$: apex recedes ⇒ switch to ALG-04; non-parallel end planes or non-circular ends ⇒ not this module (use §4.6 general developable loft, eq. 3.1); the wall thickness direction varies around an oblique cone, so the reference-surface correction is not a uniform radius offset — class A with the stated simplification, or computed by offsetting the surface (R-05).
- **Class.** E-N; A with thickness.
- **Errors.** Quadrature error bounded (interval-verified rule or comparison of two orders); $\varepsilon_{rep}$ from curve representation.
- **Independent verification.** (i) ALG-04 on the large circle (different formulation: geodesic curvature ODE, no apex); (ii) area identity: $\tfrac12\!\int\!\rho^2\,d\psi\,(1 - r_2^2/r_1^2)$ versus the surface integral $\iint\lVert S_\theta\times S_\lambda\rVert$ of the 3D ruled surface (verified numerically for ECC-01: both give 304 679.842 mm²); (iii) each developed boundary arc length equals $2\pi r_i$; (iv) fold-back and implicit residual; (v) discrete triangulation development converging to the same pattern at the expected rate (a deliberately different, cruder method).
- **Reference cases.** ECC-01 (flat-sided), ECC-02 (small offset, must approach CONE-01), OBLCYL-01.

### 4.4 Branch connections and intersections

- **Definition.** A branch (cylinder or cone, i.e. a ruled surface with straight rulings $P(\phi, s) = B(\phi) + s\,\mathbf d(\phi)$) trimmed against a header that is a quadric $F(\mathbf p) = \mathbf p^{\mathsf T}Q\mathbf p + 2\mathbf q^{\mathsf T}\mathbf p + c = 0$ (cylinder, cone, sphere, plane as a degenerate case).
- **General equation.** Substituting the ruling gives, for each $\phi$, a quadratic in $s$:

$$a s^2 + 2 b s + c' = 0,\quad a = \mathbf d^{\mathsf T}Q\mathbf d,\quad b = \mathbf d^{\mathsf T}(Q B + \mathbf q),\quad c' = F(B).\tag{4.2}$$

Roots by the cancellation-free form; root *selection* (which sheet of the header, near or far side) is a declared attribute, not a numerical accident. Discriminant sign changes mark where the branch leaves the header (partial intersection) — a topology event that is decided with interval arithmetic and otherwise reported as `Undecidable`.
- **Cylinder-on-cylinder, explicit.** Header axis = $x$-axis, contact radius $R$. Branch contact radius $r$, axis through $(0,\delta,0)$ with direction $(\cos\beta, 0, \sin\beta)$, $\beta\in(0,\pi)$; branch azimuth $\phi$ measured from $+y$ toward $(-\sin\beta, 0, \cos\beta)$:

$$P(\phi,s) = \big(s\cos\beta - r\sin\phi\sin\beta,\ \ \delta + r\cos\phi,\ \ s\sin\beta + r\sin\phi\cos\beta\big),$$
$$s(\phi) = \frac{\sqrt{R^2 - (\delta + r\cos\phi)^2}\; -\; r\sin\phi\cos\beta}{\sin\beta}\qquad(\text{upper sheet}).\tag{4.3}$$

Branch pattern: $(u,v) = (r_{dev}\,\phi,\ s(\phi))$. Header opening on the header development: $\omega = \arcsin\big((\delta + r\cos\phi)/R\big)$, $x = s\cos\beta - r\sin\phi\sin\beta$, pattern point $(x,\ R_{dev}\,\omega)$.
- **Parameters.** Both diameters with reference attributes, $t$ of each, $\beta$, $\delta$, set-on or set-in, which surfaces are in contact ($r$, $R$: e.g. branch inside surface on header outside surface), root gap, bevel.
- **Boundary conditions.** Full saddle requires $|\delta| + r \le R$; pattern closes: $s(0) = s(2\pi)$.
- **Limits.** $|\delta| + r \to R$ (including the equal-diameter tee): the intersection curve stays continuous but its slope becomes unbounded/cornered at the tangent points; adaptive sampling must place vertices exactly at these parameter values and fitting across them is forbidden. $|\delta| + r > R$: different topology (branch partly misses) → `OutOfRange` for this module. $\sin\beta \to 0$: ill-conditioned (grazing) → limit set by policy. Reinforcement pads, weld bevel variation around the saddle and the real contact line through the wall thickness are not pure geometry: class A.
- **Class.** E-C for the ideal contact surfaces (closed form involving a square root); A as fabricated.
- **Errors.** Conditioning of (4.3) near tangency: $ds/d\phi \sim (R^2 - y^2)^{-1/2}$; reported by ALG-11.
- **Independent verification.** (i) Implicit residuals: each 3D point must lie on both surfaces, tested by point-to-axis distances — a formulation that does not use the parametrisation; (ii) quadratic route (4.2) versus explicit (4.3); (iii) kernel surface–surface intersection through the kernel port, compared as curves (Hausdorff distance in interval); (iv) symmetry invariants ($\delta = 0$: $s(\pi - \phi) = s(\phi)$); (v) equal tee closed form $s = R|\sin\phi|$ at $r = R$, $\beta = \pi/2$, $\delta = 0$.
- **Reference cases.** BR-01…BR-04, BR-05 (equal tee), BR-06 (branch on cone via 4.2).

### 4.5 Rectangular-to-round transition

- **Surface construction (parallel end planes).** Rectangle with corners $Q_k$ in plane $z = 0$; circle of radius $r$ in plane $z = h$. The classical transition surface consists of four **planar triangles** (base = a rectangle side; apex = the circle point whose tangent is parallel to that side) and four **oblique conical patches** (apex = a rectangle corner $Q_k$; directrix = the circle arc between the two adjacent triangle apexes). Along each shared ruling the cone's tangent plane contains the ruling and the circle tangent, which is parallel to the rectangle side, so it coincides with the triangle's plane: the composite is tangent-continuous and **developable**.
- **Equations.** Each conical patch is developed with (4.1) generalised: $\mathbf w(\theta) = C(\theta) - Q_k$, $\rho = \lVert\mathbf w\rVert$, $d\psi/d\theta = \lVert\mathbf w\times\mathbf w'\rVert/\rho^2$. Triangles are developed by their three true lengths. Pieces are assembled in the plane by shared rulings (ALG-03 patch assembly).
- **Parameters.** Rectangle size and position, circle radius and centre offset (two components), $h$, $t$, seam location(s), number of pieces, forming method (rolled corners vs $N$ discrete bends per corner).
- **Panel division / seams.** Typical: seam on the centreline of one flat triangle (one-piece) or two/four pieces with seams on triangle centrelines; seam edges are straight lines whose developed lengths must match pairwise exactly.
- **Boundary conditions.** Developed circle-edge length $= 2\pi r_{dev}$; developed rectangle-edge lengths equal the rectangle sides; matching seam lengths.
- **Triangulation (traditional shop method).** Replacing each conical patch by $n$ flat triangles is an *approximation of the same development*; it converges to (4.1) as $n\to\infty$ (reference SQR-01 shows the convergence). If the part is actually formed by $n$ discrete bends, triangulation describes the **formed** shape and the facet deviation $r(1 - \cos(\pi/(4n)))$ per quarter is reported (§3.5).
- **Limits.** Circle must lie so that the four patches do not intersect each other (validity check on ruling crossings); non-parallel planes or non-circular round end ⇒ general loft (§4.6) with tangent-plane condition (3.1); thickness: reference surface of a composite with sharp plan corners is not a uniform offset (inside corner radius of the rectangular end, bend lines at patch boundaries) ⇒ class A with stated bend model (§6.2); near-degenerate geometry ($r$ approaching half the rectangle side, very small $h$) flagged by conditioning.
- **Class.** E-N for the ideal mid-surface; A as fabricated; X only if a non-developable blending surface is requested instead.
- **Independent verification.** (i) Triangulation at increasing $n$ with Richardson-style convergence to the integral result; (ii) invariants above; (iii) fold-back of pattern rulings to 3D (each pattern ruling length equals 3D ruling length; each pattern edge length equals 3D edge length); (iv) tangent-plane continuity residual at patch boundaries; (v) kernel-built loft compared for deviation (expected non-zero if the kernel constructs a different, non-developable loft — which is itself informative).
- **Reference cases.** SQR-01 (centred square-to-round), SQR-02 (offset), SQR-03 (rectangular).

### 4.6 General framework for non-catalogue geometry

No universal formula exists. The framework is a decision procedure over surface *regions*:

1. **Decompose** the part into faces (from a shape description, or from kernel B-rep data treated as untrusted geometry).
2. **Recognise and certify** each face as plane / cylinder / cone / general cone / general cylinder (MATH-08), or as a ruled surface satisfying the developability condition, or as *other*.
3. **Classify** (ALG-05): E, or measure curvature and proceed to X/N.
4. **Develop** by the method matching the class:
   - plane: rigid map;
   - generalised cylinder: arc length of the normal section (ALG-04 specialises);
   - generalised cone: ALG-03;
   - general developable (including a developable loft between two edge curves by eq. 3.1): ALG-04;
   - class X: distortion-minimising parametrisation of a mesh of the region, with the metrics of §3.5 — the method choice (e.g. length-preserving along a chosen direction versus conformal versus as-rigid-as-possible) changes the result and must be selected by rule and recorded (R-04);
   - class N: propose segmentation (gores, petals, panels) such that each piece becomes X within budget or E/A, or return *out-of-scope: forming*.
5. **Assemble** adjacent developed faces across shared edges where the joint is a bend or tangent-continuous; leave separate where it is a weld.
6. **Verify** with the generic checks of §9.1 — necessarily weaker than a family-specific verifier, which is reflected in the evidence's independence class and normally results in REVIEW REQUIRED for first-of-kind geometry.

A custom geometry becomes a catalogue module only through the shape-module contract: parametric definition, applicability range, verification method and reference cases.

---

## 5. Plate division and seam optimisation

### 5.1 Model

**Decision variables.** Number of pieces $N$; seam positions (angular $\theta_k$ for longitudinal seams, stations for girth/transverse seams); seam type per joint; for each piece: stock plate assignment, placement and orientation (optionally grain direction).

**Hard constraints** (each with an ID so that infeasibility can be explained):

| ID | Constraint |
|---|---|
| H-COV | Pieces cover the required surface exactly once (no gaps/overlaps beyond weld gap) |
| H-STOCK | Each developed piece, with cutting margins, fits an available stock plate (bounding test on the true pattern, not on its bounding box only, when orientation is free) |
| H-GEO | Each piece satisfies §3.4 (injective, regular) |
| H-PROC | Formability: roll/brake capacity, min/max radius, max plate width and thickness for the machine (`FR-04-1`) |
| H-SEAM | Seam rules: minimum distance between parallel seams, stagger between courses, seams clear of openings/branches/supports by a rule distance, prohibited zones (customer/standard rules — `FR-04-3`; none assumed) |
| H-ACC | Weld accessibility rule set (declared, not inferred) |
| H-TOL | Predicted tolerance budget (§8) satisfied for the strategy |

**Objective terms** (all reported separately):

$C_{mat}$ material consumed including waste (requires nesting estimate); $C_{weld}$ weld length × per-type cost factor; $C_{cut}$ cut length and pierce count; $C_{form}$ number and difficulty of forming operations; $C_{asm}$ number of pieces, fit-up operations, assembly sequence feasibility; $R_{dist}$ distortion-risk indicator (weld length and asymmetry about neutral axes, thin sections — an *indicator* requiring factory calibration, not a prediction).

$$\min\ \mathbf w^{\mathsf T}(C_{mat}, C_{weld}, C_{cut}, C_{form}, C_{asm})\quad\text{s.t. hard constraints};\qquad R_{dist}\ \text{reported as a separate objective.}$$

- **MATH-14** Safety- and rule-related items are hard constraints or separate objectives; they are never traded against cost inside a single weighted sum.
- **MATH-15** Weights and cost factors are factory configuration (ProcessData) with provenance; no defaults are shipped as if they were facts.

### 5.2 Solution method (ALG-12)

The search space is small and structured for the initial families (few seams, candidate positions from rules and symmetry). Method: enumerate $N$ and candidate seam sets on a rule-derived grid → filter by hard constraints (exact/interval comparisons) → evaluate objective vector → retain the **Pareto-nondominated set** → local continuous refinement of seam positions within feasibility → rank by configured weights. Nesting ($C_{mat}$) is NP-hard; a heuristic estimate is used for ranking and labelled as such; the final nest is verified for non-overlap exactly (ALG-08).

### 5.3 Explainability (required output)

For the selected strategy and each alternative: objective terms with units; binding constraints (which limits are active, and margin on the others); for rejected candidates the violated constraint IDs; sensitivity (how far a weight must change before the ranking changes); what was *not* modelled. The engineer selects; the selection is a Decision record. The lowest-cost strategy is presented as lowest-cost, not as "best".

---

## 6. Bending, rolling and manufacturing allowances

**Theory versus calibration.** Each model below has a geometric form (exact given its parameters) and parameters that are properties of material, tooling and machine. Parameter source classes: **EI** engineering input · **MD** material database · **CAL** machine/tooling calibration · **PT** physical test. A parameter lacking its required source is *assumed* → REVIEW REQUIRED (ARCH-11). **No universal K-factor, springback or kerf value is embedded in the software** (MATH-16).

### 6.1 Reference (neutral) surface

In bending, fibres inside the neutral surface shorten and those outside lengthen; the developed length is the length of the neutral surface, located at $r_n = r_i + k\,t$ with $0 < k \le 0.5$. For large $r_i/t$ (typical rolled shells) $k \to 0.5$ (mean radius); for tight bends $k$ decreases. $k$: **CAL/PT** per material, thickness, $r_i/t$ band and process; **EI** permitted with recorded justification.

### 6.2 Press-brake bends

Bend angle $\theta$ = angle through which the flange is rotated (the supplement of the included angle), in radians.

$$BA = \theta\,(r_i + k\,t)\qquad\text{(bend allowance: neutral arc length)}$$
$$OSSB = (r_i + t)\tan(\theta/2)\qquad\text{(outside setback, } 0 < \theta \le \pi/2\text{)}$$
$$BD = 2\,OSSB - BA\qquad\text{(bend deduction)}$$
$$L_{flat} = A + B - BD\qquad\text{(}A, B\text{ outside dimensions to the mould-line intersection)}$$

For $\theta > \pi/2$ the mould-line intersection moves and conventions for "outside dimension" differ between shops; the module requires the dimensioning convention as an explicit attribute and computes from neutral-arc geometry ($L_{flat} = \ell_1 + \ell_2 + BA$ with straight-leg lengths $\ell_i$), which is convention-free (MATH-17).

| Parameter | Source |
|---|---|
| $\theta$, leg dimensions and their convention | EI (drawing) |
| $t$ (nominal) and its tolerance | EI / MD |
| $r_i$ | CAL — in air bending it results from die opening and material, not from the punch radius alone |
| $k$ | CAL / PT |
| Springback | CAL / PT |
| Minimum bend radius, grain-direction limits | MD + PT |

**Springback.** Elastic recovery changes angle and radius after unloading; it depends on yield strength, modulus, $r/t$, tooling and method (air bending, bottoming, coining). A textbook approximation relates initial and final radius, $r_i/r_f = 4\big(\tfrac{r_i Y}{E t}\big)^3 - 3\big(\tfrac{r_i Y}{E t}\big) + 1$ (cited from memory; register R-08). It is admitted only as a prior for a calibration and never as a release-grade value. Springback affects *tool settings* more than the flat pattern; its effect on the pattern (through achieved radius) enters via $r_i$.

### 6.3 Rolling

Developed length of a rolled shell: $L = 2\pi (r_i + k t) - g + \Delta_w$ with $k$ as §6.1 (near 0.5 at large $r_i/t$, to be confirmed by trial: PT), weld-gap deduction $g$ (EI), and weld shrinkage allowance $\Delta_w$ (PT; empirical, process-dependent). Additional items that are **process facts, not geometry**: unbent flat ends on three-roll machines without pre-bending (extra length cut off, or pre-bend) — CAL; thinning/elongation in heavy rolling — PT; cone rolling behaviour — CAL. Validity range in $r_i/t$, plate width and material: CAL.

### 6.4 Cutting kerf

Kerf width $w_k$ (CAL per process, material, thickness, nozzle/consumables, speed) removes material both sides of the path centre. Compensation = offset of the profile by $w_k/2$ outward for outer profiles and inward for holes (ALG-10). **Exactly one party must apply it**: either our output or the CAM/machine controller. Which one is an explicit output-profile attribute (`FR-06`); double or missing compensation is a classic error and is a release-gate item (MATH-18). Thermal cutting taper/bevel means top and bottom profiles differ: the reference face is an attribute.

### 6.5 Weld preparation and joint geometry

Root gap, root face, bevel angle, set-on versus set-in branches determine *which surface* of each wall defines the cut curve (contact radii $r$, $R$ in §4.4) and deductions $g$. Sources: EI (welding procedure / drawing), customer rules. Variable bevel around a saddle (dihedral angle varies with $\phi$) is computed geometrically from the two surface normals: $\cos\Omega(\phi) = \mathbf n_{branch}\cdot\mathbf n_{header}$; the bevel schedule derived from it is class A. Weld shrinkage and distortion: PT.

### 6.6 Material-specific compensation

Any correction of the form "add/subtract $x$ per bend / per metre / per weld" is ProcessData: value, uncertainty, validity range (material, thickness, machine, tooling, date), calibration evidence, owner. Calibration procedure (trial pieces, measurement uncertainty, regression with parameter uncertainty, §2.4) is specified in Spec 04/10.

---

## 7. Numerical precision and error control

### 7.1 Arithmetic regimes — evaluation

| Regime | Strength | Limitation | Use in UEM |
|---|---|---|---|
| IEEE 754 binary64 | Fast; ~15–16 significant digits; correctly rounded $+,-,\times,\div,\sqrt{}$ | Rounding accumulates; cancellation; transcendental functions are **not** required to be correctly rounded and differ between math libraries | Default for constructions, with error tracking |
| Compensated / double-double (~31 digits) | Cheap extra precision for sums, dot products, determinants | Not a proof | First escalation step |
| Arbitrary precision floating point | Any precision | Slow; still rounding, needs bound analysis | Second escalation; reference values |
| Rational arithmetic | Exact for $+,-,\times,\div$ | No $\sqrt{}$, no trig; coefficient growth | Written dimensions; linear constructions; exact predicates on rational data |
| Exact geometric predicates (adaptive filters) | Correct sign of orientation/in-circle type determinants at near-float speed in non-degenerate cases | Covers specific polynomial predicates on float inputs; not curved or transcendental geometry | All line/segment topology decisions |
| Interval arithmetic (outward rounding) | Rigorous enclosure of the range of the computed expression | Dependency problem → over-wide intervals; needs careful formulation | Certified bounds on release-critical quantities; curved predicates; classification |
| Adaptive precision | Use the cheapest regime that decides | Complexity | Policy below |

### 7.2 Where each regime is required

- **MATH-19 Exact required:** storage of written dimensions; unit conversion factors; topological predicates on polygonal geometry (orientation, segment intersection existence, point-in-polygon, profile closure by vertex identity — closure is established by shared vertex identifiers, not by coordinate proximity).
- **MATH-20 Bounded numerical required:** every coordinate of a released profile ($\varepsilon_{num}$ from interval evaluation or a proven a-priori bound); quadratures in developments; root selection in intersections; comparisons against limits in gates (a comparison whose interval straddles the limit is *inconclusive*, not pass).
- **MATH-21 Approximate, explicitly identified:** class-X developments; curve representation (polyline, arc-fit, spline-fit) with $\varepsilon_{rep}$; heuristic nesting; any fitted primitive.
- **Curved topology.** Predicates involving arcs and transcendental curves (does this sinusoid cross that arc?) are decided by interval root isolation; if not decidable at the maximum configured precision the outcome is `Undecidable` → REVIEW REQUIRED (ARCH-08). Near-tangent profile contacts are a manufacturing concern in themselves and are reported with their separation interval.

**Escalation policy.** binary64 with running error → if the bound exceeds its budget share or a predicate is undecided → double-double → arbitrary precision / interval at higher precision → `Undecidable`.

### 7.3 Conditioning and error accumulation

Each algorithm sheet states its condition number with respect to inputs and the mitigations: stable quadratic roots; `hypot`-style norms; `atan2` instead of `acos` near 0 and $\pi$ ($\arccos$ loses half the digits there); avoiding apex-based formulas for near-cylindrical cones; arc length by quadrature rather than differences of large quantities; sums by compensated summation; angles reduced exactly before trig. Condition numbers above a threshold produce an outcome flag even if the bound is met (ill-conditioned problems amplify *input* uncertainty, §8).

### 7.4 Tolerance budget

For each released dimension or profile, with tolerance $T$ (from drawing/customer; none assumed):

$$\underbrace{\varepsilon_{num} + \varepsilon_{rep} + \varepsilon_{mod}}_{\text{bounded, deterministic}}\ +\ \underbrace{U_{inputs} + U_{process}}_{\text{expanded uncertainty}}\ \le\ \beta\,T .$$

$\beta \in (0,1)$ is the share of the tolerance allocated to pattern generation, the rest being reserved for cutting, forming, welding and measurement. $\beta$ and the tolerance sources are policy set with the factory in Spec 05 (`FR-05-1`). Deterministic bounds add linearly; uncertainties combine per §8. Internal numeric targets (e.g. $\varepsilon_{num}$ several orders below $T$) are set per algorithm so that numerics are never the limiting term.

### 7.5 Reproducibility

Three levels, distinguished in every claim and test:

| Level | Definition | Requirement |
|---|---|---|
| **R1 bitwise** | Identical output bits | Required for the pure-Rust UEM on the *same* build and platform, any thread count (MATH-22) |
| **R2 numerically equivalent** | Results differ by no more than the declared $\varepsilon_{num}$ of each | Required across supported platforms for UEM results |
| **R3 geometrically equivalent** | Geometry agrees within an approved tolerance and identical topology | Required for anything involving an external kernel |

Cross-platform R1 is a **goal, not a claim**: it is plausible for code restricted to correctly rounded IEEE operations with fused-multiply-add contraction disabled, fixed evaluation order and a single pinned software implementation of transcendental functions (no platform libm), but it is **undemonstrated** and listed for benchmarking (R-10). The release rule depends only on R2/R3: a regenerated result must match the archived one within declared bounds, with identical topology and identical gate outcomes.

---

## 8. Uncertainty and tolerance mathematics

### 8.1 Sources and how each is represented

| Source | Nature | Representation |
|---|---|---|
| Written dimension (nominal) | Authoritative | Exact value; **no** uncertainty; tolerance carried as requirement |
| OCR reading of a written dimension | **Discrete** misread risk | Alternative hypotheses with evidence; must be resolved (redundancy, cross-view consistency, human) — never a variance (MATH-31) |
| Dimension interpretation (which entity; inside/outside; which view) | Discrete | Alternative hypotheses → `Ambiguous` |
| Drawing resolution / scan measurement | Continuous | Pixel quantisation $\pm a$ → rectangular, $u = a/\sqrt3$; plus scale, skew and paper-distortion terms estimated from known dimensions on the same sheet |
| Vector-PDF measured geometry | Continuous, small | Coordinate precision of the file; "not to scale" risk handled by MATH-10 |
| Geometric reconstruction | Derived | Covariance from §2.4, valid only at full rank |
| Material (thickness, yield) | Continuous | MD tolerance band → rectangular or as specified |
| Process ($k$, kerf, shrinkage) | Continuous | CAL/PT estimate with standard uncertainty and validity range |
| Machine (positioning, repeatability) | Continuous | CAL; belongs to the non-pattern share $(1-\beta)T$ unless modelled |

Evaluation types follow the GUM distinction between statistical (Type A) and other (Type B) evaluation (to be verified against the document, R-11).

### 8.2 First-order propagation

For $\mathbf y = f(\mathbf x)$, $J = \partial f/\partial\mathbf x$ (by automatic differentiation, not finite differences): $\Sigma_y \approx J\,\Sigma_x\,J^{\mathsf T}$. Correlations are retained — pattern points share inputs and are strongly correlated; treating them as independent misstates seam-fit uncertainty (MATH-32). Validity is checked by a non-linearity diagnostic (second-order term or comparison with 8.3); if it fails the first-order result is not reported.

### 8.3 Monte Carlo propagation

Sample inputs from their assigned distributions (seeded, recorded), evaluate $f$, report mean, standard uncertainty and a coverage interval from quantiles; number of trials adapted until the interval endpoints stabilise to a set numerical tolerance (procedure to follow JCGM 101 once consulted, R-11). Required when: the model is strongly non-linear, input distributions are far from normal, or an output is near a limit.

### 8.4 Interval and worst-case analysis

Tolerances are bounds, not distributions. Worst-case: $\Delta y \le \sum_i |\partial y/\partial x_i|\,T_i$ (linearised) or rigorous interval evaluation of $f$ over the tolerance box. Statistical stack (root-sum-square) is reported only alongside worst-case and only when the independence/distribution assumptions are recorded. Which rule governs acceptance is contractual (`FR-05-1`).

### 8.5 Sensitivity and traceability

For each key output: contributions $c_i^2 u_i^2 / u_y^2$ with $c_i = \partial y/\partial x_i$, ranked; each $x_i$ is a truth-graph record, so every output uncertainty is traceable to source regions, assumptions and ProcessData (ARCH-12). The ranking tells the engineer which input to confirm to reduce uncertainty.

### 8.6 Uncertainty versus tolerance versus acceptance limits

- **Uncertainty** describes our knowledge of a value. **Tolerance** is the designer's permitted deviation. **Acceptance limits** are the limits used to decide conformity and may be tightened relative to the tolerance by a guard band related to $U$ (the principle of deciding conformity with account of measurement uncertainty; the exact rule is to be taken from ISO 14253-1 after review, R-12 — no compliance is claimed).
- Expanded uncertainty $U = k_c\,u$; the coverage factor and the distributional assumption behind it are recorded with the result.
- **MATH-33 Automatic-release limit.** If the budget inequality of §7.4 fails, or any input lacks an assigned uncertainty (MATH-30), or a discrete ambiguity is open, or the non-linearity diagnostic fails without a Monte Carlo result, the state is REVIEW REQUIRED at best.

---

## 9. Independent verification mathematics

### 9.1 Principles

- **MATH-40** A verifier must differ from the primary method in formulation, and is separately implemented in `uem-verify`; re-running the primary algorithm (even at higher precision or on another machine) is a *reproducibility* check, not verification.
- Independence classes recorded in Evidence: **I-F** different mathematical formulation · **I-I** different implementation of the same formulation · **I-L** different library/kernel · **I-P** physical measurement. Class I-I alone is insufficient for critical geometry.
- Verifiers consume original inputs and the primary *result*; they do not consume primary intermediates.

**Generic checks (all patterns).**

| Check | Statement |
|---|---|
| Length invariants | Developed length of each boundary/feature curve = its 3D length (independent quadrature) — class E/A; within strain metric for X |
| Seam compatibility | The two edges to be joined have equal developed length and matching station positions |
| Area invariant | Pattern area = surface area (independent integration) — E/A |
| Angle invariants | Angles between boundary curves at corners preserved |
| Fold-back | Pattern points mapped back by an independently coded inverse map satisfy the implicit equations of the target surfaces |
| Closure / topology | Profiles closed, simple, correctly oriented and nested (independent exact implementation) |
| Dimensional invariants | Written dimensions re-measured on the reconstructed 3D model reproduce the written values exactly (rational comparison where possible) |
| Units / scale | Bounding dimensions consistent with an independent estimate from the written dimensions |
| Tolerance / uncertainty | Budget inequality re-evaluated by the verifier's own propagation |

### 9.2 Family-specific independent methods

| Family | Primary | Independent method(s) |
|---|---|---|
| Cylinder + plane cuts | Closed form | Fold-back to implicit plane; ellipse arc length via elliptic integral; area |
| Right cone | Closed form (apex) | Apex-free geodesic-curvature integration (ALG-04); general-cone integral at $e=0$; invariants |
| Oblique cone | Integral (4.1) | ALG-04; area identity (surface vs polar); triangulation convergence; fold-back |
| Branch | Explicit (4.3) / quadratic (4.2) | Implicit distance residuals; the other of (4.2)/(4.3); kernel intersection (I-L); symmetry |
| Rect-to-round | Patch integrals + assembly | Triangulation convergence; true-length checks per ruling; tangent-plane continuity; kernel loft comparison |
| General developable | ALG-04 | Mesh-based isometric unfolding (triangle strip along rulings); invariants |
| Class X | Chosen flattening | Independent recomputation of distortion metrics from the mesh and its image; second flattening method for comparison (difference reported, not required to vanish) |
| Constraint solution | ALG-01/02 | Independent residual evaluation; interval certification; alternative solver |
| Bend/roll allowances | §6 formulas | Neutral-arc geometry recomputed from first principles; **physical calibration evidence (I-P) is the only verification of the parameters** |
| DXF | Writer | Independent parser re-import and comparison (Spec 06) |

### 9.3 Comparison and disagreement (ALG-13)

Primary value $P$ with bound $\varepsilon_P$, verifier $V$ with bound $\varepsilon_V$ (both enclosing the same exact quantity if both are correct):

- **Agree:** $|P - V| \le \varepsilon_P + \varepsilon_V$ **and** $\varepsilon_P + \varepsilon_V \le$ the check's resolution requirement (a verifier too coarse to be meaningful is *inconclusive*, not a pass).
- **Disagree:** $|P - V| > \varepsilon_P + \varepsilon_V$. At least one implementation or bound is wrong — a defect, not a tolerance question.
- **Inconclusive:** verifier out of its range, failed to converge, or too coarse.

For curves the comparison is a two-sided Hausdorff distance on certified samples.

- **MATH-41** Disagreement on critical geometry ⇒ gate FAIL ⇒ state REJECTED for that calculation record. It cannot be overridden by an approver. Resolution requires a new calculation or verifier version, with the defect recorded and a regression case added.
- **MATH-42** Inconclusive on critical geometry ⇒ REVIEW REQUIRED; an engineer may accept an alternative evidence path by Decision, which is recorded in the release.
- Agreement of two methods lowers, but does not remove, the probability of a common error in shared inputs or shared assumptions; evidence records state what was shared.

---

## 10. Engineering release classification

### 10.1 States

`REJECTED`, `REVIEW_REQUIRED`, `APPROVED`. (A result that passes every automated gate but lacks human approval is `REVIEW_REQUIRED` with reason `AWAITING_APPROVAL`.)

### 10.2 Gate predicates (machine-readable)

Each gate $G$ evaluates to `PASS | FAIL | INCONCLUSIVE | NOT_APPLICABLE` from truth-graph records only. Criticality (`critical | non_critical`) is fixed per gate in Spec 05.

```
G01 provenance_complete      : every parameter traces to Observation(accepted) | Decision | ProcessData
G02 no_open_ambiguity        : no Ambiguous / discrete-hypothesis record unresolved
G03 constraints_determined   : DOF report = well-determined; no Contradictory; hard residuals certified
G04 written_vs_measured      : no unresolved discrepancy record (MATH-10)
G05 class_assigned           : class ∈ {E,A,X}; (N ⇒ FAIL unless segmented into E/A/X pieces)
G06 assumptions_backed       : every assumption of class A has ProcessData(calibrated, in range) | Decision
G07 distortion_accepted      : class X ⇒ metrics ≤ budget ∧ Decision(accept_approximation)
G08 numeric_bounds           : eps_num, eps_rep present and within allocation; no Undecidable / NoConvergence
G09 budget                   : §7.4 inequality holds (interval-certain); MATH-33 conditions
G10 verification             : every required check for the family = PASS (agree); none FAIL
G11 profile_topology         : closed, simple, oriented, nested; kerf-compensation owner declared (MATH-18)
G12 process_constraints      : stock, machine, seam-rule constraints satisfied
G13 reproducibility          : regeneration matches at level R2/R3
G14 algorithm_status         : all algorithm versions are production-qualified (not experimental)
G15 output_integrity         : DXF re-import comparison PASS (Spec 06)
G16 human_approval           : Approval record, signed by a user with release authority for this tenant,
                               over the content hash of exactly the evaluated records
```

### 10.3 Transition rules

```
state(job) =
  REJECTED         if  any critical gate = FAIL
                   or  any EngineeringOutcome ∈ {Contradictory, NotDevelopable(unresolved), Degenerate}
                       on critical geometry
                   or  verification disagreement on critical geometry (MATH-41)
  APPROVED         if  all applicable gates G01..G16 = PASS
  REVIEW_REQUIRED  otherwise   (reasons = list of gates not PASS, with record references)
```

- REJECTED → (other): only by creating new records (new revision, corrected input, new algorithm version); the rejected record set stays immutable.
- REVIEW_REQUIRED → APPROVED: engineer Decisions resolve each listed reason *where the gate definition permits resolution by Decision* (G02, G04, G06, G07, G12-rule-waivers if customer policy allows), gates are re-evaluated, then G16.
- APPROVED → REVIEW_REQUIRED automatically when any upstream record is superseded (ARCH-15).
- **MATH-50** No gate predicate has a model confidence score among its inputs. `APPROVED` is unreachable without G10 and G16.
- **MATH-51** Gates G03, G08, G10, G11, G13, G15 cannot be resolved by Decision.

---

## 11. Implementation-ready algorithms

Pseudocode is language-neutral. Rust boundaries: each ALG is a pure function in the crate named in §1.1, generic over a `Scalar` trait implemented for `f64`, double-double, interval and dual numbers (automatic differentiation), so the same code yields values, bounds and derivatives. External kernels are reached only through `uem-kernel-port` (ADR-002/003) and never from `uem-exact`, `uem-interval` or `uem-verify` primitives.

### ALG-01 Constraint analysis and solve

```
input : vars x0 (with bounds), hard g[], soft r[] (with sigma), gauge (datum) spec
1  scale variables and equations to O(1)
2  x ← solve_hard_then_soft(x0):                     # SQP / null-space LM
      repeat: Jg ← ∂g/∂x (AD); Jr ← ∂r/∂x
              step from  min ½|r + Jr d|²  s.t. g + Jg d = 0   (QR-based)
              line search / trust region; until |g|,|proj grad| < tol or maxit → NoConvergence
3  svd(Jg restricted by gauge) → singular values s
      rank ← gap_criterion(s)  else  outcome RankUncertain
      dof ← n − rank;  null_basis ← right singular vectors beyond rank
4  if redundant equations: test residual of each against the rounding of its written value
      inconsistent → Contradictory{ minimal_conflict_set via deletion filter }
5  if dof > 0: check whether soft data span null_basis
      yes → tag those parameters provenance=measured, covariance from (Zᵀ JrᵀJr Z)⁻¹
      no  → UnderDetermined{dof, free parameters}
6  certify: if hard system square after gauge: interval Newton/Krawczyk on box around x
      → unique solution in box proven | not proven (flag)
7  discrepancy test of soft residuals vs sigma (MATH-10); robust outlier report
output: x, covariance, DOF report, residuals, condition number, certificates
```
Data: sparse Jacobian (constraints touch few variables); constraint graph for decomposition into small subsystems solved in order. Complexity: dense worst case $O(m n^2)$ per iteration; decomposition makes typical drawings near-linear in size. Risks: singular configurations, poor scaling, false rank from a singular point. Tests: §12 groups 5, 6, 10, 11.

### ALG-02 Solution enumeration

```
for each assignment of discrete branches (tangency side, arc major/minor, mirror, view side):
    for each start in deterministic low-discrepancy set within bounds:
        s ← ALG-01; if converged and feasible: add to set
cluster solutions (distance > certified separation ⇒ distinct)
low-dimensional module systems: interval branch-and-bound over the box instead (complete)
return [solutions];  |set| > 1 ⇒ Ambiguous with discriminating constraints listed
```
Risk: incompleteness of multi-start (stated in the outcome).

### ALG-03 General cone development (cones, oblique cones, conical patches)

```
input : apex A, directrix C(θ) on [θa, θb], second edge as fraction λ(θ) or curve, budget
1  w(θ) ← C(θ) − A;  ρ(θ) ← |w|;  require ρ ≥ ρ_min (apex margin) else Degenerate
2  f(θ) ← |w × w'| / ρ²
3  ψ(θ) ← adaptive quadrature of f from θa, with error bound per panel (interval-verified)
4  require ψ(θb) < 2π − margin  else NotDevelopable{overlap}
5  pattern outer: (ρ(θ) cos ψ, ρ(θ) sin ψ); inner: λ(θ)·same
6  samples chosen by ALG-07; bounds: eps_num = |∂p/∂ψ|·eps_ψ + eval error
output: boundary samples with bounds, mapping θ ↦ (ρ, ψ), class E-N
```
Complexity $O(N)$ evaluations. Risks: near-apex conditioning; integrand peaks when the directrix passes close to the apex direction.

### ALG-04 General developable development (apex-free)

```
input : boundary curve C(t), ruling direction D(t) (unit), ruling length ℓ(t), det[C',D,D']≡0 by construction
1  n(t) ← normalize(C' × D);  κg(t) ← (C' × C'')·n / |C'|³;  speed v(t) ← |C'|
2  integrate planar frame:  τ' = κg·v ;  c' = v·(cos τ, sin τ)      # ODE/quadrature, bounded
3  angle between tangent and ruling: ω(t) ← atan2((T × D)·n, T·D),  T = C'/v
4  second edge: c2(t) ← c(t) + ℓ(t)·rot(τ + ω)
5  check rulings do not cross within the strip (injectivity); check edge of regression outside strip
output: c(t), c2(t) with bounds; class E-N
```
Serves as primary for general developables and as the independent verifier for ALG-03 and the right cone. Risks: curvature evaluation needs $C''$; sign conventions for $n$ and $\omega$ (tested by mirror metamorphic tests).

### ALG-05 Developability classification

```
input : surface region (typed primitive | ruled surface | general parametric/NURBS)
1  typed plane/cylinder/cone/general cone/general cylinder → E (by construction)
2  ruled with developability identity proven (symbolic or by construction) → E
3  otherwise: interval-evaluate K over a subdivision of the region
       all boxes K > 0 or all K < 0 or |K| bounded away from 0 → not developable
       compute indicators ∫|K|dA, max|K|ρ²; trial flattening → distortion metrics
       metrics ≤ budget → X ; else → N (with segmentation proposal)
4  enclosures containing 0 everywhere without a proof → X (never E)   # MATH-01
5  global checks §3.4 (regularity, disk topology, injectivity)
```

### ALG-06 Ruling–quadric intersection curve

```
for φ in adaptive parameter set:
    a,b,c ← coefficients (4.2);  if |a| small → linear case
    Δ ← b² − a c   (interval);   Δ<0 certainly → no intersection at φ (topology event)
                                  sign undecided → refine φ / raise precision / Undecidable
    q ← −(b + sign(b)·√Δ);  roots s1 = q/a, s2 = c/q
    select root by declared sheet rule; verify continuity with previous sample
insert exact breakpoints where Δ = 0 or selection rule switches (root-find in φ)
output: s(φ) samples with bounds, breakpoints, 3D points
```

### ALG-07 Adaptive curve sampling

```
recursively bisect parameter interval until, on each piece, the interval-bounded deviation
of the curve from its chord ≤ δ_sample; always split at declared breakpoints
(tangency points, seam, patch boundaries, extrema of curvature)
```

### ALG-08 Profile validation (exact)

```
input : closed chains of segments/arcs with vertex identities
1  closure by identity; zero-length and duplicate element detection
2  sweep-line intersection detection: segments by exact predicates; arcs by exact
   circle–line / circle–circle algebra or interval isolation → simple | intersecting | Undecidable
3  orientation by exact signed area; nesting by exact point-in-profile
4  minimum feature distances (web, bridge) as intervals vs process minima
```
Complexity $O((n + k)\log n)$.

### ALG-09 Curve representation for output

```
input : exact/bounded curve, allowed entity types from output profile, δ_rep
polyline: vertices from ALG-07 (deviation certified by interval evaluation, chord-sagitta model
          d ≤ R − √(R² − c²/4) with R = 1/κ_max used only as a predictor)
biarc/arc fit: fit between breakpoints with tangent continuity; certify max deviation by
          interval evaluation; never fit across a breakpoint
output: entities + certified eps_rep + entity→curve-parameter map (provenance, ARCH-12)
```

### ALG-10 Offset (kerf, margins)

Offset of line/arc chains with exact handling of joins and removal of self-intersections (trim by ALG-08 arrangement); offset distance carries ProcessData provenance; result re-validated by ALG-08. Library candidates to be benchmarked (Spec 01 §4); result always verified by an independent distance check: every offset point lies at distance $w_k/2 \pm$ bound from the original profile.

### ALG-11 Uncertainty propagation

```
input : pure function f, inputs with (value, distribution|covariance|tolerance box)
1  missing uncertainty → error (MATH-30)
2  J ← AD(f);  Σy ← J Σx Jᵀ;  contributions per input
3  nonlinearity diagnostic: compare with second-order term / sigma-point evaluation
4  if diagnostic fails or requested: Monte Carlo (seeded), adaptive trials, coverage interval
5  worst-case: interval evaluation over tolerance box
output: u, U (with coverage factor and assumption), intervals, sensitivity ranking, method used
```

### ALG-12 Division strategy search

```
generate candidates (N, seam sets) from rule grid and symmetries
for each: develop pieces (cached by content hash) → hard-constraint filter (record violated IDs)
          → objective vector
pareto ← nondominated(feasible);  refine seam positions locally within feasibility
rank by weights;  compute weight-sensitivity of ranking
output: pareto set with explanations, selected candidate, rejected list with reasons
```
Complexity: bounded enumeration; cost dominated by developments (cached).

### ALG-13 Verification comparison

```
for each required check c of the family:
    V ← verifier_c(original inputs, primary result)          # uem-verify implementation
    if V inapplicable → INCONCLUSIVE
    d ← |P − V| (interval, or Hausdorff for curves)
    if sup(d) ≤ εP + εV and εP + εV ≤ resolution_c → PASS
    elif inf(d) > εP + εV → FAIL
    else → INCONCLUSIVE
    emit Evidence{check, method, independence class, shared inputs/assumptions, margin}
```

---

## 12. Mathematical test suite

Concrete cases: [Mathematical Reference Cases](../benchmark/MATHEMATICAL_REFERENCE_CASES.md). This section defines the groups and the evidence rules.

| # | Group | Content |
|---|---|---|
| 1 | Analytical reference | Closed-form results for every family (CYL, CONE, BR equal tee, BEND) |
| 2 | Exact geometric | Rational-coordinate configurations with exactly known predicates (collinear, cocircular, touching) |
| 3 | Numerical boundary | Near-cylindrical cone, near-tangent branch ($|\delta| + r \to R$), grazing angle $\beta \to 0$, large coordinates, tiny features |
| 4 | Degenerate | Zero radius, zero height, coincident axes, $r_1 = r_2$ with and without offset, apex inside region |
| 5 | Contradictory constraints | Inconsistent redundant dimensions; expected minimal conflict set |
| 6 | Under-determined | Missing dimension; expected DOF and free parameters; mirror ambiguities |
| 7 | High curvature | Small $r/t$, steep cones, sharp saddles: sampling density and bounds |
| 8 | Approximate developments | Spherical cap, torus segment: distortion metrics against analytic strain values; label checks (never "exact") |
| 9 | Tolerance boundary | Budget exactly met / just exceeded / straddled by the interval → PASS / FAIL / INCONCLUSIVE |
| 10 | Property-based | Random valid parameters: invariants of §9.1 hold; $T T^{-1} = I$; development ∘ fold-back = identity |
| 11 | Metamorphic | Rigid motion of inputs ⇒ congruent pattern; uniform scaling ⇒ scaled pattern; unit change ⇒ identical geometry; seam shift ⇒ cyclic re-cut with identical invariants; mirror ⇒ mirrored pattern; $e \to 0$ oblique cone ⇒ right cone; $\beta\to\pi/2,\delta\to0, r\to R$ ⇒ equal tee |
| 12 | Regression | Every defect found (including field and pilot findings) becomes a permanent case |
| 13 | Independent verification | Seeded faults (wrong radius reference, wrong root sheet, sign of $\psi$, mirrored pattern, dropped weld gap, double kerf, unit error, off-by-one sample at seam): each verifier must FAIL the fault it targets; verifier-too-coarse must yield INCONCLUSIVE |

**How tests become release evidence.**

1. Reference values are produced by a method independent of the implementation under test (closed form, or arbitrary-precision evaluation by separate code) and stored with their own derivation and precision.
2. An algorithm version becomes *production-qualified* (gate G14) only when: all groups applicable to it pass; its family has reference cases in groups 1, 3, 4, 10, 11, 13; seeded-fault detection is demonstrated; an independent reviewer has signed the algorithm sheet; and (for anything with process parameters) the benchmark's physical tier requirements are met (Spec 10).
3. A qualification record lists the corpus version, test counts and the **limits of what was shown** (parameter ranges covered). Outside those ranges the algorithm returns `OutOfRange`.
4. Passing the suite is necessary for qualification; it is **not** a claim that the algorithm is correct for all inputs. Per-job evidence (gates of §10) is always required in addition.

---

## 13. Open research and technical questions

Maintained in the [Mathematical Research Register](../research/MATHEMATICAL_RESEARCH_REGISTER.md). Summary:

- **Methods needing research:** general statement of the developable structure theorem with planar points (R-03); principled choice of flattening method for class X and its link to forming strain (R-04); reference-surface offset for oblique cones and composite transitions (R-05); completeness of multi-view reconstruction search (R-01); certified solution enumeration beyond low dimension (R-02); weld-distortion indicators (R-09).
- **Algorithms needing benchmarks:** interval over-estimation cost (R-06); offset/boolean libraries (R-07); cross-platform reproducibility (R-10); constraint decomposition on real drawings; quadrature strategies.
- **Factory input:** `FR-02-1…3` and those added below.
- **Physical calibration:** $k$ (bend and roll), springback, kerf, weld shrinkage, unbent ends, cone rolling limits.
- **Standards to review (not yet consulted):** GUM documents, ISO 14253-1, IEEE 754 / 1788, JIS drawing and tolerance standards (R-11…R-14).
- **Patent / prior art:** classification-driven release gating; dual-formulation verification of developments; provenance-typed constraint solving (R-15) — for Document 09, no novelty claimed.

### Additional factory requirements raised by this specification

| ID | Need |
|---|---|
| FR-02-4 | Which wall surface defines cut curves for set-on / set-in branches; weld preparation standards in use |
| FR-02-5 | Marking side convention for patterns (inside or outside up) |
| FR-02-6 | Seam datum conventions and customary seam positions per part family |
| FR-02-7 | Whether transitions are rolled or press-braked, and customary number of bends per corner |
| FR-02-8 | Who applies kerf compensation today (drawing office or machine/CAM) |
| FR-02-9 | Existing shop allowances (roll, bend, weld shrinkage) and how they were derived |
| FR-02-10 | Share of the tolerance currently consumed by cutting/forming/welding (to set $\beta$) |

---

## 14. Consistency with Specification 01 and the ADRs

| Item | Spec 01 / ADR | This specification | Resolution |
|---|---|---|---|
| UEM component list | Nine components (§3.1) | Twelve subsystems | Refinement: adds `uem-frame`, `uem-primitive`, `uem-param`, `uem-develop`, `uem-optim`; Spec 01 §3.1 table should be updated to reference §1.1 here. No decision reversed |
| Verification inside UEM | ARCH-03 and §3.4: Z3, must not call Z2 implementation | Task list named verification as a UEM subsystem | Kept **separate** (`uem-verify`, Z3-owned), sharing only audited primitives — Spec 01 prevails |
| Development classes | Four labels (§1.3 stage 5): exact / piecewise / approximate / not developable | E / A / X / N, adding "under defined assumptions" as requested and treating "piecewise" as a division strategy applied to any class | **Proposed amendment to Spec 01 §1.3 and master plan §6.2 C**; not applied to those documents pending review |
| Release states | Three statuses | Same three; `AWAITING_APPROVAL` is a reason within REVIEW_REQUIRED | Consistent |
| Reproducibility | §3.3: cross-platform bit-identity open | R1/R2/R3 levels; cross-platform R1 undemonstrated | Consistent; sharpens the open question |
| ADR-004 | Mixed regimes, pure library | Implemented as specified | No change to the ADR |
| ADR-003 | Kernel replaceable; own analytic layer narrow | All families here are analytic/numeric in Rust; kernel used for general modelling and as I-L verifier | Consistent; scope check: nothing here constitutes a general B-rep kernel |

No ADR decision is changed by this document.
