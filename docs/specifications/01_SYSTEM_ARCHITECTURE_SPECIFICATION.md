# 01 — System Architecture Specification

Version 0.1 (draft for review) · 18 November 2026 · Parent: [Master Plan](../MASTER_PLAN.md)

**Reading conventions.** Every decision carries a status: **Confirmed** (stated in the master plan as a governing principle), **Proposed** (recommended here, awaiting evidence or review), or **Open**. Nothing in this document is a measured capability. "Must" marks a requirement; requirement IDs (`ARCH-nn`) are referenced by later specifications and tests. Detailed rationale lives in the [ADRs](../architecture/).

---

## 1. Product architecture

### 1.1 Purpose

Convert PDFs, scans, images and supported CAD files into mathematically verified, manufacturing-ready DXF, with provenance from every output entity back to its source and to the engineering decisions that shaped it. Target: zero known unresolved critical errors at release, under explicit tolerance and uncertainty control. Absolute physical zero error is not claimed.

### 1.2 Trust zones

The architecture is organised around who is allowed to assert what.

| Zone | Contents | May assert | May not |
|---|---|---|---|
| **Z0 Untrusted input** | Customer files | Nothing | Be parsed outside a sandbox |
| **Z1 Interpretation** (non-authoritative) | OCR, vision, automated drawing reading, heuristics | *Proposals*: observations with provenance class and uncertainty | Write geometry, parameters or DXF coordinates |
| **Z2 Deterministic engineering core** (authoritative for calculation) | UEM, reconstruction, fabrication development, DXF generation | Geometry and calculations, reproducibly | Approve its own output |
| **Z3 Independent verification** (authoritative for evidence) | Verification engine, DXF re-import validation | Pass / fail / inconclusive evidence | Modify the thing it verifies; share calculation code with Z2 for the checked quantity |
| **Z4 Human authority** | Authorised engineers | Engineering decisions, release approval | Bypass a failed required gate (overrides are recorded decisions with scope and reason, and only where policy allows) |

- **ARCH-01** Data crosses Z1→Z2 only as typed observation records. No free-form automated-reading output is consumed by Z2.
- **ARCH-02** Every coordinate in a released DXF must be the output of a Z2 calculation whose inputs trace to accepted observations or recorded Z4 decisions.
- **ARCH-03** A Z3 check for a quantity must not call the Z2 implementation that produced it (see §3.4).

### 1.3 Subsystems and data flow

```
 Z0            Z1                         Z2                                 Z3              Z4
┌──────┐  ┌───────────────┐   ┌──────────────────────────────────┐   ┌──────────────┐  ┌──────────┐
│Ingest│→ │Vector extract │→  │Reconstruct → UEM → Fabrication → │→  │Verification  │→ │Review &  │
│      │  │Image process  │   │                    DXF generate  │   │DXF re-import │  │approval  │
│      │  │OCR / interpret│   └──────────────────────────────────┘   └──────────────┘  └──────────┘
└──────┘  └───────────────┘
     └──────────── all stages read/write the Engineering Truth Graph (§6) ────────────┘
     Cross-cutting: job orchestration · project/revision management · identity & permissions · audit
```

Stage outputs (each is an immutable, content-addressed record set):

1. **Ingest** → source artifact (bytes, hash, format, sanitised rendering).
2. **Extract / interpret** → observations: text, dimensions, symbols, linework, views, each with provenance class (*written / measured / inferred / assumed*), source region and uncertainty.
3. **Reconcile** (engineer-in-the-loop where needed) → accepted observations, flagged conflicts, open questions.
4. **Reconstruct** → constraint system, solved parameters, residuals, ambiguity report, candidate 3D geometry.
5. **Develop** → development class (*exact / piecewise / approximate with quantified distortion / not developable*), flat patterns, plate divisions, seams, allowances, alternative strategies.
6. **Generate** → DXF plus the mapping from each DXF entity to its originating geometry.
7. **Verify** → evidence records per release gate.
8. **Release** → status *Approved / Review Required / Rejected*, approval identity, immutable package.

### 1.4 External integrations

Inbound: file upload, watched folders (on-premises), future ERP/job systems. Outbound: DXF/STEP/PDF package, CAM handoff via per-customer layer and format profiles, production feedback (measurements) returning as new observations. External reading services are optional and governed by the customer automated-reading policy (§8).

## 2. Service architecture

"Service" means a component with a defined contract. Whether components run as separate processes or within one deployable is a deployment choice ([ADR-001](../architecture/ADR-001-application-architecture.md)); contracts are identical either way.

| Service | Zone | Responsibilities | Explicitly not responsible for |
|---|---|---|---|
| Drawing ingestion | Z0 | Accept files; hash; type detection by content; size/complexity limits; malware and structural sanitisation; create source artifact; route to pipeline | Interpreting content |
| PDF/vector extraction | Z1 | Parse PDF objects in a sandbox; paths, text, transforms, page geometry; classify vector vs raster pages | Deciding which lines are part geometry |
| Image processing | Z1 | Orientation, deskew, perspective, resolution/scale estimation, denoise, line/curve detection, vectorisation; record transformation chain and scan uncertainty | OCR semantics |
| OCR & drawing interpretation | Z1 | JA/EN text recognition; dimension, tolerance and symbol parsing; title block; view detection and classification; dimension-to-entity association; candidate interpretations with confidence | Resolving contradictions silently; filling in missing dimensions |
| Geometry reconstruction | Z2 | Build constraint system from accepted observations; solve; report residuals, degrees of freedom, conflicts; produce 2D/3D geometry; enumerate competing interpretations | Choosing between interpretations that remain consistent with all evidence (escalates to Z4) |
| Mathematical engineering engine (UEM) | Z2 | See §3 | Any I/O, any automated interpretation |
| Fabrication development | Z2 | Shape modules (parametric definition, applicability range, development method, verification method); development classification; plate division, seam and allowance computation; alternative strategies with cost terms | Asserting process parameters it has no calibrated data for |
| Verification | Z3 | Run gate checks via independent methods; uncertainty vs tolerance budget; fold-back/assembly rehearsal; produce evidence | Repairing failures |
| DXF generation & validation | Z2 + Z3 | Z2 writer: entities, layers, units, profile mapping. Z3 validator: independent parser re-import, topology, units, extents, comparison with approved geometry | CAM-specific post-processing beyond configured profiles |
| Project & revision management | — | Customers, projects, jobs, drawing revisions, diffing, impact analysis via the truth graph, package assembly | |
| Authentication & permissions | — | Identity, MFA, roles, per-customer isolation, release authority, signing of approvals | |
| Job orchestration | — | Durable job graph, idempotent steps, retries, checkpointing, resource classes, cancellation, progress | Business logic |

- **ARCH-04** Every service step is a pure function of (declared inputs, algorithm version, configuration) and records all three.
- **ARCH-05** A step that cannot complete confidently must emit an explicit *unsupported*, *ambiguous* or *insufficient-data* outcome; these are first-class results, not errors.

## 3. Universal Engineering Mathematics Engine (UEM)

A library (not a network service) with no I/O, no global state and no automated-reading dependency, consumed by reconstruction, fabrication and — through separate implementations — verification.

### 3.1 Components

| Component | Scope | Numeric regime |
|---|---|---|
| `uem-exact` | Predicates (orientation, incidence, intersection existence), rational constructions for linear geometry | Exact rational / filtered exact predicates |
| `uem-numeric` | Root finding, quadrature (arc length), linear algebra, special cases needing extended precision | f64 with stated error bounds; multi-precision on demand |
| `uem-interval` | Bounded enclosures of computed quantities | Outward-rounded interval arithmetic |
| `uem-constraint` | Constraint graph, DOF analysis, nonlinear solve, weighted least squares, residual and conditioning diagnostics | f64 + diagnostics; solutions re-checked in `uem-interval` |
| `uem-diffgeo` | Parametric surfaces, fundamental forms, Gaussian curvature, developability classification, geodesic/arc-length development | Analytic where closed form exists; numeric with bounds otherwise |
| `uem-compgeo` | 2D profiles: offsets, booleans, self-intersection, closure, nesting primitives | Exact predicates; construction error tracked |
| `uem-uncertainty` | First-order propagation (JΣJᵀ), Monte Carlo, interval propagation; tolerance budgets | Deterministic seeded RNG |
| `uem-process` | Bend allowance/deduction, thickness (neutral surface) compensation, kerf, springback — **parameterised by calibrated, versioned process data** | Model + validity range |
| `uem-kernel-port` | Abstract interface to the B-rep kernel (§4) | Kernel tolerance recorded |

### 3.2 Rules

- **ARCH-06** Every UEM result carries: value, unit, numeric regime, error bound or uncertainty, and applicability flag. A result outside a model's validity range is returned as *out-of-range*, never silently extrapolated.
- **ARCH-07** Units are typed. Dimensionless floats do not cross component boundaries for physical quantities.
- **ARCH-08** Topological decisions (inside/outside, closed/open, intersecting/not) use exact predicates or are reported as *undecidable at current precision*.
- **ARCH-09** A least-squares fit is never evidence of correctness by itself; `uem-constraint` must report residuals, redundancy, rank deficiency and alternative solutions.
- **ARCH-10** An approximate development must carry its quantified distortion and may never be labelled exact.
- **ARCH-11** Process models without customer/process calibration data produce results flagged *assumed* and force *Review Required*.

### 3.3 Determinism

Fixed evaluation order in parallel reductions; pinned compiler, flags and dependency versions; no fast-math; seeded RNG recorded per job; kernel version and tolerance recorded. Whether bit-identical results across CPU architectures are achievable with a C++ kernel in the loop is **Open** (benchmark required); the fallback requirement is equivalence within recorded bounds.

### 3.4 Independence of verification

For each checked quantity the Verification Specification (05) must name a method that differs from the generating method in at least one of: mathematical formulation (e.g. analytic development vs discrete fold-back of the pattern onto the target surface), implementation (separate code, separately reviewed), or library (e.g. DXF written by our writer, re-read by a third-party parser). Shared code is limited to audited primitives (`uem-exact`, units). The degree of independence achieved is itself recorded in the evidence. Independence reduces common-mode error; it does not eliminate it, and the specification must not claim otherwise.

## 4. CAD and geometry technology evaluation

No library is selected in this document. This section defines candidates, known facts and the evidence required. Licensing detail: [Dependency Licensing Review](../research/DEPENDENCY_LICENSING_REVIEW.md). Selection strategy: [ADR-003](../architecture/ADR-003-cad-kernel-selection-strategy.md).

### 4.1 Candidates (facts as of 2026-11-18; "TBV" = to be verified in the evaluation)

| Candidate | Role | Capabilities | Precision model | Rust integration | Exchange | Licence | Key concerns |
|---|---|---|---|---|---|---|---|
| **Open CASCADE (OCCT 8.x)** | B-rep kernel | B-rep, NURBS, booleans, shape healing, STEP/IGES | Tolerance-based floating point | C++; via FFI. Community binding `opencascade-rs` describes itself as "a major work in progress" | STEP/IGES native; DXF not in open core (TBV) | LGPL-2.1 + OCCT exception | Boolean robustness on degenerate input (TBV by benchmark); C++ exceptions/crashes need isolation; LGPL relinking duties; binding maturity |
| **CGAL 6.x** | Robust computational geometry | Exact kernels, arrangements, 2D booleans, offsets, meshes | Exact predicates; optional exact constructions | C++ header-heavy templates; needs hand-written C shim | None | Per package: kernel/number types LGPL-3+; most algorithms (arrangements, 2D booleans, offsets, Minkowski, PMP, AABB) **GPL-3+**; commercial via GeometryFactory | GPL packages incompatible with closed distribution unless commercially licensed; template FFI cost |
| **Commercial kernels** (Parasolid, ACIS, CGM) | B-rep kernel | Industry-standard robustness (vendor-claimed) | Tolerance-based | C/C++ FFI | STEP etc. usually add-ons | Proprietary; terms not public | Cost, royalties, on-prem redistribution terms — quotation needed |
| **Rust-native B-rep** (e.g. `truck`, Fornjot) | B-rep kernel | Partial (TBV) | f64 | Native | Limited (TBV) | Permissive (TBV) | Maturity for production booleans/STEP is unproven |
| **2D polygon libraries** (Clipper2, Boost.Geometry, Rust `geo`/`i_overlay`) | 2D profile ops as alternative to CGAL GPL packages | Booleans, offsets | Integer-snapped or f64 | Native or thin FFI | — | Permissive (TBV per library) | No arcs natively (most); robustness model differs — must be benchmarked against exact reference |
| **Number libraries** (GMP/MPFR, Rust `rug`, pure-Rust bigint/rational, interval crates) | Exact and interval arithmetic | — | Exact / arbitrary / interval | Native or FFI | — | GMP/MPFR LGPL; others TBV | LGPL on mobile/static builds; performance |
| **Own analytic layer** | Cylinders, cones, planes, their intersections and developments | Closed-form where it exists | Analytic + interval | Native | — | Ours | Scope must stay narrow; this is where our IP lives, not a general kernel |

### 4.2 Observations that shape the strategy

1. Phase-1 geometry (pipes, cones, reducers, branch intersections, planar plates) is dominated by quadrics and developable surfaces. Much of the *critical* mathematics can be analytic in Rust, with a B-rep kernel providing general modelling, STEP exchange and — usefully — an independent second opinion for verification.
2. The output is 2D profiles. Robust 2D operations (closure, self-intersection, offset for kerf) are safety-critical and are exactly where CGAL's best packages are GPL.
3. No candidate gives exact B-rep modelling. Kernel results must be treated as tolerance-bounded and checked.

### 4.3 Evaluation protocol (evidence required before selection)

For each candidate, on the same reference set (analytic cases with known closed-form answers + degenerate/near-degenerate cases + authorised factory geometry when available):

- Correctness against analytic references; behaviour at tangency, coincident faces, tiny edges.
- Failure mode: wrong answer vs explicit failure vs crash (crash and silent-wrong rates reported separately).
- Determinism across runs, thread counts and platforms.
- Time and memory per operation class.
- FFI effort and safety: exception containment, memory ownership, build reproducibility on Windows/Linux/macOS.
- STEP round-trip fidelity; DXF path.
- Maintenance signals: release cadence, issue response, bus factor.
- Licence obligations per deployment mode, reviewed by counsel.

## 5. Application architecture

Detail: [ADR-001](../architecture/ADR-001-application-architecture.md), [ADR-007](../architecture/ADR-007-desktop-mobile-architecture.md), [ADR-008](../architecture/ADR-008-cloud-vs-on-premises-deployment.md).

- **One engineering backend, several clients.** All authoritative calculation, verification and release happen in the backend. Clients never compute release geometry. *(Confirmed principle; mechanism Proposed.)*
- **Desktop engineering application** — full workspaces (drawing intelligence, 3D, development, verification, output). Proposed: web UI in a desktop shell talking to a backend that may be local (same workstation), on-premises server or private cloud.
- **Tablet** — review, annotation, 3D inspection, approval.
- **Mobile** — upload, status, previews, comments, approval actions with step-up authentication.
- **Backend** — same artifacts in three topologies: single-node (factory workstation/server, no internet), on-premises cluster, private cloud. Feature parity for Z2/Z3 is required across topologies; Z1 capability may differ by automated-reading policy (e.g. local models only).
- **Offline** — clients cache jobs read-only plus annotations and draft decisions. Offline outputs are labelled *local / unsynchronised* and can never carry *Approved* status. Approval requires connection to the authoritative backend for that tenant.
- **Synchronisation and conflict** — the truth graph is append-only and content-addressed, so sync is set-union of immutable records plus reconciliation of mutable *heads* (current revision pointers, job status). Concurrent head changes are detected and surfaced for human resolution; engineering decisions are never auto-merged. *(Proposed.)*

## 6. Data architecture

Detail: [ADR-006](../architecture/ADR-006-data-storage-and-provenance.md).

### 6.1 Engineering Truth Graph

A directed acyclic graph of immutable, content-addressed records. Edges mean "was derived from".

| Record | Contents |
|---|---|
| SourceArtifact | Original bytes (object store), hash, format, upload identity, customer automated-reading policy at time of upload |
| Observation | Value, unit, provenance class, source region (page, bbox / entity id), extractor + version, uncertainty, status (proposed / accepted / rejected / superseded) |
| Decision | Human engineering decision: who, when, what, why, scope; signed |
| Constraint | Relation over parameters, originating observations |
| Parameter | Solved value, uncertainty, residual contribution |
| Geometry | Kernel-neutral description + kernel-native blob, tolerance, kernel version |
| Calculation | Function id, algorithm version, inputs, outputs, numeric regime, error bounds, environment fingerprint |
| ProcessData | Material, thickness, tooling, calibrated model parameters, validity range, calibration evidence |
| Evidence | Gate id, method, independence class, inputs, result (pass / fail / inconclusive), margins |
| Output | DXF/STEP/report bytes, hash, entity→geometry map |
| Approval | Release status, approver identity, signature, gate snapshot |

- **ARCH-12** From any DXF entity it must be possible to traverse to: generating calculation(s), parameters, constraints, observations, source regions on the original drawing, and every human decision involved.
- **ARCH-13** Records are never updated in place. Corrections create new records that supersede old ones; history is retained according to retention policy.
- **ARCH-14** The four provenance classes are distinct types and are never merged into an undifferentiated dimension list.
- **ARCH-15** Changing an input identifies the affected sub-graph (impact analysis) and invalidates dependent evidence and approvals.

### 6.2 Storage (Proposed)

Relational database for graph metadata, indices, projects, permissions and audit; object storage for large immutable blobs keyed by hash; an append-only, hash-chained audit log. Both stores must have single-node on-premises equivalents. Tenant isolation model (schema-, database- or deployment-per-customer) is **Open** pending Specification 07.

### 6.3 Revisions

A drawing revision is a new SourceArtifact linked to its predecessor. The system diffs observations between revisions, re-uses unaffected calculations by content address, and requires re-verification and re-approval of everything downstream of a change.

## 7. Reliability and performance

- **Deterministic processing** — ARCH-04, §3.3.
- **Reproducibility** — **ARCH-16** Any released output must be regenerable from recorded inputs, configuration, algorithm versions and decisions; regeneration is an automated test, run before release and periodically on archived jobs. Old algorithm versions remain executable (pinned build artifacts) for the retention period.
- **Parallelism** — pages, parts, candidate interpretations, Monte Carlo samples and verification checks are independent units; scheduling must not affect results.
- **Incremental recomputation** — calculations keyed by hash of (function, version, inputs); unchanged sub-graphs are reused.
- **Geometry caching** — content-addressed; cache is an optimisation only and can be dropped without changing results.
- **Job recovery** — durable job graph; steps idempotent; checkpoint after each step; resume after crash without duplicating records.
- **Failure isolation** — untrusted-file parsers and native geometry code run in separate, resource-limited processes; a crash fails one step with a recorded outcome, not the service. Experimental algorithm versions are deployable alongside production but cannot produce releasable evidence.
- **Resource management** — per-job limits (time, memory, page count, entity count); resource classes (CPU, GPU, high-memory); back-pressure and fair scheduling across tenants.

### 7.1 Benchmark methodology

No performance figures are stated until measured. For each stage in master-plan §12: fixed, versioned corpus (see [Benchmark Plan](../benchmark/ENGINEERING_BENCHMARK_PLAN.md)); declared hardware profiles (reference workstation, reference server); cold and warm runs; report median, p95 and maximum wall time, peak memory, and failure/timeout counts; concurrency sweeps for throughput; results stored with commit, dependency versions and hardware fingerprint; regressions gate merges once baselines exist. Targets are set from the factory's actual workload after the first baseline.

## 8. Security

Full treatment in Specification 07. Architectural commitments:

- **Encryption** — TLS for all transport including internal service traffic; encryption at rest for database, object store, backups and client caches.
- **Customer isolation** — every record carries a tenant; enforced in the data layer, not only in application code; cross-tenant caches are forbidden (content-address deduplication is per tenant).
- **Role-based access** — at minimum: viewer, engineer, approver, administrator, auditor; release authority is a distinct permission; separation of duties between the engineer who prepared a job and the approver is configurable per customer.
- **Key management** — keys in an HSM/KMS or OS keystore; per-tenant data keys; customer-held keys supported for private deployments; rotation and revocation procedures.
- **Auditability** — append-only, hash-chained log of access, decisions, overrides, approvals, exports and administrative actions.
- **Private deployment** — fully functional with no outbound network access; updates delivered as signed offline bundles.
- **Data retention** — per-customer retention and verifiable deletion, reconciled with reproducibility duties (ARCH-16). The conflict between "delete on request" and "regenerable for N years" is **Open** and needs customer/legal input.
- **Secure file processing** — all Z0 files are hostile: content-based type detection, sandboxed parsing without network or filesystem access, resource limits, decompression-bomb limits, no execution of embedded scripts/macros, sanitised derived renderings for client display.
- **Confidential drawings and automated reading** — customer automated-reading policy has three levels (external reading services allowed / local models only / deterministic software only), enforced at the orchestration layer and recorded on every SourceArtifact. No customer data is used for model training without explicit recorded permission. Corrections captured for learning are stored per tenant.

## 9. API and integration architecture

- **Style** — one versioned public API for all clients; internal contracts between services defined in a schema language with generated bindings (Rust, Python, client). Protocol choice is **Open** (ADR-001).
- **Versioning** — API version independent of algorithm versions; algorithm and schema versions appear in every result; additive changes within a major version; record schemas are migrated forward, never rewritten.
- **Long-running work** — submit → job id → status stream; states: queued, running, waiting-for-input, succeeded, failed, cancelled; each step exposes progress, outcome and produced records.
- **Error model** — three distinct categories that must never be conflated: *system faults* (retryable infrastructure errors), *engineering outcomes* (unsupported geometry, ambiguous drawing, contradiction, out-of-range model, tolerance exceeded — normal results with structured detail and source references), and *authorisation failures*. Messages are localisable (JA/EN) by code, with parameters.
- **CAD exchange** — import/export adapters behind a stable interface; each adapter declares supported versions and entity coverage, and reports everything it dropped or approximated. Detail in Specification 06.
- **CAM integration** — per-customer/machine output profiles (layer mapping, entity restrictions such as arcs vs polylines, units, file naming); feedback endpoint for inspection measurements and correction reports.
- **Extensibility** — shape modules and process models are versioned plug-ins implementing the shape-module contract (master plan §8); a module cannot be enabled for release without its reference tests and declared applicability range.

## 10. Architecture decisions register

### Confirmed (governing principles from the master plan)

| ID | Decision |
|---|---|
| C1 | Automated drawing reading is never the final authority for critical geometry; calculation authority is deterministic |
| C2 | Release requires independent verification evidence plus an authorised engineer |
| C3 | Three release statuses; failed critical checks never yield a "complete" DXF |
| C4 | Four provenance classes kept distinct; no silent guessing |
| C5 | End-to-end provenance and reproducibility for every released output |
| C6 | Cloud, private-cloud and on-premises deployment; Japanese and English |
| C7 | No CAD kernel written from scratch |
| C8 | No absolute zero-physical-error claim |

### Proposed (need review and/or evidence)

| ID | Decision | ADR |
|---|---|---|
| P1 | Modular backend with identical contracts in single-node and distributed topologies; native/parsing code process-isolated | 001 |
| P2 | Rust for core, orchestration and APIs; native libraries behind a narrow C ABI in isolated worker processes | 002 |
| P3 | Kernel-neutral port; own analytic layer for developable primitives; B-rep kernel for general modelling/exchange and as independent cross-check | 003 |
| P4 | UEM as a pure library with mixed numeric regimes and mandatory error/uncertainty metadata | 004 |
| P5 | Four ingestion pipelines producing typed observations only | 005 |
| P6 | Immutable content-addressed truth graph on relational + object storage | 006 |
| P7 | Shared web-technology UI across desktop shell, tablet and mobile; thin clients | 007 |
| P8 | Single build, three topologies; on-premises treated as the baseline, not an afterthought | 008 |
| P9 | Python confined to Z1 | 002, 005 |

### Open technical questions

1. Cross-platform bit-reproducibility with a C++ kernel in the loop — achievable, or bounded-equivalence only?
2. How much of general (non-quadric) reconstruction can avoid the B-rep kernel?
3. Multi-view reconstruction: search strategy over candidate interpretations and its termination criteria.
4. Representation of arcs/splines in the 2D profile pipeline versus CAM acceptance of each entity type.
5. Tenant isolation model; retention versus reproducibility.
6. DWG support path (see licensing review): licensed SDK, GPL tool in an isolated process, or DXF-only initially.
7. Which OCR/vision models meet accuracy needs on Japanese drawings when restricted to local execution.
8. Internal protocol and schema language.

### Risks

| Risk | Consequence | Mitigation direction |
|---|---|---|
| Common-mode error between calculation and verification | False confidence | §3.4 independence classes; physical validation (benchmark level 3) |
| Kernel silent-wrong results on degenerate geometry | Bad geometry passes Z2 | Analytic references, exact 2D checks on final profiles, cross-check |
| GPL/LGPL obligations conflict with closed on-premises distribution | Legal exposure or forced redesign | Licensing review before any dependency is adopted; kernel port keeps options open |
| Scope: 14 modules × 3 device classes × 3 topologies | Schedule failure | Roadmap phases; on-prem single-node + desktop first |
| Insufficient authorised factory data | Unvalidated product | Benchmark plan started now; synthetic analytic corpus as a floor, not a substitute |
| Uncalibrated process models (K-factor, springback, kerf) | Physically wrong parts despite correct maths | ARCH-11; calibration data as ProcessData with evidence |
| FFI instability / native crashes | Reliability | Process isolation, step-level recovery |
| Reproducibility across dependency upgrades | Cannot regenerate old releases | Pinned, archived build artifacts per algorithm version |

### Decisions requiring benchmarks

Kernel selection (§4.3); 2D profile library; exact vs filtered arithmetic costs; PDF extraction library; OCR/vision models on JA/EN drawings; vectorisation accuracy vs scan resolution; determinism across platforms; all performance targets.

### Decisions requiring legal or licensing review

OCCT LGPL-2.1 + exception in on-premises and (if ever) client-side distribution; any CGAL GPL package; commercial kernel terms; DWG technology and trademark; PDF library (AGPL exposure); reading-model weight licences; aggregate third-party notices; export-control and customer contractual terms on drawing data; patent landscape (Document 09).
