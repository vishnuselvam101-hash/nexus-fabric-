# Implementation Dependency Matrix

Updated 18 November 2026

What each remaining block of work needs before it can be built (**B**) and before it can be trusted on real work (**V**). "Free" = can proceed now.

| Work block | Needs to build | Needs to validate | Status |
|---|---|---|---|
| More geometry in the maths engine (header opening, oblique cylinder, branch on cone, near-cylindrical cones) | Free (Spec 02) | Independent mathematical review of Spec 02; reference case RC-001 | Next |
| Interval bounds / exact predicates | Library choice or own implementation; licence evaluation | Benchmark of cost and width | Open |
| Rectangular-to-round transitions | Free for the ideal surface | Engineer review T1–T14; forming practice | Open |
| Thickness, allowances, kerf | ProcessData record type (free) | **Calibrated factory parameters (EV-5)** — none exist | BLOCKED for real use |
| Constraint solver | Free (Spec 02 §2.3) | Benchmark on real drawings | Open |
| Round scheduler, budgets, signals | Free (Determinism note) | Replay benchmark | Open |
| Dependency edges, obligations, Gate G18 | Mechanism built for one assembly type. **Rule table content: engineer review (0/56)** | Review package returned | Mechanism done; rules BLOCKED |
| Investigations, probes, strategy profiles | Mechanism free; explanation catalogues need engineering authorship | Benchmark protocol v1.3 | Partly blocked |
| Coverage ledger | Vector/CAD enumeration needs parsers; raster enumeration needs imaging library | Benchmark | Blocked on parser selection |
| Lanes, overlays, quarantine | Free | Safety-invariant tests | Open |
| Independent-instance replay | Separate process done; separate build/machine open | — | Partly done |
| Engineer approval (G16) and release | Requires G18, G19, lanes, per-user signing, roles for release authority | Security review | Deliberately last |
| PDF parsing | **Library selection + Dependency Evaluation Record** (PDFium / lopdf / own) | Benchmark on CAD-plotted Japanese PDFs | BLOCKED on selection and sample drawings |
| OCR (JA/EN), dimension recognition | **Engine and model selection; weight licences** | Authorised Japanese drawings | BLOCKED |
| View / projection intelligence | Depends on PDF parsing and OCR | Authorised drawings | BLOCKED |
| DXF import | Free (own reader can be extended) | Factory DXF samples | Open |
| STEP import/export, 3D kernel | **Kernel selection (ADR-003) and licence review** | Kernel benchmark | BLOCKED |
| DWG / JWW / IGES | Licensing decision | — | BLOCKED |
| 3D visualisation | Rendering library choice | — | Open |
| Sandboxed workers | Needed as soon as any parser or native library is added | Fuzzing | Open (required before PDF parsing) |
| PostgreSQL option, tenant isolation | Free | — | Open |
| Database encryption, key management, TLS, backup | Design decisions (Spec 07 not written) | Security review | Open |
| Japanese UI | Vocabulary review by a native-speaking engineer | — | Blocked on review |
| CAM acceptance of DXF | Access to the factory's CAM system; required DXF version and entity rules | Factory test | BLOCKED on factory |
| Installation packages | Target OS and hardware of the factory | — | BLOCKED on factory survey |

## Technology decisions taken for implementation (all provisional, replaceable)

| Decision | Choice | Reason | Revisit when |
|---|---|---|---|
| Core language | Rust (workspace of four crates) | Master plan and ADR-002 | — |
| Database | SQLite (bundled), single node | On-premises single-node baseline (ADR-008); no service to install | Multi-user load or clustering; PostgreSQL adapter |
| Web server | axum / tokio | Mainstream Rust stack, permissive licences as declared | — |
| Frontend | Dependency-free ES modules served from the binary | Runs offline with no build tooling; no third-party front-end licences to evaluate yet | UI complexity grows (3D views, drawing viewer) |
| Geometry kernel | **None** | ADR-003: no kernel selected; current families are analytic | Kernel evaluation |
| DXF | Own writer and independent reader | Format is simple; keeps validation independent | CAM requirements known |
| Records | JSON with SHA-256 identity | ADR-006 | Canonical encoding specification |

Third-party crates in use: [THIRD_PARTY_RUST_DEPENDENCIES.md](THIRD_PARTY_RUST_DEPENDENCIES.md) — all *unevaluated*.

## Undefined terms in the implementation directive

The directive refers to "approved defaults: Hybrid storage placement, Always Encrypt, Balanced Retention". **These were not defined anywhere in this repository or in earlier decisions.** Interim interpretation, to be confirmed by the owner:

| Term | Interim interpretation | Implemented |
|---|---|---|
| Hybrid storage placement | Metadata and records in the relational database; large artefacts in a content-addressed blob store on disk | Yes |
| Always Encrypt | Every stored artefact encrypted at rest with no opt-out | Blobs: yes. Database: **no** |
| Balanced Retention | Unknown | No — nothing is ever deleted at present |
