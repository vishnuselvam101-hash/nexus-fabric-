# NEXUS FABRIC — Product, Engineering and Development Master Plan

Proposed product strategy · Version 1.0 · 18 November 2026

> **Status:** proposal. The decisions in [section 21](#21-decisions-to-lock) are recommended defaults, not formally approved.
> Competitor and standards statements were taken from the vendors' and standards bodies' published material at the time of writing and must be re-verified in the competitor and patent research (document 09).

## Contents

1. [The product](#1-the-product)
2. [What the complete product must do](#2-what-the-complete-product-must-do)
3. [Competitive strategy](#3-competitive-strategy)
4. [Application architecture: 14 modules](#4-application-architecture-14-modules)
5. [User interface: six workspaces](#5-user-interface-six-workspaces)
6. [Mathematical foundation](#6-mathematical-foundation)
7. [Drawing intelligence engine](#7-drawing-intelligence-engine)
8. [Geometry and fabrication engine](#8-geometry-and-fabrication-engine)
9. [DXF generation system](#9-dxf-generation-system)
10. [Multi-device architecture](#10-multi-device-architecture)
11. [Technology stack](#11-technology-stack)
12. [Performance and reliability](#12-performance-and-reliability)
13. [Security and industrial data protection](#13-security-and-industrial-data-protection)
14. [VNX-DNA reuse](#14-vnx-dna-reuse)
15. [Testing and benchmark](#15-testing-and-benchmark)
16. [Development roadmap](#16-development-roadmap)
17. [Commercial approach to competitors](#17-commercial-approach-to-competitors)
18. [Potential intellectual property](#18-potential-intellectual-property)
19. [Success criteria](#19-success-criteria)
20. [First research and build package](#20-first-research-and-build-package)
21. [Decisions to lock](#21-decisions-to-lock)

---

## 1. The product

**Objective:** convert engineering drawings supplied as PDFs, images, scanned documents and CAD files into mathematically verified, manufacturing-ready DXF drawings, with traceable engineering decisions and measurable accuracy.

This is not a PDF converter, OCR application, CAD editor or AI chatbot. It is an **Engineering Drawing-to-Manufacturing Geometry System**.

| | |
|---|---|
| Input | PDF · images · scans · CAD |
| Output | Verified DXF · CAD package |
| Core | Mathematical engineering |
| First market | Japanese manufacturers |

### The fundamental principle

1. AI interprets the drawing.
2. Mathematical and geometric engines calculate the result.
3. Independent verification engines challenge that result.
4. Manufacturing rules determine whether it can be produced.
5. Engineers authorise release.

**AI must never be the final authority for critical geometry.**

### What we can and cannot promise

We can target **zero unverified releases** and **zero known mathematical inconsistencies**. We cannot honestly guarantee zero physical manufacturing error: input uncertainty, drawing ambiguity, material behaviour, machine calibration, cutting kerf and welding distortion make an absolute guarantee impossible. The system must detect, quantify, reject or escalate those risks instead of hiding them. That distinction is a central product advantage.

## 2. What the complete product must do

One continuous engineering workflow:

| Step | Stage | Description |
|---|---|---|
| 01 | Import | Upload PDF, scanned PDF, image, DWG, DXF or supported CAD. |
| 02 | Understand | Identify views, dimensions, units, annotations, materials and drawing relationships. |
| 03 | Reconstruct | Build a constrained 2D/3D engineering representation. |
| 04 | Calculate | Compute exact developments where mathematically possible, and controlled approximations where necessary. |
| 05 | Engineer | Determine plate divisions, seams, bends, allowances and production constraints. |
| 06 | Verify | Run independent mathematical, geometric, dimensional and manufacturing checks. |
| 07 | Deliver | Generate DXF, preview, engineering report and machine/CAM handoff package. |

The product must not stop at the first DXF export. It must preserve the relationship between the original drawing, the calculated geometry, the verification evidence and the final manufacturing output.

## 3. Competitive strategy

Two particularly relevant competitors (to be re-verified hands-on):

- **Unfoldo AI** accepts PDF, DXF and photos, identifies supported sheet-metal shapes, reads dimensions and generates flat patterns, with DXF, STEP and PDF outputs.
- **CUTL** has a PDF-to-STEP beta producing a 3D model, flat-pattern DXF and preview. Its documentation describes restrictions including vector PDFs, one sheet-metal part per drawing and supported-geometry limits.

These are meaningful competitors and should not be dismissed.

### Traps to avoid

| Common limitation or trap | Our design response |
|---|---|
| Shape catalogue dependency | Parametric geometry plus a general constrained-geometry framework |
| Narrow set of supported drawings | Multiple input pipelines and an explicit unsupported-geometry workflow |
| AI guesses missing dimensions | No silent guessing; source traceability and uncertainty tracking |
| One PDF page treated as one part | Multi-page, multi-view, multi-part drawing relationships |
| Flat pattern without sufficient validation | Independent verification before release |
| One file conversion is the whole product | Complete engineering job, revision, approval and manufacturing workflow |
| Cloud-only processing | On-premises and private deployment options |
| No learning from production | Capture approved corrections and measured manufacturing feedback |
| Mobile is an afterthought | Mobile, tablet and desktop interfaces over one engineering platform |

The differentiation is not more AI features. It is an engineering system that can explain why it generated a geometry, prove which inputs support it, show its uncertainty and refuse unsafe output.

### Four prioritised innovations

1. **Engineering Truth Graph** — a dependency graph linking every source dimension, drawing view, geometric constraint, calculation, manufacturing decision and DXF entity. Change a dimension and the system identifies everything affected.
2. **Independent Verification Tribunal** — different mathematical methods independently reconstruct and check the same part. Disagreement blocks production release rather than being hidden behind a confidence score.
3. **Digital Fabrication Rehearsal** — simulate the proposed flat patterns being assembled into the intended geometry. Check seams, fit-up, plate boundaries, bend sequence and manufacturing constraints before cutting.
4. **Engineering Uncertainty Map** — every dimension and geometric feature gets a provenance record, uncertainty estimate and release status. Ambiguous source information becomes a visible engineering issue.

These are proposed differentiators, not claims of patentable novelty. A dedicated prior-art and patent landscape study is required before any exclusivity claims.

## 4. Application architecture: 14 modules

A full engineering platform with multiple workspaces, sharing one engineering data model, one permission system and one calculation infrastructure.

| No. | Module | What it must contain |
|---|---|---|
| 1 | Project & Job Management | Projects, customers, jobs, revisions, materials, quantities, deadlines and status |
| 2 | Drawing Import Centre | PDF, scanned PDF, PNG, JPG, TIFF, DWG, DXF, STEP and supported CAD formats |
| 3 | Drawing Intelligence | OCR, dimensions, symbols, annotations, views, units, title blocks and Japanese/English interpretation |
| 4 | Drawing Workspace | Original drawing viewer, vector layers, dimension overlays, page comparison and annotations |
| 5 | Geometry Reconstruction | Constrained 2D geometry, 3D reconstruction, surfaces, solids, assemblies and topology |
| 6 | Mathematical Engineering Core | Dimensional calculations, surface development, numerical methods, geometry and constraints |
| 7 | Fabrication Development | Flat patterns, reducers, branches, transitions, plate divisions, seams, bends and allowances |
| 8 | Material & Process Planner | Plate selection, stock dimensions, kerf, cutting methods, nesting and waste optimisation |
| 9 | Simulation & Digital Rehearsal | Fold-back reconstruction, assembly, fit-up, seam and manufacturing simulation |
| 10 | Verification Centre | Independent calculations, uncertainty analysis, tolerance checks and release decisions |
| 11 | CAD/DXF Studio | DXF editing, layers, dimensions, profiles, geometry inspection and export |
| 12 | Manufacturing Integration | CAM handoff, machine profiles, cutting instructions and production feedback |
| 13 | Engineering Knowledge Centre | Approved templates, standards, customer rules, material databases and versioned engineering knowledge |
| 14 | Administration & Security | Users, roles, approvals, audit logs, encryption, deployment and system configuration |

## 5. User interface: six workspaces

- **A. Engineering Home** — active jobs, recent drawings, pending reviews, production status, engineering alerts and team workload.
- **B. Drawing Intelligence Workspace** — original PDF on one side, extracted dimensions and detected views on the other, with clickable source-to-geometry references.
- **C. 3D Engineering Workspace** — reconstructed model, surface inspection, cross-sections, dimensions, constraints and editable engineering parameters.
- **D. Development & Fabrication Workspace** — flat-pattern layouts, plate divisions, seam positioning, bending, allowances and nesting.
- **E. Verification & Approval Workspace** — mathematical results, independent comparison, uncertainty, tolerance budget, failed checks and approval history.
- **F. Output & Production Workspace** — DXF preview, layer configuration, cutting profiles, manufacturing package, CAM handoff and production results.

The interface supports Japanese and English throughout, not merely translated buttons: technical terms, units, drawing annotations, error messages, reports and customer-specific terminology are localised. The engineering calculations remain language-independent.

## 6. Mathematical foundation

There is no single equation that makes every fabrication drawing error-free. We need a collection of mathematical models, each appropriate to a particular geometric and manufacturing problem: the **Universal Engineering Mathematics Engine (UEM)**.

### 6.1 Disciplines to implement

| Discipline | Purpose |
|---|---|
| Linear algebra | Coordinate transformations, projections, transformations between drawing views |
| Euclidean geometry | Distances, angles, circles, intersections and profiles |
| Analytic geometry | Lines, planes, conics and geometric relationships |
| Differential geometry | Surface curvature and flat-pattern development |
| Computational geometry | Intersections, offsets, clipping, topology and geometric predicates |
| Numerical analysis | Stable numerical calculations and error bounds |
| Interval arithmetic | Propagating bounded uncertainty through calculations |
| Constraint solving | Finding geometries consistent with drawing dimensions |
| Nonlinear optimisation | Resolving overdetermined or noisy geometric constraints |
| Statistical estimation | Evaluating uncertain measurements and competing interpretations |
| Mechanics and material models | Bending, springback, deformation and process-specific compensation |
| Discrete optimisation | Plate divisions, seam placement and material nesting |

### 6.2 Equations and models

#### A. Coordinate transformations

A drawing can contain multiple projections of the same 3D object.

$$\mathbf{p}' = R\,\mathbf{p} + \mathbf{t}$$

where $R$ is a rotation matrix, $\mathbf{t}$ a translation vector, $\mathbf{p}$ the original coordinate and $\mathbf{p}'$ the transformed coordinate. Homogeneous coordinates combine rotation, translation and scaling:

$$T = \begin{bmatrix} R & \mathbf{t} \\ 0 & 1 \end{bmatrix}$$

This is central to reconstructing geometry from front, side, top and sectional views.

#### B. Constraint-based reconstruction

Measured dimensions and relationships give a system

$$f_i(\mathbf{x}) = 0$$

where $\mathbf{x}$ contains the unknown geometric parameters. For noisy or conflicting observations $y_i$, use weighted least squares:

$$\mathbf{x}^{*} = \arg\min_{\mathbf{x}} \sum_i w_i \left(f_i(\mathbf{x}) - y_i\right)^2$$

**Rule:** a least-squares solution is not proof that the drawing is correct. It is only a best fit under the chosen model. Residuals, constraints, ambiguity and uncertainty are checked separately.

#### C. Surface modelling and developability

Parametric surface:

$$\mathbf{S}(u,v) = \begin{bmatrix} x(u,v) \\ y(u,v) \\ z(u,v) \end{bmatrix}$$

First fundamental form:

$$I = E\,du^2 + 2F\,du\,dv + G\,dv^2, \qquad E = \mathbf{S}_u\cdot\mathbf{S}_u,\quad F = \mathbf{S}_u\cdot\mathbf{S}_v,\quad G = \mathbf{S}_v\cdot\mathbf{S}_v$$

Gaussian curvature, with $e, f, g$ the coefficients of the second fundamental form:

$$K = \frac{eg - f^2}{EG - F^2}$$

For a regular surface, $K = 0$ everywhere is the condition for local developability. Cylinders and cones are the important examples. A doubly curved surface generally cannot be flattened without distortion.

The software must make the development class explicit:

- Exact development.
- Piecewise development.
- Approximate development with quantified distortion.
- Not developable under the requested manufacturing process.

**Never label an approximate pattern as an exact pattern.**

#### D. Sheet-metal bending

For suitable conventional bend models:

$$BA = \theta\,(R_i + K t)$$

where $BA$ is the bend allowance, $\theta$ the bend angle in radians, $R_i$ the inside bend radius, $t$ the material thickness and $K$ the neutral-axis factor (K-factor; distinct from Gaussian curvature above). The bend deduction is commonly expressed as:

$$BD = 2\,(R_i + t)\tan\!\left(\frac{\theta}{2}\right) - BA$$

These are not universal material laws. K-factor, bend radius, springback and compensation must be calibrated to material, thickness, tooling and process. Rolled or welded heavy plate may need different models and process data.

#### E. Uncertainty propagation

For an output $y = g(\mathbf{x})$, the first-order approximation is

$$u_y^2 \approx J\,\Sigma\,J^{T}$$

where $J$ is the sensitivity (Jacobian) of $y$ with respect to the inputs and $\Sigma$ the covariance matrix of the input quantities. For difficult nonlinear cases use Monte Carlo propagation or interval methods rather than trusting the first-order approximation.

The metrology foundation follows the GUM family of guidance (JCGM 100, JCGM 101 for Monte Carlo propagation; ISO/IEC Guide 98-1:2024 as introduction).

#### F. Geometric consistency

Compare the folded/reconstructed development with the intended 3D geometry, for example

$$e_{\max} = \max_{p \in P} d(p, S)$$

where $d(p, S)$ is the distance from a reconstructed point $p$ to the intended surface $S$, with a carefully specified correspondence and metric. Also evaluate:

- Surface deviation.
- Boundary mismatch.
- Edge-length consistency.
- Seam alignment.
- Geometric closure.
- Volume and area consistency where applicable.
- Self-intersection and invalid topology.

A single maximum-distance metric is not enough. Each part type needs its own set of checks.

### 6.3 Numerical precision strategy

Do not use one precision strategy for everything:

- Exact rational arithmetic where the geometry permits it.
- Exact geometric predicates for critical topological decisions.
- Higher-precision numerics where exact algebraic representation is impractical.
- Interval arithmetic for bounded uncertainty.
- Independent geometric validation for critical outputs.
- Explicit manufacturing tolerance budgets.

CGAL distinguishes exact-predicate/inexact-construction kernels from exact-predicate/exact-construction kernels. Exact decisions do not automatically make every constructed coordinate exact.

### 6.4 The engineering release gate

Every critical DXF must pass:

- [ ] All required source dimensions have traceable origins
- [ ] Units, scale and projection are resolved
- [ ] Geometric constraints are satisfied
- [ ] Surface development method is appropriate
- [ ] Thickness and process allowances are validated
- [ ] Independent reconstruction agrees within specified tolerance
- [ ] No invalid or self-intersecting cutting profiles
- [ ] DXF entities, layers and units pass validation
- [ ] Manufacturing process constraints are satisfied
- [ ] Authorised engineer approves release

**Release decision:** approved only if all required gates pass. A failed or unresolved critical check produces *Review Required* or *Rejected*, never a supposedly complete DXF.

The release policy is based on the customer's specified tolerances and the applicable measurement uncertainty, not an arbitrary universal tolerance. ISO 14253-1 addresses conformity decisions that take measurement uncertainty into account.

## 7. Drawing intelligence engine

Four distinct processing pipelines.

### Pipeline A — Vector PDF

- Parse PDF objects.
- Extract vector paths, lines, curves and text.
- Preserve page coordinates and transformation matrices.
- Identify drawing boundaries, title blocks and views.
- Extract dimension text and associate it with geometric entities.
- Reconcile extracted values against the drawing's scale and explicit dimensions.

### Pipeline B — Scanned PDF and images

- Detect page orientation.
- Correct rotation and perspective where appropriate.
- Estimate image resolution and physical scale.
- Remove noise without destroying engineering lines.
- Detect text regions and dimension symbols.
- OCR with Japanese and English technical vocabulary.
- Detect linework and curves; vectorise the geometry.
- Compare detected geometry with written dimensions.
- Record the uncertainty introduced by scanning.

### Pipeline C — CAD files

- Parse supported DWG/DXF entities.
- Preserve layers, blocks, units and coordinate systems.
- Identify model-space geometry and drawing annotations.
- Extract supported 3D models and assembly information.
- Preserve original geometry for comparison.

### Pipeline D — Multi-view engineering interpretation

- Identify front, side, top, section and detail views.
- Determine projection relationships.
- Match corresponding edges and features across views.
- Associate dimensions with the relevant geometric parameters.
- Build candidate 3D interpretations.
- Check candidates against all views and dimensions.
- Identify under-defined and contradictory geometry.
- Ask for clarification or mark the job for engineer review.

### Provenance classes

The system distinguishes between what is:

| Class | Meaning |
|---|---|
| **Written** | Stated explicitly on the drawing |
| **Measured** | Measured from an image or vector geometry |
| **Inferred** | Derived from geometric constraints |
| **Assumed** | Introduced by a selected manufacturing rule |

These must never be mixed into one undifferentiated set of dimensions.

## 8. Geometry and fabrication engine

The main intellectual property opportunity. Do not hard-code the product around reducers and pipes: build a general geometry framework with process-specific engineering modules.

### Geometry families

| Family | Scope |
|---|---|
| Pipes and reducers | Cylinders, cones, eccentric transitions, concentric transitions |
| Branches and intersections | Tee branches, oblique intersections, saddle cuts, multi-branch assemblies |
| Rectangular transitions | Square-to-round, rectangular-to-round, custom transitions |
| Hoppers and chutes | Multi-panel developments, sloped surfaces and custom plate structures |
| Vessels and tanks | Shell courses, heads, openings and qualified fabrication methods |
| Sheet-metal components | Bends, flanges, hems, reliefs and folded parts |
| Structural fabrication | Plates, profiles, connections, weld preparations |
| Custom fabrication | New geometry types through extensible engineering modules |

### Shape module contract

Each supported shape has:

- A parametric definition.
- Required and optional parameters.
- Geometric constraints.
- A mathematical development method.
- A manufacturing method.
- A verification method.
- A tested set of reference examples.
- An explicit applicability range.

A reducer module, for example, does not just store diameter and length. It represents the complete engineering problem: inlet and outlet geometry, thickness, eccentricity, material, segmentation, seam constraints, end conditions and the intended fabrication process.

### Plate division and seam optimisation

For a complex part there may be several ways to divide the geometry into plates. Optimise the division subject to engineering constraints:

$$\min_{\mathbf{x}} \left[\, w_1 C_{\text{material}} + w_2 C_{\text{weld}} + w_3 C_{\text{cut}} + w_4 C_{\text{assembly}} + w_5 C_{\text{risk}} \,\right]$$

subject to:

- Required geometry is covered.
- Plate sizes are within available stock dimensions.
- Seams are geometrically compatible.
- Weld accessibility and customer rules are satisfied.
- Manufacturing and tolerance constraints are satisfied.
- The chosen process is capable of producing the result.

Weights are configurable per factory. A company prioritising minimum welding may choose a different solution from one prioritising minimum plate waste.

**Innovation:** produce multiple feasible fabrication strategies, explain their trade-offs and let the authorised engineer select one. The best geometry is not necessarily the cheapest fabrication strategy.

## 9. DXF generation system

DXF is an engineering deliverable with its own validation requirements.

### Proposed layer convention

| Layer | Contents |
|---|---|
| `CUT_OUTER` | External cutting profiles |
| `CUT_INNER` | Holes and internal profiles |
| `BEND_LINES` | Bend or fold references |
| `ROLL_LINES` | Rolling references |
| `WELD_REFERENCE` | Welding and seam references |
| `CENTER_LINES` | Centre lines |
| `REFERENCE` | Construction geometry |
| `DIMENSIONS` | Dimensions |
| `ANNOTATIONS` | Labels and instructions |
| `NO_CUT` | Geometry not intended for cutting |

These are internal conventions. Each CAM customer or machine integration may require a different layer mapping.

### Generation stages

1. Convert the verified engineering geometry into a manufacturing drawing representation.
2. Convert units explicitly.
3. Generate the required entities.
4. Validate closed profiles and curve continuity.
5. Check duplicate, overlapping and zero-length entities.
6. Check self-intersections and topology.
7. Validate layer names and entity types.
8. Validate extents, orientation and scale.
9. Re-import the generated DXF through an independent parser.
10. Compare the re-imported geometry with the approved engineering geometry.
11. Produce a validation report.
12. Release the output only after the approval policy is satisfied.

Autodesk's DXF reference describes the header, table and entity structure, including unit information; a writer must keep the drawing structure consistent for reliable acceptance.

### Manufacturing job package

| Item | Contents |
|---|---|
| Original drawing | Customer-supplied PDF or image |
| 3D model | STEP or other supported neutral CAD format |
| DXF files | Verified flat patterns and cutting profiles |
| Engineering report | Dimensions, calculations, assumptions and tolerances |
| Verification certificate | Checks performed, results and approval identity |
| Revision history | Input changes and output differences |
| Manufacturing data | Material, thickness, quantity and process instructions |

## 10. Multi-device architecture

A single engineering platform across devices, not separate products with separate calculations.

| Device | Role |
|---|---|
| Desktop / engineering workstation | Full drawing processing, 3D CAD, calculations, fabrication planning, DXF editing, verification and approval |
| Tablet | 3D inspection, drawing annotation, comparison, measurement review, collaboration and engineering approval |
| Mobile | Upload drawings, job status, 2D/3D previews, alerts, comments, approval requests and authorised release actions |
| Engineering compute infrastructure | Heavy geometry, AI inference, batch processing, verification, data storage and integrations |

### Recommended architecture

- Desktop: native engineering application or high-performance desktop shell.
- Mobile and tablet: responsive application with native device integrations where required.
- 3D preview: GPU-accelerated rendering, with lightweight viewing on mobile.
- Server: Rust-based calculation services, job orchestration and storage.
- Local deployment: optional factory workstation or on-premises server.
- Cloud: optional private cloud or managed enterprise deployment.

Mobile does not run every complex CAD operation locally. Heavy computation runs on the engineering server or a capable workstation; the mobile client provides a complete, responsive review experience.

### Offline operation

Internet availability cannot be assumed for industrial customers:

- Local job caching.
- Offline drawing viewing and annotations.
- Secure local storage.
- Controlled synchronisation.
- Conflict detection for concurrent edits.
- Local calculation for selected lightweight operations.
- A clear distinction between locally calculated, synchronised and approved outputs.

## 11. Technology stack

| Component | Proposed technology | Responsibility |
|---|---|---|
| Core calculation engine | Rust | Deterministic engineering services |
| CAD geometry kernel | Open CASCADE Technology | B-rep, surfaces, solids and CAD exchange |
| Computational geometry | CGAL | Selected robust geometric operations |
| AI services | Python | OCR, vision, drawing interpretation and orchestration |
| Application backend | Rust services | APIs, jobs, permissions and workflow |
| Database | PostgreSQL | Projects, geometry metadata, revisions and approvals |
| Object storage | S3-compatible storage | Original drawings, CAD files and outputs |
| Cache / job queues | Redis or equivalent | Job scheduling and temporary results |
| Desktop UI | Web technology in a desktop shell, with native integrations | Engineering workspace |
| Mobile UI | Cross-platform application | Mobile and tablet experience |
| 3D rendering | OpenGL/Vulkan/WebGPU-capable rendering path | Interactive 3D visualisation |

- **Why Rust:** memory safety and concurrency model for critical calculation and system services. Rust does not make mathematics correct; every numerical algorithm still needs independent validation.
- **Why Open CASCADE:** B-rep geometry, modelling algorithms, STEP/IGES exchange and shape healing.
- **Why CGAL:** geometric kernels with different exactness/performance trade-offs. Benchmark selected operations rather than making every calculation use exact constructions.

**Architecture decision:** Rust as the core application and calculation layer; established CAD libraries integrated through well-defined interfaces; the AI interpretation layer isolated from the trusted engineering calculation layer.

Do not build a CAD kernel from scratch.

> **Superseded in part (18 Nov 2026):** OCCT and CGAL are candidates, not commitments. No geometry library is adopted until licensing, integration, performance and suitability are evaluated — see [ADR-003](architecture/ADR-003-cad-kernel-selection-strategy.md) and the [licensing review](research/DEPENDENCY_LICENSING_REVIEW.md). Specifications are developed in the order given in the [specification roadmap](specifications/SPECIFICATION_ROADMAP.md), whose numbering replaces the list in section 20.

## 12. Performance and reliability

### Performance features

- Asynchronous drawing processing.
- Parallel processing of independent pages and parts.
- Incremental recomputation when one parameter changes.
- Geometry caching with content-based identifiers.
- Spatial indexes for large geometry models.
- Adaptive precision based on operation risk.
- GPU acceleration for visualisation and selected parallel operations.
- Batch job processing.
- Memory-efficient CAD data structures.
- Background verification and controlled job scheduling.

### Measured targets

Separate targets for: drawing import time; OCR and dimension extraction time; geometry reconstruction time; flat-pattern calculation time; independent verification time; DXF generation time; memory use per job; concurrent job throughput; mobile preview responsiveness.

No performance numbers are invented before benchmarking representative factory drawings. A fast incorrect DXF is worse than a slower verified DXF.

### Reliability architecture

- Deterministic calculations.
- Idempotent processing jobs.
- Versioned algorithms.
- Reproducible calculation records.
- Crash recovery and automatic checkpointing.
- Immutable approval history.
- Safe rollback.
- Strict separation of production and experimental calculation versions.

Every released output is reproducible from its original inputs, configuration, algorithm version and engineering decisions.

## 13. Security and industrial data protection

Manufacturers supply confidential customer drawings, dimensions, material specifications and proprietary fabrication methods. Required controls:

- Encryption in transit and at rest.
- Customer and project isolation.
- Role-based access control.
- Multi-factor authentication.
- Secure key management.
- Audit logs.
- Approval and release permissions.
- Private/on-premises deployment.
- Retention and deletion policies.
- Backup and recovery.
- No use of confidential customer drawings for model training without explicit permission.

**Customer-controlled AI policy:** customers decide whether drawing data can be processed by external AI services, local AI models or only deterministic software.

## 14. VNX-DNA reuse

VNX-DNA remains a separate product and IP project unless explicitly decided otherwise. Possible reuse after technical review:

| VNX-DNA area | Potential use in NEXUS FABRIC |
|---|---|
| Rust engineering | Shared implementation techniques and reusable components |
| VNX-RAM concepts | Experimental caching or memory-management ideas |
| VNX-IO concepts | Storage interfaces and data handling |
| Encryption and integrity concepts | Secure engineering file management |
| Verification infrastructure | Reproducible tests and integrity checks |
| Simulated DNA encoding | Not needed for CAD geometry or flat-pattern calculations |

The simulated DNA-storage engine is not inserted into the geometry engine; there is no mathematical or performance reason to do so. Reuse goes through a formal code audit: identify reusable components, test performance, check licences and dependencies, and document ownership and provenance before integration.

## 15. Testing and benchmark

A private **NEXUS FABRIC Engineering Benchmark** using authorised factory drawings and approved manufacturing outputs.

### Dataset design

Representative jobs: simple cylindrical parts; concentric and eccentric reducers; branch connections; intersecting pipes; rectangular-to-round transitions; multi-segment parts; complex custom geometry; scanned drawings; multi-page drawings; ambiguous and incomplete drawings; different materials and thicknesses.

Each benchmark job contains:

- Original drawing.
- Source dimensions.
- Engineer-approved 3D model or geometry reference where available.
- Approved development and DXF.
- Fabrication decisions.
- Material and process information.
- Physical inspection measurements where available.
- Engineer review notes.

### Test categories

| Category | What we measure |
|---|---|
| Drawing recognition | Correct identification of dimensions and annotations |
| Reconstruction | Geometric agreement with approved references |
| Development | Flat-pattern deviation and mathematical consistency |
| Manufacturing | Feasibility of plate division, seams and process |
| DXF | CAM acceptance, valid entities and correct scale |
| Reliability | Repeatability and reproducibility |
| Performance | Processing time and resource use |
| Safety | Correct rejection of ambiguous or invalid jobs |

### Three levels

1. **Mathematical tests** — known analytical solutions, independently derived examples, boundary cases and property-based tests.
2. **Historical factory tests** — run completed jobs through the system and compare with approved historical engineering work.
3. **Physical production tests** — cut the material, fabricate the component and measure the finished part.

Level 3 is essential. Agreement with a digital reference does not prove that a real component will fit after cutting, rolling, bending and welding.

## 16. Development roadmap

Not all 14 modules at once. Each phase produces a testable engineering capability. These are planning estimates, not guaranteed dates.

| Phase | Timing | Scope | Deliverable |
|---|---|---|---|
| **0 — Research and specification** | Weeks 1–4 | Document the factory workflow; collect authorised drawings and completed jobs; competitor and prior-art research; geometry categories and mathematical requirements; data, security and verification models; benchmark CAD libraries and file-processing technologies | Complete technical specification and benchmark dataset plan |
| **1 — Drawing intelligence foundation** | Months 2–3 | PDF and image ingestion; vector extraction and scanned-image processing; Japanese/English OCR; dimension and annotation extraction; drawing workspace; source traceability | Drawing intelligence system that extracts and displays information with explicit uncertainty |
| **2 — Mathematical and geometry core** | Months 4–5 | Rust calculation services; coordinate transformations; constraint solver; CAD kernel integration; 2D/3D reconstruction; independent geometry verification | Tested geometric reconstruction engine |
| **3 — Fabrication development engine** | Months 6–7 | Pipe and reducer development; branch and intersection geometry; plate divisions; seam and welding constraints; thickness and material allowances; DXF generation | First end-to-end PDF-to-verified-DXF workflow |
| **4 — Verification and production simulation** | Months 8–9 | Independent reconstruction; uncertainty and tolerance analysis; fold-back and assembly checks; DXF validation; manufacturing simulation; engineer approval and audit trail | Controlled factory pilot version |
| **5 — Factory pilot and commercial readiness** | Months 10–12 | Historical benchmark testing; shadow-mode operation; physical cutting and inspection trials; Japanese/English production release; mobile and tablet applications; private deployment and customer onboarding | First commercially deployable product, subject to validation results |
| **6 — Universal fabrication expansion** | After validation | Hoppers and chutes; pressure vessels and tanks; advanced sheet metal; structural fabrication; ducting; machine-specific integrations | Expanding universal manufacturing engineering platform |

## 17. Commercial approach to competitors

Compete on five measurable capabilities, not on "more AI":

1. **Drawing coverage** — how many real factory drawing types can we process?
2. **Engineering reliability** — how often do outputs pass independent engineering review?
3. **Engineering time saved** — how many hours does the product eliminate?
4. **Manufacturing acceptance** — how often are outputs accepted by the customer's CAM and production workflow without correction?
5. **Traceability** — can the customer understand and reproduce every important engineering decision?

### Positioning

| Competitor category | Their primary proposition | Our intended proposition |
|---|---|---|
| Flat-pattern calculators | Calculate known shapes | Interpret drawings and engineer custom geometry |
| PDF-to-CAD converters | Extract or convert geometry | Reconstruct and validate engineering intent |
| General CAD systems | Create and edit models | Automate drawing-to-fabrication engineering |
| Sheet-metal CAD/CAM | Design and manufacture sheet-metal components | Connect interpretation, development, verification and production |
| AI drawing tools | Read technical documents | Generate auditable manufacturing geometry |

The market gap is a hypothesis until tested. Conduct hands-on evaluations of Unfoldo, CUTL, FastSHAPES/FastCAM and relevant Japanese CAD/CAM systems against the same authorised benchmark jobs.

### Shadow-engineering pilot

The customer's engineers continue their existing work while NEXUS FABRIC independently processes the same drawings. Compare: their engineering time vs our processing time; their approved geometry vs our proposed geometry; corrections required; actual manufacturing outcomes. This gives a prospective customer evidence instead of a marketing promise.

## 18. Potential intellectual property

To be investigated in a dedicated patent and prior-art study:

| Research area | Potential differentiation |
|---|---|
| Drawing-to-geometry provenance | Traceable links from source dimensions to generated geometry |
| Multi-view constraint reconstruction | Resolving ambiguous drawing geometry through multiple independent constraints |
| Independent verification architecture | Separate calculation and validation pathways |
| Adaptive development | Selecting exact, segmented or approximate development based on surface geometry and tolerances |
| Fabrication strategy optimisation | Optimising plate divisions and seams under real factory constraints |
| Manufacturing feedback loop | Updating process models from measured physical production results |
| Uncertainty-aware DXF release | Preventing release when source or calculation uncertainty is unacceptable |
| Cross-device engineering workflow | Securely coordinating work across workstation, tablet, mobile and factory infrastructure |

None of these is assumed new or patentable. Patent databases, technical papers and commercial products must be searched, followed by a freedom-to-operate review.

## 19. Success criteria

Success is defined by engineering evidence, not a feature checklist. Targets to validate and refine with the factory; not claims of existing performance.

| Criterion | Requirement |
|---|---|
| Input integrity | Every critical dimension has a source or an explicit approved engineering decision |
| Mathematical integrity | All applicable calculations pass independently defined numerical and geometric tests |
| Uncertainty control | No output is released when its uncertainty exceeds the job's specified tolerance budget |
| Geometry integrity | Generated patterns satisfy the applicable reconstruction and closure requirements |
| DXF integrity | Generated files pass independent parsing, units, topology and CAM compatibility tests |
| Production integrity | Physical test components meet the applicable drawing specifications |
| Reproducibility | A released result can be regenerated from its recorded inputs and versions |

### Release statuses

- **Approved** — all required checks pass and an authorised engineer has approved it.
- **Review required** — a calculation, input or manufacturing decision needs human attention.
- **Rejected** — the geometry, source information or manufacturing requirements are invalid or inconsistent.

## 20. First research and build package

Ten version-controlled engineering documents to exist before main implementation. Every implementation task refers to a requirement and a test.

| Document | Required contents |
|---|---|
| 01. Product Requirements Specification | Complete functionality and user workflows |
| 02. System Architecture Specification | Services, modules, APIs, deployment and data flow |
| 03. Mathematical Engineering Specification | Equations, algorithms, numerical methods and applicability |
| 04. Drawing Intelligence Specification | PDF, images, scans, OCR and multi-view interpretation |
| 05. Geometry and Fabrication Specification | Shape families, developments, segmentation and manufacturing rules |
| 06. Verification and Accuracy Specification | Tolerances, uncertainty, independent tests and release gates |
| 07. DXF and CAD Exchange Specification | Entities, layers, units, validation and compatibility |
| 08. Security and Deployment Specification | Encryption, private deployment, permissions and data protection |
| 09. Competitor and Patent Research | Product comparisons, prior art, patent landscape and FTO questions |
| 10. Engineering Benchmark Specification | Test dataset, metrics, acceptance criteria and physical validation |

## 21. Decisions to lock

### Strategic recommendation

Three tightly connected foundations:

1. **Drawing Intelligence** — understand what the engineer intended.
2. **Universal Engineering Mathematics** — calculate and reconstruct the geometry correctly.
3. **Independent Manufacturing Verification** — prove that the output meets the defined engineering requirements.

Output: verified DXF, plus 3D model, engineering report and manufacturing package.

**Make the verification system — not the AI model — the heart of the company.** AI models will improve and competitors can adopt similar models. A mature engineering system with a large validated dataset, proven geometry algorithms, customer-specific manufacturing rules, independent verification, physical production feedback and deep factory integration is much harder to reproduce.

The first commercial product should be narrow enough to validate rigorously, on a platform designed to expand.

### Recommended defaults

| Decision | Recommendation |
|---|---|
| Product identity | NEXUS FABRIC (working name) |
| Product category | AI-assisted manufacturing engineering platform |
| Core input | PDF, image, scanned drawing and CAD |
| Primary output | Verified DXF |
| Additional outputs | STEP, PDF, reports and manufacturing packages |
| Initial engineering domain | Complex steel pipe and plate fabrication |
| First market | Japan |
| Languages | Japanese and English |
| Core implementation | Rust + established CAD/geometry libraries |
| AI role | Drawing interpretation and engineering assistance |
| Calculation authority | Deterministic mathematical engine |
| Release authority | Independent verification plus authorised engineer |
| Deployment | Cloud, private cloud and on-premises |
| Devices | Desktop, tablet and mobile |
| VNX-DNA | Separate project; selective infrastructure reuse only |
| Accuracy objective | Zero known unresolved critical errors at release; tolerance-based physical validation |

### Immediate next step

Establish the authorised factory benchmark and complete the ten engineering specifications before starting the main implementation. The first coding milestone is a working drawing-ingestion and provenance pipeline, not a dashboard or an AI-generated DXF.
