# Release Gates Addendum

Version 1.0 · 18 November 2026 · **Adopted by owner decision (D-28, D-29, D-30).** To be incorporated into Spec 02 §10 at its next revision; Spec 02 itself is not edited here.

## Gate A — Obligation closure (G18, critical)

`PASS` iff no **mandatory** scrutiny obligation — downstream, lateral or common-cause ([Dependency Model](DEPENDENCY_MODEL.md) §4) — whose target is in the release closure of the part is in state `open`.

- An obligation is resolved only by `discharged` (evidence from every required check) or `waived` (an engineer's Decision with reason, shown at approval), or by `superseded` when its target record is itself replaced and the successor carries the obligation.
- **Budget exhaustion, a low-yield stop, a timeout or a strategy change can never close an obligation.**
- Outcome when not PASS: `REVIEW_REQUIRED` with the list of open obligations and their origin findings. If a required check has **failed** (the interface does not fit, the recomputed result does not verify): `REJECTED` for that record set.
- Not resolvable by Decision except through the per-obligation waiver above; waivers of obligations arising from verification failure are not permitted.

## Gate B — Self-consistency (G19, critical)

`PASS` iff every mandatory self-consistency audit on the release closure has passed ([Investigation Model](INVESTIGATION_MODEL.md) §2):

| Audit | Requirement at release |
|---|---|
| A1 Recompute-from-scratch | All derived conclusions in the closure (statuses reached by rule, obligation set, coverage states in scope, gate results, the closure itself) match independent recomputation |
| A2 Invariance | Required invariance tests for the release-critical calculations pass |
| A3 Verifier cross-examination | No unresolved primary/verifier disagreement in the closure |
| A4 Assumption ledger | Every entry in the closure cleared by a cited rule or accepted by Decision |
| A5 Deterministic replay | See below |

- Outcome when an audit **fails**: `REJECTED` for the affected record set, quarantine of the implicated conclusion or component, and an investigation. When an audit is **inconclusive or not yet run**: `REVIEW_REQUIRED`.
- Not resolvable by Decision.

## Replay audit (A5) — before every manufacturing release

1. Runs on an **independent execution instance**: separate process space and working storage from the instance that produced the release, started from the Snapshot, the configuration hash and the recorded human-input sequence only. (Whether a separate machine or separately built binaries are also required is a Specification 05/07 decision.)
2. **Reconstructs the approved release closure** and compares its hash.
3. **Reproduces the required deterministic calculations** in the closure.
4. **Compares canonical outputs and verification evidence** — DXF and other release files in canonical form, gate evaluation, coverage statement — at the declared reproducibility level (Spec 02 §7.5).
5. **Reports every discrepancy** as a Finding with both values; it never repairs, overwrites or selects between them.

A replay audit shows that the result is *reproducible*. It does not show that it is *correct*: it does **not replace independent engineering verification** (Gate G10), which uses different methods, nor engineer approval (G16).

## Resulting gate list (for the next Spec 02 revision)

G01–G15 as in Spec 02 §10.2 · G16 human approval, redefined over the release-closure hash ([Determinism and Approval](DETERMINISM_AND_APPROVAL.md) §4.2) · G17 coverage — **deferred** · **G18 obligation closure — adopted** · **G19 self-consistency — adopted**.

`APPROVED` is reachable only when G01–G16, G18 and G19 all PASS.
