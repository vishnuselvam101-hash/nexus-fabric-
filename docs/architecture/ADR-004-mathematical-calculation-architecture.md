# ADR-004: Mathematical calculation architecture

**Status:** Proposed · 2026-11-18 · Spec: [01 §3](../specifications/01_SYSTEM_ARCHITECTURE_SPECIFICATION.md)

## Context
Outputs must be traceable, reproducible and bounded in error. Different problems need different numeric regimes; a single strategy (all-f64 or all-exact) is either unsafe or impractically slow.

## Options
1. **f64 everywhere with tolerances.** Fast, conventional; topological decisions can be wrong near degeneracy; error is not quantified.
2. **Exact arithmetic everywhere.** Strong guarantees for algebraic geometry; transcendental quantities (arc lengths, trig) are not exactly representable; severe cost.
3. **Mixed regimes with mandatory metadata:** exact predicates for decisions; exact rational constructions where linear; f64/multi-precision numerics with stated bounds elsewhere; interval enclosure for release-critical quantities; uncertainty propagation as a separate layer; process models parameterised by calibrated data.

## Decision (proposed)
Option 3, implemented as the UEM: a pure library (no I/O, no AI, no global state) split into the components in Spec 01 §3.1. Every result carries value, unit, regime, bound/uncertainty and applicability. Verification uses separately implemented methods (Spec 01 §3.4).

## Advantages
Guarantees matched to risk; numerical error and input uncertainty are both explicit and kept distinct; pure functions enable caching, replay and property-based testing.

## Disadvantages
More complex API (no bare floats); developer discipline; interval results can be pessimistic.

## Risks
Confusing *numerical error bounds* with *measurement uncertainty* with *manufacturing tolerance* — three different things that Specification 02/05 must define separately; over-wide intervals causing needless Review Required; independence between Z2 and Z3 implementations decaying over time.

## Validation required
- Specification 02 defines, per algorithm: formulation, regime, bound derivation, applicability range, reference cases.
- Prototype reducer and branch developments in two independent formulations; compare within interval bounds.
- Cost measurement of interval/exact layers on representative parts.
- Passing a set of tests is not accepted as proof of reliability: acceptance criteria and evidence classes come from Specification 05.
