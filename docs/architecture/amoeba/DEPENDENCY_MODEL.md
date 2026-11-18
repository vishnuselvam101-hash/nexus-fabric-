# Engineering Dependency Model

Version 0.1 (refinement A) · 18 November 2026 · Proposed design under ADR-010 conditions · Responds to review findings W1, W2 and requirement "dependency-following investigation"

**Review status (18 November 2026): not reviewed by any fabrication engineer.** Review is requested through the [Fabrication Engineering Review Package](../../reviews/FABRICATION_ENGINEERING_REVIEW_PACKAGE.md); until it is completed every rule, edge type and example here is an unvalidated hypothesis.

**Status.** Proposed [P]. The worked examples are paper exercises using symbolic values; they demonstrate the mechanism, not a validated capability. The rule table (§4) is a first draft to be reviewed by fabrication engineers — which checks a defect *should* trigger is engineering knowledge, and gaps in it are gaps in the product.

---

## 1. Purpose

Make the question "what else is affected?" answerable by traversal. A finding in one place must create explicit, tracked **scrutiny obligations** on everything that depends on it, mates with it, or shares its cause — and release must be impossible while relevant obligations are open.

The earlier design had `derived_from` edges only. That supports *downstream invalidation* (recompute what was computed from a changed input) but not *lateral* effects (the part that must fit this one) or *common-cause* effects (everything else read the same way, or computed by the same algorithm version).

## 2. Nodes

Dependency edges connect existing EOS records. Node kinds used below:

| Group | Kinds |
|---|---|
| Product structure | Assembly, Subassembly, Part |
| Geometry | Surface, Feature (edge curve, end, opening, axis), IntersectionCurve |
| Definition | Dimension (observation), Constraint, Parameter |
| Interfaces | **Interface** (new): a joint between two or more parts — records the mating features on each side, the fit condition and the joint type |
| Manufacturing | Material, Thickness, ProcessData, Assumption (e.g. reference-surface choice, contact-surface choice), FormingMethod |
| Fabrication | Pattern (development), Piece (result of plate division), Seam, Weld, BendLine |
| Output | Profile, DxfEntity, Package |
| Computation | Activity (calculation / process instance), AlgorithmVersion, Template |
| Assurance | Evidence, GateEvaluation, CoverageStatement, Decision |

## 3. Edge types

Every edge is an immutable record: `{type, from, to, roles, class, schema_version, created_by activity}`. "from depends on to" unless marked symmetric. The type table itself is versioned; adding a type is a qualified change.

| Class | Type | From → To | Meaning |
|---|---|---|---|
| **Derivation** (hard) | `computed_from` | any derived record → its inputs | Output of an activity |
| | `defined_by` | Parameter → Constraint → Dimension | A value exists because of this definition |
| | `bounded_by` | Surface → Feature | Trim/extent |
| | `developed_from` | Pattern → Surface (+ Assumptions) | Flat pattern of a surface |
| | `divided_from` | Piece → Pattern | Plate division |
| | `rendered_from` | Profile → Piece/Pattern; DxfEntity → Profile | Output representation |
| **Containment** | `part_of` | Part → Subassembly → Assembly; Feature → Part | Structure |
| **Interface** (symmetric) | `mates_at` | Part ↔ Interface | Participation in a joint |
| | `interface_feature` | Interface → Feature (each side) | Which features must fit |
| | `intersects` | Surface ↔ Surface → IntersectionCurve | Shared curve owned by both |
| | `joined_by` | Piece edge ↔ Piece edge → Seam | Two edges welded together |
| | `positioned_by` | Part → Constraint (assembly placement) | Location/orientation relative to another part |
| **Manufacturing** (hard) | `uses_material`, `uses_thickness`, `uses_process_data`, `assumes`, `formed_by` | Part/Pattern/Bend → the respective record | Physical basis of a calculation |
| | `welded_as` | Seam/Interface → Weld (preparation, gap) | Joint geometry basis |
| **Assurance** | `verified_by` | record → Evidence | |
| | `decided_by` | record → Decision | |
| **Equivalence** | `same_feature_as` | observation ↔ observation (cross-view) | Same physical feature |
| | `restates` | Dimension ↔ Dimension | Redundant statement |
| **Common cause** | `read_by` | observation → (recogniser version, source region, transform chain) | Same acquisition circumstances |
| | `computed_by` | Activity → AlgorithmVersion | Same code |
| | `instantiates` | record → Template / shape-module version / convention record | Same rule or convention applied |
| | `convention_of` | Dimension → sheet-level declaration (units, surface reference, projection, general tolerance) | Same drawing-level interpretation |

Interfaces are the key addition. An **Interface** record states what must be true for parts to fit: e.g. *"branch end-cut curve of B and opening curve on H are the same IntersectionCurve Γ"*, *"developed length of seam edge e₁ of piece P₁ equals that of e₂ of P₂"*, *"circumference of reducer large end equals circumference of pipe end, on the declared reference surfaces"*. These are **interface constraints**: checkable equalities/inequalities over records owned by different parts.

- **DEP-01** Every joint between parts or pieces has an Interface (or Seam) record with at least one checkable interface constraint. A part with a mating neighbour and no Interface record is a gap Issue.
- **DEP-02** Edges are created by the activity that creates the dependency, at the time it does so; they are never reconstructed afterwards by inference.

## 4. Propagation

When a **Finding** is confirmed on a record *x* (a conflict resolved as a real error, a correction, a verification failure, an out-of-range process parameter, a quarantine, a revision change), propagation creates obligations in three directions:

| Direction | Follows | Semantics | Bound |
|---|---|---|---|
| **Downstream** | Derivation and Manufacturing edges, reversed (who depends on *x*) | Everything derived is *invalid* until recomputed and re-verified | **Unbounded and mandatory** — this is correctness, not exploration |
| **Lateral** | Interface edges | Partners must be *re-checked for fit* against the changed side; if the interface constraint now fails, a new Finding is raised on the partner (which propagates in turn) | One interface hop mandatory; further hops only through a new Finding |
| **Common-cause** | Common-cause and Equivalence edges | Siblings sharing the cause are *suspect*: targeted re-examination | Same sheet/declaration/algorithm version: mandatory for release-critical records; wider: discretionary, budgeted |

**Scrutiny obligation** record:

```
Obligation { id, origin_finding, path: [edge ids], target, direction,
             required_checks: [check ids from rule table], class: mandatory | discretionary,
             state: open | discharged(evidence ids) | superseded | waived(decision id) }
```

- **DEP-03** Obligations are created by a deterministic function of (finding kind, edge type, target kind) from a versioned, human-authored **rule table**. The table is production-lane data; it is never generated or modified by a model.
- **DEP-04** A mandatory obligation can be discharged only by evidence from the required checks, or waived by an engineer's Decision with reason. It cannot expire, be dropped for budget, or be stopped by a yield rule.
- **DEP-05** Gate **G18 `obligations_discharged`** (adopted by owner decision D-28; see the [Release Gates Addendum](RELEASE_GATES_ADDENDUM.md)): no open mandatory obligation targets a record in the release closure. To be incorporated into Spec 02 at its next revision.
- **DEP-06** Propagation terminates: downstream traversal is over a DAG; lateral propagation continues only via *new* confirmed Findings, each recorded; common-cause sets are finite and visited once per originating finding.

Rule table (first draft, excerpt):

| Finding on… | Edge | Target | Obligation (checks) | Class |
|---|---|---|---|---|
| Dimension corrected / reinterpreted | `defined_by`⁻¹ … | Parameters, surfaces, curves, patterns, profiles, DXF | Recompute; re-verify; re-validate profile; re-import check | mandatory |
| Parameter of a surface changed | `intersects` | Partner surface's intersection curve, partner's cut/opening | Recompute Γ; re-derive partner pattern; interface fit check | mandatory |
| Any change on one side of an Interface | `mates_at` | The other part(s) | Evaluate interface constraints; fit-up rehearsal | mandatory |
| Pattern changed | `joined_by` | Partner piece edge | Seam length and station match | mandatory |
| Thickness / material / ProcessData changed or out of range | `uses_*`⁻¹ | Every pattern, bend, weld basis using it | Recompute allowances; validity-range check; stock-fit check | mandatory |
| Reading corrected (misread) | `read_by` | Other class-H readings from the same recogniser on the same region/condition | Re-confirmation by a person | mandatory if in a release closure, else discretionary |
| Convention changed (inside/outside, units, projection, scale) | `convention_of`⁻¹ | Every dimension interpreted under it | Re-evaluate each; recompute dependants | mandatory |
| Verification disagreement on a calculation | `computed_by` | All activities by that algorithm version (this job; other jobs via quarantine) | Quarantine version; re-verify by independent method | mandatory |
| Redundant dimension found inconsistent | `restates`, `same_feature_as` | The other statements of that feature | Conflict investigation | mandatory |
| Template/convention record found wrong | `instantiates`⁻¹ | Everything produced with it | Re-examination | mandatory for closure records |

## 5. Worked examples

Notation: boxes are records; `→` reads "depends on". Symbols instead of numbers.

### Example 1 — Pipe header with an oblique set-on branch

**Design.** Header H: cylinder, written diameter `D_H`, thickness `t_H`, rolled from plate (one longitudinal seam). Branch B: cylinder, `D_B`, `t_B`, at angle `β`, offset `δ`, located at station `x_B`. Second branch B2 further along. Flange F on one header end. Set-on joint: branch end sits on the header's outer surface.

**Dependency structure (excerpt).**

```
Dimension D_H ──defined_by── Parameter R_H(contact) ── Surface S_H(outer)
   │ convention_of: sheet declaration "diameters are outside"            │ intersects
   │                                                                      ▼
Dimension D_B, t_B ── R_B(contact = inner), R_B(dev) ── Surface S_B ── IntersectionCurve Γ
Dimension β, δ, x_B ── positioned_by (assembly placement) ───────────────┘
        Γ ──developed_from── Pattern P_B (branch end cut)  ── Profile ── DXF_B
        Γ ──developed_from── Pattern P_Hopen (opening on header development) ┐
   R_H(dev) = R_H + k·t_H (assumes k) ── Pattern P_H (shell) ◄───────────────┘ ── Profile ── DXF_H
        Interface I_HB: {B.end curve, H.opening curve} must both equal Γ; weld W_HB (bevel from dihedral along Γ)
        Interface I_HB2 (branch 2), Interface I_HF: flange bore ↔ header outer diameter
        Seam of P_H: position θ_s; rule: clear of openings by a rule distance
```

**Finding.** Reconciliation RC-03 raises a discrepancy on `D_H`; investigation and the engineer conclude the header diameter on this sheet is the **inside** diameter (convention record corrected). Effective outer radius changes by `t_H`.

**Obligations generated.**

| # | Direction | Path | Target | Required checks |
|---|---|---|---|---|
| 1 | Downstream | D_H → R_H(contact) → S_H → Γ | Γ | Recompute intersection (ALG-06); verify by implicit residual and kernel cross-check |
| 2 | Downstream | Γ → P_B → Profile → DXF_B | Branch pattern and output | Re-develop; profile validation; re-import comparison |
| 3 | Downstream | Γ → P_Hopen → P_H → DXF_H; R_H(dev) → P_H width | Header shell pattern, opening, output | Re-develop; seam closure; opening position |
| 4 | Lateral | I_HB | Branch B (owned by a different part) | Interface constraint: B's cut curve and H's opening both equal the new Γ; fit-up rehearsal |
| 5 | Lateral | W_HB `welded_as` | Bevel schedule along Γ | Recompute dihedral; weld preparation review |
| 6 | Lateral | I_HB2 | Branch B2 | Same as 1–5 for the second branch (a branch nobody touched) |
| 7 | Lateral | I_HF | Flange F | Bore/OD fit check — may raise a **new Finding** on F if F was specified against the old interpretation |
| 8 | Lateral | Seam rule | Seam position θ_s of P_H | Clearance from the (moved) openings |
| 9 | Common-cause | `convention_of` | Every diameter on the sheet interpreted "outside" (including `D_B`) | Re-evaluate surface reference for each; engineer confirmation |
| 10 | Common-cause | `instantiates` (customer convention record) | Other sheets of the package using the same convention record | Discretionary unless in a release closure |

What a conventional invalidation would do: items 1–3. What the dependency model adds: 4–10 — in particular 6 and 7 (parts not derived from the changed dimension but which must fit it) and 9 (the same mistake probably repeated).

### Example 2 — Eccentric reducer in two halves between two pipes

**Design.** Reducer E (oblique cone frustum; radii `r₁`, `r₂`, height `h`, offset `e`), divided into two pieces E₁, E₂ by two longitudinal seams σ_a, σ_b; girth joints to pipe P₁ (large end) and pipe P₂ (small end).

**Structure.** Surface S_E → Pattern Π_E (`computed_by` general-cone module v) → `divided_from` → Pieces E₁, E₂; Seams σ_a, σ_b (`joined_by` edges of E₁ and E₂); Interfaces I_E1: {E large-end circle ↔ P₁ end circle}, I_E2 likewise; ProcessData for rolling (`assumes` k).

**Finding.** Independent verification (apex-free method) **disagrees** with the primary development of E₁ beyond the sum of declared bounds (Spec 02 MATH-41) → calculation REJECTED.

| # | Direction | Target | Required checks |
|---|---|---|---|
| 1 | Downstream | Profiles and DXF of E₁ | Invalid; recompute after resolution |
| 2 | Lateral (`joined_by`) | E₂ edges at σ_a, σ_b | Seam length and station equality against E₁ — E₂ came from the same pattern and is equally suspect |
| 3 | Lateral (I_E1, I_E2) | Pipe ends P₁, P₂ | Circumference equality on the declared reference surfaces; re-run when E is recomputed |
| 4 | Common-cause (`computed_by`) | Every activity in the project run with general-cone module v (e.g. the conical patches of a transition elsewhere in the package) | **Quarantine** module version v; re-verify each affected result by an independent method |
| 5 | Common-cause (cross-job) | Other jobs, including approved ones, whose release closure contains module v | Issue raised on each; approval validity re-evaluated |
| 6 | Self-audit | The verifier itself | The disagreement may be the verifier's fault: investigation must test both (Investigation Model §2), not assume the primary is wrong |

### Example 3 — Rectangular-to-round transition, press-braked, after a thickness revision

**Design.** Transition T between a rectangular flange frame R and a round duct C; two pieces with seams on the centrelines of two flat triangles; corners formed by `N` discrete bends each; bend allowances from ProcessData `PD(material, t, tooling)`.

**Finding.** Drawing revision B changes thickness `t` from `t₀` to `t₁`. `PD` has a validity range that does not include `t₁` → Finding *process data out of range* (Spec 02 `OutOfRange`), plus the revision change itself.

| # | Direction | Target | Required checks |
|---|---|---|---|
| 1 | Downstream (`uses_thickness`, `uses_process_data`) | Reference surface, every bend allowance, patterns of both pieces, profiles, DXF | Recompute — blocked until calibrated ProcessData for `t₁` exists or an engineer Decision supplies values (flagged *assumed*) |
| 2 | Downstream | Plate division feasibility (stock fit, machine capacity for `t₁`) | Re-evaluate hard constraints H-STOCK, H-PROC; the previous strategy may no longer be feasible |
| 3 | Lateral (I_TR) | Rectangular frame R | Rectangular end dimensions are inside or outside the wall? Fit shifts by multiples of `t₁ − t₀`: interface constraint re-evaluated; possible new Finding on R |
| 4 | Lateral (I_TC) | Round duct C | Circumference on reference surface vs duct end |
| 5 | Lateral (`joined_by`) | Both seams | Developed seam lengths equal after recomputation |
| 6 | Lateral (`welded_as`) | Weld preparations | Preparation valid for `t₁`? |
| 7 | Common-cause (`uses_thickness`/`uses_material` siblings) | Every other part in the package that the revision note applies to | Check whether revision B's thickness change applies to them (revision scope is itself an observation to confirm) |
| 8 | Approval | Any approval whose release closure contains `t₀`-dependent records | Invalid by closure rule ([Determinism and Approval](DETERMINISM_AND_APPROVAL.md) §4) |

## 6. Why this is more than a fixed pipeline with invalidation

A fixed pipeline can recompute dependants (downstream) and can run a fixed list of checks on everything. It cannot, without this model: know which *other parts* must be re-checked for fit when one changes (interfaces are not derivations); recognise that a corrected reading or convention makes specific siblings suspect; or block release on exactly the set of affected records. Whether directing effort this way finds more defects per unit of computation than running every check everywhere is **not assumed** — it is hypothesis H-DEP in the [benchmark protocol](../../benchmark/AMOEBA_BENCHMARK_PROTOCOL.md). If all checks are cheap enough to run everywhere, the *selectivity* has no value; the *obligation tracking and release blocking* remain valuable regardless.

## 7. Open questions

1. Completeness of the rule table — requires fabrication-engineer review; which interface constraints exist for each joint type in the factory's work (`FR-04`).
2. How assembly-level placement constraints are captured when only detail drawings are supplied (interfaces unknown ⇒ gap Issues; how many jobs is that?).
3. Granularity: obligations per record can be numerous on large assemblies; grouping by part/pattern without losing traceability.
4. Cross-job propagation (example 2, item 5): operational and contractual handling when an algorithm fault touches released work.
5. Tolerance interaction across interfaces (stack-up across several parts) — Spec 05.
6. Generalisation to other domains: the edge classes are domain-neutral; edge types and the rule table are domain packs. A *domain contract* (what a domain must supply: node kinds, edge types, interface constraint types, rule table, shape modules, verification methods) is to be written with Spec 04.
