# Consolidated Record Model

Version 0.1 (refinement F) · 18 November 2026 · Proposed design under ADR-010 conditions · Responds to review §10 (complexity and duplication)

Proposed [P]. Supersedes nothing yet: Specs 01–03 and the earlier EOS note keep their record lists until the amendments are decided.

## 1. Overlaps found

| Overlap | Where | Problem |
|---|---|---|
| `EngineeringOutcome` (UEM return) · stage outcomes · Issues · ObservationSet `conflicts/gaps/unsupported` · review reason codes | Spec 02 §1.2, Spec 01 ARCH-05, EOS note, Spec 03 §10 and schema §5 | Five vocabularies for "something is wrong or missing here" |
| ProcessInstance (ProcessLedger) · Calculation record · probe outcome | EOS note, Spec 01 §6.1, Mechanisms | Three records for "an activity ran with these inputs and outputs" |
| Evidence · gate evaluation · coverage statement · audit result · qualification record | Spec 02, Coverage model, Investigation model | Machine assessments with different shapes |
| Validation entry · Correction · Decision · Sign-off · Approval | Spec 03 §9, Spec 01 | Human acts with different shapes |
| ObservationSet · EOS view | Spec 03 §10, EOS note §3 | Two hand-over objects |
| Hypothesis as an observation type · Investigation hypotheses · interpretation branches | Spec 03 schema, Investigation model | Same concept twice |
| `derived_from` · relations on observations · cross-reference edges · dependency edges | Spec 01, Spec 03, EOS note, Dependency model | Four edge mechanisms |
| Supersession · Quarantine · Tombstone · lifecycle status | EOS note | Lifecycle expressed as both records and a status field |
| Observation `status` (Spec 01) · `reading_state` (Spec 03) · "accepted" | Specs 01, 03 | One word for different things |

## 2. Seven record families

Every record belongs to exactly one family. (The shape is close to established provenance models — entities, activities, agents — which is deliberate.)

| Family | What it is | Absorbs |
|---|---|---|
| **Source** | Bytes that came from outside, and their enumeration | SourceArtifact, Snapshot, enumeration units, transform chains |
| **Claim** | A statement about the drawing or about the part: an observation, a parameter value, a geometry, a pattern, a profile, a material, an assumption, a ProcessData value, an output file | All observation types; Constraint, Parameter, Geometry, Pattern, Piece, Profile, DxfEntity; Material, ProcessData; Hypothesis *content* (a hypothesis is a bundle of Claims with validation status `candidate`) |
| **Relation** | A typed edge between records | `derived_from`, observation relations, cross-references, all dependency edge types, Interface and Seam constraints |
| **Activity** | Something that ran: type, version, lane, zone, inputs, outputs, logical cost, outcome | ProcessInstance, Calculation, probe, test, audit run, propagation run |
| **Finding** | Something wrong, missing, ambiguous, unsupported, suspicious or owed | EngineeringOutcome (when emitted from an Activity), conflicts, gaps, ambiguities, unsupported items, quarantines, obligations, investigations (as a Finding with a dossier), audit discrepancies; review reason codes become Finding kinds |
| **Assessment** | A machine evaluation of other records | Verification evidence, gate evaluation, coverage statement, audit result, qualification record, residual-uncertainty statement, release closure |
| **Attestation** | An act by an identified person | Reading confirmation, correction, interpretation acceptance, exclusion, Decision, waiver, sign-off, approval, lane promotion, quarantine lift |

Rules: `EngineeringOutcome` remains the UEM's *return type* (a library must not write records); the calling Activity converts a non-`Ok` outcome into a Finding — one mapping table, one vocabulary downstream. The ObservationSet becomes a named, signed **view** (an Assessment listing record ids + an Attestation), not a separate container. Lifecycle changes are expressed only as Relation/Attestation records (`supersedes`, `quarantines`, `retires`); "lifecycle status" is derived.

## 3. Five orthogonal axes on a Claim

Stored attributes are immutable facts about how the claim arose; statuses that change are **derived** from other records.

| Axis | Question | Values | Stored or derived |
|---|---|---|---|
| **Provenance** (origin) | By what means did this enter the state? | written_exact · vector_geometry · ocr · raster_geometry · cross_view · inferred_candidate · human_entered · manufacturing_assumption · computed (by a Z2 activity) | Stored |
| **Authority** | What standing does the *content* have as a statement about the part? | written · measured · inferred · assumed · calculated | Stored |
| **Reading status** | Is it established that we read the source correctly? (only where a reading exists) | n/a · unvalidated · corroborated · confirmed · corrected · rejected | Derived from Assessments (corroboration) and Attestations |
| **Validation status** | Has the *content or interpretation* been accepted for use? (association, role, hypothesis, interpretation of notation, a calculated value's verification) | candidate · rule_accepted · human_accepted · verified · refuted | Derived from Assessments and Attestations |
| **Lifecycle status** | Is this record in force? | current · superseded · quarantined · retired | Derived from Relations/Attestations |

Reading status and validation status are different: a dimension can be read correctly (`confirmed`) while its association to geometry is still a `candidate`; a calculated pattern has no reading status at all but has a validation status (`verified` once its required evidence passes).

Two values added relative to the earlier four-axis proposal — provenance `computed` and authority `calculated` — so that engineering results sit in the same model as observations (needed for Spec 04).

Derived predicates (computed, audited, never stored):

```
usable_as_H(c)  := authority(c) = written ∧ reading(c) ∈ {confirmed, corrected}      # corroborated only if Spec 05 permits
                 ∧ validation(c) ∈ {rule_accepted, human_accepted} for every interpretation it relies on
                 ∧ lifecycle(c) = current ∧ no open Finding involves c
usable_as_S(c)  := authority(c) = measured ∧ lifecycle(c) = current ∧ not excluded by interpretation flags
releasable(c)   := authority(c) = calculated ∧ validation(c) = verified ∧ lifecycle(c) = current
                 ∧ lane closure is production ∧ no open Finding/mandatory obligation in its closure
explains(c, u)  := lifecycle(c) = current ∧ validation(c) ∈ {rule_accepted, human_accepted, verified} ∧ u ∈ attribution(c)
```

## 4. What is *not* a stored status

Coverage state (derived from `explains`, applicability table and Findings) · release state (derived from gate Assessments and Approval validity) · "accepted" as a bare word (always one of the axes above).

## 5. Mapping from the existing documents

| Existing | Becomes |
|---|---|
| Spec 01 Observation.status proposed/accepted/rejected/superseded | Reading status + validation status + lifecycle (derived) |
| Spec 03 `origin` (8) | Provenance (+ `computed`) |
| Spec 03 `authority` (4) | Authority (+ `calculated`) |
| Spec 03 `reading_state` | Reading status |
| Spec 03 flags `reference`, `not_to_scale`, … | Interpretation Claims with their own validation status (per review amendment A-03-3) |
| EOS note lifecycle status | Lifecycle (derived) |
| Spec 02 Quantity envelope | Payload of a `calculated` Claim; `assumptions` become `assumes` Relations |

## 6. Open questions

Whether deriving statuses on demand is fast enough at scale (materialised views are permitted as reclaimable caches, audited by A1); migration of requirement IDs; final naming; whether Interface should be a Claim (a stated requirement) plus Relations rather than a Relation only — to settle in Spec 04.
