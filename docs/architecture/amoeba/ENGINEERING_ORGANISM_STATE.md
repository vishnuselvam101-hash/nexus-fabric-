# Engineering Organism State (EOS)

Version 0.1 · 18 November 2026 · Supporting note to the [AMOEBA architecture](../../specifications/AMOEBA_NATIVE_SYSTEM_ARCHITECTURE.md) §3. Proposed [P]; extends the Engineering Truth Graph of Spec 01 §6 without removing anything from it.

## 1. Definition

`EOS = (Snapshot, Records, RegionTree, Issues, CoverageLedger, ProcessLedger, SignalLog, Heads)`

`eos_version = H(snapshot_root, records_root, issues_root, coverage_root, process_root, heads)` — a hash; two EOS states with the same version are identical. `SignalLog` is derived from the other components and is not hashed separately.

## 2. Components

### Snapshot
- Hash tree over all SourceArtifacts in the project at engulfment, with: declared role per file (assembly, detail, reference, specification, model), tenant, automated-reading policy level, uploader, time.
- Immutable. New or replaced files ⇒ new Snapshot with `predecessor`; the difference drives revision handling (Spec 03 §8.3).
- Also stores the **enumeration**: the countable source elements that form the coverage denominators (see Coverage Model).

### Records
All immutable, content-addressed records. Families:

| Family | Record types | Defined in |
|---|---|---|
| Source | SourceArtifact, ingestion outcome, transform chain | Spec 01 §6.1, Spec 03 §1–2 |
| Observation | Dimension, Tolerance, TextNote, TitleField, GeometryEntity, LineRole, Symbol, ViewRegion, ViewRelation, Scale, ProjectionMethod, Association, Material, Thickness, Designation, RevisionMark, DocumentRelation, UnitDeclaration, Hypothesis | Spec 03 §6 + schema note |
| Human | Validation, Correction, Decision, Sign-off, Approval | Spec 01, Spec 03 §9 |
| Engineering | Constraint, Parameter, Geometry (reconstructed objects), Calculation, ProcessData, Strategy (plate division) | Spec 01 §6.1, Spec 02 |
| Evidence | Evidence (verification), gate evaluation, coverage statement, qualification record | Spec 02 §9–10, §12 |
| Output | DXF/STEP/report, entity→geometry map, package manifest | Spec 01 §6.1 |
| Lifecycle | Supersession, Quarantine, Tombstone, Lane promotion | this note |

Every record carries: id (hash), tenant, type, payload, `derived_from` edges, producing process id, and the provenance axes (§4).

### RegionTree
Two containment hierarchies sharing leaf references:

- **Source side:** project → document → sheet/page → region (frame, title block, revision table, notes, parts list, view) → element (text group, glyph run, entity, symbol, ink component).
- **Engineering side:** project → assembly → part → surface / feature / joint → pattern → profile → entity. (Outlined only; completed in Spec 04.)

Tree edges express *containment*. Everything else — a dimension measuring an entity, a section label pointing to another sheet, a constraint over two views, a pattern derived from a surface — is a **cross-reference edge** between records. Decomposition follows the tree; correctness depends on the cross-references, which is why views carry boundary references (§3).

### Issues
`Issue { id, kind: conflict | gap | ambiguity | question | quarantine | unsupported, rule/reason code, records involved, affected regions, state: open | resolved(by record) | accepted_risk(by Decision), history }`. Issues are never deleted; resolution is by reference to the resolving record.

### CoverageLedger
Per element of the enumeration: current coverage state, the processes that examined it, the records that explain it, open issues. See the Coverage Model.

### ProcessLedger
`ProcessInstance { id, type, version, lane, zone, inputs (record ids / region), trigger (signals), budget granted/used, outcome: completed | budget_stopped | terminated(reason) | failed(reason), outputs }`.

### SignalLog
Ordered sequence of typed signals, each `{type, subject record/region ids, cause process}`. Derivable by replay from the other components.

### Heads
Current-revision pointers per document and part; job state; current release state per part. The only mutable data; every change is itself logged.

## 3. Views and boundary references

A process is given `View(region R, EOS version v)`:

- **Interior:** records contained in R.
- **Boundary references:** records outside R that are connected to interior records by any cross-reference edge, delivered read-only with their identity and provenance.
- **Obligations:** the list of boundary references the process must either resolve (by producing a relation record) or report as unresolved.

Rules: a process may append records about its interior and relations across its boundary; it may not append records about the interior of another region. Unresolved boundary references keep R at *partially examined*. Merging is trivial (append-only union) because nothing is copied or mutated.

## 4. Provenance axes (orthogonal; formal mapping)

Review finding: origin classes, reading states and authority describe different things and must not be collapsed. Each record carries **four independent attributes**:

| Axis | Question | Values |
|---|---|---|
| **Origin** | By what means did this enter the EOS? | written_exact · vector_geometry · ocr · raster_geometry · cross_view · inferred_candidate · human_entered · manufacturing_assumption |
| **Authority** | What standing does the *content* have as a statement about the part? | written · measured · inferred · assumed (Spec 01 ARCH-14) |
| **Reading state** | How well established is it that we read the content correctly? | n/a · unvalidated · corroborated · confirmed · corrected · rejected |
| **Lifecycle status** | Is this record current? | current · superseded · quarantined |

Derived property used by the UEM (never stored, always computed):

```
usable_as_H(record) :=  authority = written
                     ∧ reading_state ∈ {confirmed, corrected}            # or corroborated where Spec 05 allows
                     ∧ lifecycle = current
                     ∧ 'reference' ∉ interpretation flags
                     ∧ no open Issue of kind conflict|ambiguity involves it
usable_as_S(record) :=  authority = measured ∧ lifecycle = current ∧ not excluded by flags (across_break, not_to_scale)
```

Allowed origin→authority pairs are those of the Spec 03 schema note §2. Reading state applies only where a reading exists (origins ocr, written_exact, human_entered). Proposed as an amendment to Spec 01 §6.1 and Spec 03 §6 (migration note A-01, A-03-4).

## 5. Invariants (checked continuously; violation ⇒ quarantine)

1. Every record's `derived_from` targets exist in the same tenant.
2. Every non-source record has a producing ProcessInstance whose zone permits that record type.
3. No record of authority *written* has a model-based process as its only producer of the value (models produce candidates; value records come from exact extraction, recognition with alternatives, or people).
4. Every enumeration element has exactly one coverage state.
5. Every Approval references an EOS version at which all required gates evaluate PASS.
6. Supersession chains are linear; a superseded class-H record has an origin-7 successor or a new-revision successor.
7. Lane: no record produced by a non-production process appears in the evidence of an Approval.
8. Nothing in Snapshot or Records is absent that was present in a predecessor EOS version (append-only), except through the audited retention operation.

## 6. Open points

Canonical serialisation for hashing (ADR-006); storage and query performance of views; engineering-side RegionTree; whether SignalLog must be persisted or can always be re-derived; representation of large raster enumerations.
