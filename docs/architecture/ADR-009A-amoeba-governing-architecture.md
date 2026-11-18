# ADR-009A: AMOEBA as the governing architecture

**Status: Accepted in principle** · 18 November 2026 · Decided by the project owner; recorded in the [Decision Register](DECISION_REGISTER.md). Background: [review](../reviews/AMOEBA_INDEPENDENT_ARCHITECTURE_REVIEW.md) §11, [original proposal](amoeba/PROPOSED_ADRS.md).

## Decision
AMOEBA is the primary architecture of NEXUS FABRIC. The complete engineering lifecycle is organised as AMOEBA processes over one Engineering Organism State, under the trust zones, with examination accounting, lanes and quarantine. **This decision is not conditional on benchmark results.**

## What "in principle" leaves open
The detailed designs in the refinement notes (dependency model, investigation model, determinism and approval, coverage and safety refinements, consolidated record model, process catalogue) are proposals under review. Accepting AMOEBA's identity does not accept each of them.

## Consequences
Specification 04 and later specifications are written natively against the EOS and the process model. The stage pipeline of Specs 01–03 is retained only as a typical path. Trust zones, deterministic calculation authority, independent verification and engineer approval are unchanged and take precedence over any AMOEBA mechanism (AMB-02).

## Not decided here
Whether adaptive investigation is effective — see ADR-009B.
