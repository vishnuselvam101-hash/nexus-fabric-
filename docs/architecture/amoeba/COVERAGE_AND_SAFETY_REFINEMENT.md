# Coverage and Safety Refinements

Version 0.1 (refinements D and E) · 18 November 2026 · Proposed design under ADR-011 and ADR-013 conditions · Responds to review findings W6–W9, W11

The original [Coverage Model](COVERAGE_MODEL.md) and the safety table in the AMOEBA architecture §7 are preserved unchanged; this note records the proposed corrections.

---

## Part D — Coverage

### D1. Three different statements (never conflated)

| Statement | Nature | Who can make it |
|---|---|---|
| **Accounting closure** — every enumerated unit has exactly one state | Verifiable property of the ledger | System (audited by A1) |
| **Examination coverage** — counts and ink/area shares per state, per domain, at a stated enumeration level and allowance | Measurement | System |
| **Completeness** — "the documents contain nothing else relevant to this part" | A claim about the world; depends on what the scan captured, on documents never supplied, on marks below detection | **Not made by the system.** Replaced by a printed *enumeration-limits* statement; an engineer may assert completeness as a Decision |

Proposed wording change: the phrase "fully inspected" is withdrawn. The strongest system statement is: *"All enumerated content accounted for (enumeration level L, ink allowance ε, EOS version v), subject to the stated limits."*

### D2. States

The owner's five states are kept. Proposed addition (decision pending): a distinct **`excluded_by_decision`** state, so that content a person declared irrelevant is counted separately from content the system explained. Until decided, exclusions are reported as a sub-count of `fully_examined`.

"Explained" has one definition: linked to at least one record that is **current and accepted** on the validation axis ([Consolidated Record Model](CONSOLIDATED_RECORD_MODEL.md) §3) — a current but unaccepted candidate explains nothing.

An **applicability table** (versioned) states, per element type, which processes must complete for it to be examined; "all applicable processes completed" refers to that table.

### D3. Raster enumeration (replaces the single ink-component denominator)

| Level | Unit | Always available? | Purpose |
|---|---|---|---|
| **R0 Area** | Fixed grid tiles covering the whole page | Yes | Guarantees every part of the page is accounted for; cheap; never a "fallback" |
| **R1 Ink** | Ink pixels per tile, at the standard threshold, on the unfiltered image | Yes | Measurement of how much ink is explained |
| **R2 Structure** | Skeleton segments between junctions (linework) and compact components (glyph-like) | Where segmentation is stable | Unit-level accounting; avoids "one giant component" when lines connect everything |
| **R3 Faint** | Additional ink found at a raised-sensitivity threshold | Yes | Measurement of marks the standard pass ignores |

- Every accepted record that explains raster content carries a **pixel attribution mask** (source coordinates). Per tile: `unexplained_ink = ink − attributed ink`.
- Tile state: `fully_examined` iff unexplained ink ≤ allowance ε (declared, per scan-quality class, from the benchmark — not assumed), R3 faint ink ≤ its allowance, and no open Finding; otherwise `partially_examined` with reason (`unexplained_ink`, `faint_content`, `touching_content`), or `unsupported` / `ambiguous` / `unexamined`.
- Tiles with faint content above allowance are routed to a person ("look here"), because the system cannot interpret what it barely detects.
- Reporting is at the finest level achieved (R2 where stable, else R1 per tile) and **states that level**. Tile-level accounting is a valid, lower-resolution measurement — it does not make a coverage result "inconclusive" by itself.

Stated limits (printed with every coverage statement): marks not captured by the scanner; content lost to compression before upload; content hidden under stamps or overlapping strokes; documents not supplied; D6 references that depend on recognition; the allowance ε itself.

### D4. Vector and CAD content

Unrecognised geometry (paths/entities with no accepted role) is `partially_examined: unexplained` — it must be given a role or excluded by a person. Hidden/masked/zero-width/off-page content is enumerated and must be explained or flagged. Unsupported entity kinds remain `unsupported`; where they intersect a part's views they block that part.

### D5. Gate

Gate G17 stays **deferred** (ADR-011). Candidate definition for later decision: PASS iff, within P's scope, no unit is `unexamined`, `unsupported` or `ambiguous`, and every `partially_examined` unit is cleared by a person (explained or excluded); FAIL otherwise. There is no INCONCLUSIVE outcome caused by enumeration level; the level is reported. Decision depends on measured enumeration cost and reviewer workload (benchmark).

---

## Part E — Safety loopholes

### S-1 Experimental work influencing production scheduling

- Non-production activities (experimental and candidate lanes) write to a **lane overlay**, not to the production record set. Production activities and the production scheduler cannot read overlays; overlay signals are not delivered to production.
- `eos_version` covers production data only; overlays have their own identity.
- **Taint rule:** a record is releasable only if its entire release closure consists of production-lane records produced by production-qualified versions. Checked by gate G14 and by audit A1.
- Comparison of overlay results with production results (shadow evaluation) is how candidates are assessed — by people, through the qualification procedure.

### S-2 Runtime trust boundaries

The runtime is decomposed and given explicit permissions (new zone label **ZR**):

| Component | Trust | May | May not |
|---|---|---|---|
| **Scheduler** | Deterministic pure function; qualified like an algorithm; verified by replay (A5) | Emit the work set and budget grants | Write any engineering, observation, evidence or human record |
| **Executor / supervisor** | Operational; untrusted for content | Start/stop sandboxes, enforce safety aborts, deliver frozen views | Alter activity outputs; write records other than operational log entries |
| **Commit gate + append store** | Trusted computing base, kept minimal | Validate schema, zone permissions, lane and tenant; append | Accept a record whose producer is not permitted its type |
| **Invariant monitor / auditor** | Z3; independent implementation | Raise Findings and quarantines | Repair |

A faulty or compromised scheduler can waste budget or omit discretionary work; it cannot fabricate evidence, change authority, or satisfy a gate. Omitted **mandatory** work is detected because mandatory work is recomputed independently by the auditor from the applicability table and the obligation set.

### S-3 Recogniser confidence

One rule replaces the earlier, contradictory pair:

- Confidence is **not an input** to: reading/validation status, independent-evidence thresholds, obligations, mandatory scheduling, stopping rules, gates, coverage states.
- Confidence **may be an input only to discretionary priority**, and only monotonically: *low* confidence can add priority; *high* confidence adds nothing and can never lower an item below its confidence-free priority.
- Because discretionary budget is finite, raising one item displaces another discretionary item. This is inherent and accepted; therefore **nothing that safety relies on may be discretionary**.
- Test (replaces the earlier confidence-isolation test): with arbitrary confidence values injected, the mandatory work set, all statuses, thresholds, gates and coverage are identical; only the composition of discretionary work may differ.

### S-4 Incomplete hypothesis searches

- A hypothesis set or investigation carries `search.complete`. Complete means: produced by an exhaustive method over a declared finite space (e.g. interval branch-and-bound over a module's parameter box; full enumeration of discrete branches), with that method recorded.
- A single survivor of an **incomplete** search is `provisional_sole`. For gates it counts as *ambiguous* until either a complete search confirms it or an engineer accepts it by Decision. It is entered in the assumption ledger.
- (Proposed amendment to Spec 03 §5.5 and Spec 02 MATH-12 — not applied.)

### S-5 Promotion of generated capabilities

| Artefact kind | May be machine-proposed? | Promotion |
|---|---|---|
| Title-block templates, sheet-furniture exclusion patterns, vocabulary extensions, line-style mappings | Yes | Experimental → candidate → production, each step by signed human decision with a qualification record; author and approver distinct |
| Strategy-profile parameters, discrimination tables, cost-function calibrations | Yes, from benchmark/development data only | Same; and frozen per protocol version |
| Regression cases | Yes | Added on confirmation of the underlying defect |
| Recogniser weights (trained/tuned) | Yes, under tenant permission | Same, plus safety-metric thresholds |
| Explanation catalogues, dependency rule tables, applicability tables, acceptance rules | **No** — human-authored only | Qualified change |
| Mathematical algorithms, verification methods, gate definitions, schemas, zone permissions | **No** | Engineering change process with independent review |

Nothing changes lane within a job. No promotion is automatic, time-based or threshold-triggered.

### S-6 Residual safety questions

Model checking of the lane/overlay and release state machine; size of the trusted computing base in practice; protection of rule tables and catalogues as controlled configuration (integrity, review, signing); insider risk in exclusion decisions (bulk exclusion to obtain a clean coverage statement) — exclusion counts are always printed and bulk actions are restricted.
