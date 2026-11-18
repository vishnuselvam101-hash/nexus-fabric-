# Evidence Levels for Engineering Knowledge

Version 1.0 · 18 November 2026 · Adopted by owner decision D-39

## Purpose

Every engineering relationship, rule, model and parameter in NEXUS FABRIC carries a statement of **what kind of evidence supports it**. The five levels are different *kinds* of evidence. They are **not** a single ladder of approval, they are not interchangeable, and holding one does not imply another.

| Level | Name | Meaning | Minimum record required | What it does **not** establish |
|---|---|---|---|---|
| **EV-1** | AI-generated proposal | Written by an AI system (or by anyone) from general principles, without review | Author, date, document | Anything about real fabrication practice |
| **EV-2** | Engineer-reviewed relationship | A named, qualified engineer examined the statement and classified it, with reasoning | Reviewer name, qualifications, date, classification, reasoning; the returned review document | That it holds in a particular factory, or that it has been observed or tested |
| **EV-3** | Factory-observed relationship | Seen in a specific factory's actual practice, drawings, procedures or historical jobs | Factory, source (document, job, procedure, interview), date, observer, scope | That it is universal, or that it is physically correct beyond that practice |
| **EV-4** | Experimentally validated relationship | Tested by a defined experiment or trial with measured results (benchmark, physical trial, measured part) | Protocol, data, result, measurement uncertainty, scope of validity | Validity outside the tested range |
| **EV-5** | Calibrated manufacturing parameter | A numerical parameter determined by calibration for stated material, thickness, machine, tooling and date | Calibration procedure, data, value with uncertainty, validity range, owner | Applicability to any other material, machine or condition |

## Rules

1. **Each claim lists every level it holds, with the records.** "EV-2 + EV-3 (Factory A)" is a normal state; "validated" without a level is not a permitted description.
2. **Levels do not substitute for each other.** An engineer's review (EV-2) does not make a parameter calibrated (EV-5). A factory's habit (EV-3) is not an experimental result (EV-4). A calibration (EV-5) says nothing about whether a dependency rule is right.
3. **EV-5 applies only to parameters**; EV-2 to EV-4 apply to relationships, rules and models.
4. **Scope is part of the evidence.** EV-2 classifications *universal / process-specific / factory-specific / needs calibration* and the factory, process and range named in EV-3 to EV-5 limit where the claim may be used.
5. **Disagreement is kept.** Two reviewers or two factories may classify the same statement differently; both records stand.
6. **No level may be recorded without the evidence document filed in the repository** (returned review, factory record, experiment report, calibration record). An AI system cannot raise a level by itself.
7. **Evidence can be withdrawn** (reviewer retracts, trial invalidated); the withdrawal is recorded and the level reverts.
8. Relation to software lanes ([ADR-013](ADR-013-lanes-quarantine-retirement.md)): lanes describe the qualification state of *software artefacts*; evidence levels describe the support for *engineering knowledge*. A production-lane rule table whose rules are only EV-1 would be a contradiction — what minimum evidence each kind of rule needs before production use is a Specification 05 decision.

## Current inventory (18 November 2026)

| Item | Evidence |
|---|---|
| All 56 statements in the fabrication review package | EV-1 |
| Dependency edge types, interface constraints, propagation rule table | EV-1 |
| Spec 02 geometry-family models | EV-1 (author-checked derivations and author-computed reference values only) |
| Process and allowance models | EV-1; **no** EV-5 parameter exists |
| Japanese drawing vocabulary | EV-1 |
| Everything else engineering-related in the repository | EV-1 |
