# Mathematical Research Register

Version 0.1 · 18 November 2026 · Companion to [Spec 02](../specifications/02_MATHEMATICAL_ENGINEERING_SPECIFICATION.md)

## How to read this register

- **No external source was consulted while writing Specification 02.** The mathematics there is classical and was derived directly; references below are given from the author's knowledge so that reviewers can find them. Titles, editions and dates must be confirmed when each document is obtained (column *Verified* = No until someone has the document in hand). No compliance with any standard is claimed anywhere in the project.
- Standards are paid documents; obtaining them is an action for the project owner.

## 1. Open research items

| ID | Topic | Question | Needed for | Type |
|---|---|---|---|---|
| R-01 | Multi-view reconstruction | Search strategy over candidate 3D interpretations; completeness and termination; literature on reconstruction from orthographic views | Spec 03/04 | Research |
| R-02 | Solution enumeration | Certified enumeration of all solutions of constraint systems beyond low dimension (interval methods, homotopy continuation); practical limits | ALG-02 | Research + benchmark |
| R-03 | Developable structure theorem | Precise hypotheses (planar points, regularity class) for "K ≡ 0 ⇒ plane/cylinder/cone/tangent developable"; handling of mixed regions | Spec 02 §3.3 | Literature review |
| R-04 | Class-X flattening | Which flattening objective corresponds to which forming process; relation between distortion metrics and forming strain limits; choice rule | Spec 02 §3.5, §4.6 | Research + factory input |
| R-05 | Reference surface for non-uniform walls | Neutral-surface position for oblique cones and transition pieces where wall inclination varies; error of the uniform-offset simplification | §4.3, §4.5 | Analysis + physical trial |
| R-06 | Interval over-estimation | Width of interval enclosures for the development integrals and intersection formulas; need for Taylor models / mean-value forms; run-time cost | §7 | Benchmark |
| R-07 | 2D offset / boolean robustness | Candidate libraries versus exact reference on arc-containing profiles | ALG-08/10; Spec 01 §4 | Benchmark + licence |
| R-08 | Springback model | Source and validity range of the textbook radius-ratio formula; alternatives; calibration design | §6.2 | Literature + physical calibration |
| R-09 | Weld distortion indicator | Whether a simple indicator has predictive value for the factory's parts | §5 | Factory data |
| R-10 | Cross-platform reproducibility | Whether bitwise identity is achievable for the pure-Rust UEM (pinned transcendental implementations, no FMA contraction); cost | §7.5 | Benchmark |
| R-11 | Uncertainty framework | Exact procedures and terminology of the GUM documents; Monte Carlo adaptive procedure | §8 | Standards review |
| R-12 | Conformity decision rules | Exact rule and guard-band definitions | §8.6, Spec 05 | Standards review |
| R-13 | Floating-point and interval standards | Guarantees actually provided (which functions are correctly rounded; reproducibility clauses) | §7 | Standards review |
| R-14 | Japanese drawing / tolerance standards | Projection method, dimensioning, general tolerances for welded and sheared/cut parts, welding symbols — identify the applicable JIS documents | Spec 03/05 | Standards review + factory input |
| R-15 | Prior art | Dual-formulation verification of flat patterns; classification-gated release; provenance-typed constraint solving; triangulation and unfolding software and patents | Document 09 | Patent search (counsel) |
| R-16 | Rank determination | Robust rank/gap criteria for scaled constraint Jacobians on real drawings | ALG-01 | Benchmark |
| R-17 | Curve fitting for CAM | Arc/biarc fitting with certified deviation; what entity types the factory's CAM accepts | ALG-09, Spec 06 | Benchmark + factory input |

## 2. Models requiring physical calibration

| Model | Parameters | Evidence needed |
|---|---|---|
| Neutral factor in press-brake bending | $k$ per material / thickness / $r_i/t$ / tooling | Bend trials with measured legs |
| Achieved inside radius (air bending) | $r_i$ vs die opening and material | Trials |
| Springback | Angle and radius recovery | Trials |
| Rolling development | $k$ near mean radius; elongation; unbent flat ends | Rolled-shell trials, circumference measurement |
| Cone rolling | Achievable geometry, compensation | Trials |
| Kerf | Width and taper per process / material / thickness / consumable state | Cut coupons |
| Weld shrinkage | Transverse and longitudinal per joint type | Before/after measurement |
| Faceted forming of transitions | Deviation versus number of bends | Trial pieces |

All become ProcessData with uncertainty, validity range and evidence; none has a software default.

## 3. Reference sources (to be obtained and verified)

| Ref | Document (as recalled) | Use | Verified |
|---|---|---|---|
| S-01 | M. P. do Carmo, *Differential Geometry of Curves and Surfaces* (Prentice-Hall, 1976; revised ed. Dover, 2016) | Fundamental forms, Theorema Egregium, developable surfaces, geodesic curvature | No |
| S-02 | H. Pottmann, J. Wallner, *Computational Line Geometry* (Springer, 2001) | Developable surfaces, ruled-surface conditions | No |
| S-03 | J. R. Shewchuk, "Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates", *Discrete & Computational Geometry* 18 (1997) | Exact predicates | No |
| S-04 | N. J. Higham, *Accuracy and Stability of Numerical Algorithms*, 2nd ed. (SIAM, 2002) | Error analysis, conditioning, stable formulas | No |
| S-05 | J. Nocedal, S. J. Wright, *Numerical Optimization*, 2nd ed. (Springer, 2006) | Least squares, SQP | No |
| S-06 | R. E. Moore, R. B. Kearfott, M. J. Cloud, *Introduction to Interval Analysis* (SIAM, 2009) | Interval Newton, enclosures | No |
| S-07 | IEEE Std 754-2019, *IEEE Standard for Floating-Point Arithmetic* | Arithmetic guarantees | No |
| S-08 | IEEE Std 1788-2015, *IEEE Standard for Interval Arithmetic* | Interval arithmetic | No |
| S-09 | JCGM 100:2008, *Evaluation of measurement data — Guide to the expression of uncertainty in measurement* (BIPM; freely available from bipm.org) | Uncertainty framework | No |
| S-10 | JCGM 101:2008, *Supplement 1 to the GUM — Propagation of distributions using a Monte Carlo method* | Monte Carlo propagation | No |
| S-11 | JCGM 106:2012, *The role of measurement uncertainty in conformity assessment* | Decision rules, guard bands | No |
| S-12 | ISO 14253-1 (edition to be confirmed; 2017 recalled), *GPS — Inspection by measurement of workpieces and measuring equipment — Part 1: Decision rules for verifying conformity or nonconformity with specifications* | Conformity decisions | No |
| S-13 | ISO/IEC Guide 98-1:2024 (as cited in the master plan) | Introduction to uncertainty guides | No |
| S-14 | S. Kalpakjian, S. R. Schmid, *Manufacturing Engineering and Technology* (edition to be confirmed) | Bend allowance, springback approximation | No |
| S-15 | Applicable JIS standards for technical drawings, general tolerances and welding symbols | **Not identified** — numbers deliberately not quoted from memory | No |
| S-16 | CGAL and kernel documentation (see licensing review) | Exactness models of candidate libraries | Partly (licensing only) |

## 4. Review actions before Specification 02 can be approved

1. Independent derivation check of §4 equations by a second qualified person (mechanical/manufacturing engineer or mathematician).
2. Independent recomputation of the reference cases.
3. Factory review of §5 and §6 (process realism; inputs FR-02-1…10).
4. Standards S-09…S-12 obtained and §8 reconciled with their text.
5. Decision on the proposed amendment of the development-class labels in Spec 01 and the master plan (Spec 02 §14).
