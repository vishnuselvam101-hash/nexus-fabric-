# Stress Testing and Learning from Mistakes

19 November 2026

## What this is, and what it is not

The engineering core of NEXUS FABRIC is deterministic mathematics, not a trained model. It cannot be "trained" in the machine-learning sense, and it does not rewrite its own algorithms. Two mechanisms do the job that "training on hard cases" and "learning from mistakes" describe:

1. **Stress and mutation campaigns** throw thousands of hard and extreme parts at the engine and fail on any result that is not either verified or explicitly refused. Every defect found becomes a permanent regression case.
2. **Lessons** are recorded at run time whenever something goes wrong in a project, and every later job in that project is checked against them.

**Zero errors is the target for known, tested situations. It is not a guarantee.** A campaign shows that no defect was found in the cases tried; it cannot show that none exists. Physical accuracy still depends on thickness, allowances and calibration that the software does not yet model.

## 1. Stress campaign (`crates/nf-core/tests/stress.rs`)

Each generated case must end as a pattern whose independent checks, profile check and DXF re-import all pass and which replays to the identical release closure — or as an explicit refusal with no pattern and no DXF. Anything else is a defect.

Hard cases generated: sizes from 0.01 mm to 100 m; near-cylindrical cones; very steep cones whose development approaches a full turn; reducers from concentric to strongly oblique; branches with almost equal diameters, at and beyond tangency, at grazing angles; assemblies with the opening off the shell; invalid and non-finite inputs.

Result of the run on 19 November 2026 at scale 6:

| Family | Verified | Refused | Defects |
|---|---|---|---|
| Cylinder and right cone | 4,324 | 484 | 0 |
| Eccentric reducer | 746 | 154 | 0 |
| Branch stub | 388 | 512 | 0 |
| Header with branch | 77 | 163 | 0 |

The high refusal counts are deliberate: equal-diameter, tangent, grazing and near-cylindrical cases are not implemented and are refused rather than guessed.

### Defect found by the campaign

On its first run, 143 cone cases failed the output-integrity gate. Cause: the DXF writer rounded numbers to nine decimals; for large apex radii the rounded arc angle moved the arc's end points by more than the re-import tolerance. The geometry was right; the file was not exact. Fix: numbers are written in exact round-trip form. Four of the failing cases are kept in `crates/nf-core/tests/regressions/001-dxf-arc-angle-precision.json`.

## 2. Mutation campaign

Tests the verifiers themselves: for random valid parts, one input is changed by 0.1 % to 20 % before development, and the result is verified against the true inputs. Run at scale 3: **2,682 mutations caught, 0 missed** (18 skipped because the mutated part was out of range). Covers right cones, eccentric reducers and branch stubs.

## 3. Regression corpus

`crates/nf-core/tests/regressions/*.json` holds every case that once exposed a defect, with its expected outcome. The corpus runs with every `cargo test`. A defect that returns fails the build. This is how the *engine* does not make the same mistake twice — through a recorded case and a code fix made by a person, not by self-modification.

## 4. Lessons at run time

| Event | Lesson recorded |
|---|---|
| An engineer corrects a parameter | `corrected_value`: family, parameter, the wrong value, the right value, the reason |
| Verification rejects a produced pattern | `engine_failure`: the exact request |
| A stored result fails its replay / integrity check | `integrity_failure`: the exact request |

Every new job in the same project is compared with the project's lessons. A match raises an **advisory** on the job — for example, "header_radius = 150 repeats a value that was corrected in this project (150 → 142; reason: inside diameter)". Gate G20 `known_mistakes` then keeps the job in REVIEW REQUIRED until an engineer acknowledges the advisory with a recorded reason.

Rules, consistent with the earlier decisions on tenant knowledge (ADR-016, interim rule):

- A lesson **never changes a value** and never approves anything. It asks a person to look.
- Lessons are scoped to one project and visible only to its members.
- Lessons and acknowledgements are immutable records with author and time.
- Matching is exact: the same parameter with the same wrong value, or the identical request. It does not generalise ("all diameters on this customer's drawings are inside") — that needs reviewed rules.

### Limits

Lessons cover only mistakes already seen in that project · no learning across projects or customers (permission and legal questions open) · no statistical or model-based learning · a lesson can itself be wrong (the "correction" was the mistake): it can be acknowledged past, not yet retired · engine-failure lessons need a developer to fix the code; the system only stops the same request from passing unnoticed.

## Running

```
cargo test --release -p nf-core --test stress -- --nocapture          # default size
NF_STRESS_SCALE=6 cargo test --release -p nf-core --test stress -- --nocapture
```
