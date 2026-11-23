# Verified Feature Inventory for Advertising

18 November 2026 · Source: the repository and the running application ([Implementation Status](../../../docs/IMPLEMENTATION_STATUS.md)). Classification as required by the campaign brief.

**Overall: the software is a development build. Nothing is validated for manufacturing; no result can be approved; no customer, factory result, benchmark or certification exists.**

| Advertised capability | Classification | What actually exists |
|---|---|---|
| Flat development: cylinder shell, concentric reducer, eccentric reducer, branch stub | **Implemented and tested** | Four families; reference values reproduced; several sub-cases refused |
| Header with set-on branch as a two-part assembly, with opening | **Implemented and tested** | One assembly type |
| Independent verification of results | **Implemented and tested** | Checks by a different formulation; interface check between parts. No second implementation or external kernel |
| Dependency tracking and scrutiny obligations after a correction | **Implemented and tested** (rules provisional, unreviewed) | Parameter corrections on the assembly only |
| DXF generation with re-import check | **Implemented and tested** | Basic entities; no CAM system tested |
| Traceable records, release closure, replay audit | **Implemented and tested** | Replay on the same machine |
| Project and document management, roles, audit log | **Implemented and tested** | Upload stores encrypted files; nothing is read from them |
| Dashboard, AMOEBA Control Centre, Verification centre | **Implemented and tested** | Live data; limited scope |
| On-premises, offline single binary | **Implemented, not fully tested** | Runs locally; no installer; security gaps open |
| "Investigate design inconsistencies" | **In development** | Interface check and obligations exist; hypothesis-driven investigation does not |
| AMOEBA architecture as a whole | **In development** | Records, edges, obligations, gates, replay built; scheduler, exploration, coverage, lanes not |
| Understand engineering drawings (PDF, scans, CAD), dimensions, annotations | **Designed, not implemented** | Specification 03 only |
| Automated drawing reading, OCR (Japanese/English) | **Designed, not implemented** | No technology selected |
| 2D-to-3D geometry reconstruction | **Designed, not implemented** | — |
| 3D visualisation | **Planned** | 2D pattern preview only |
| Rectangular-to-round and irregular transitions | **Designed, not implemented** | Specification 02 |
| Plate division and seam planning | **Designed, not implemented** | — |
| Manufacturing allowances, laser-cutting specifics, kerf | **Designed, not implemented**; needs factory calibration | — |
| Authorised engineering review / approval | **Designed, not implemented** | No job can reach APPROVED |
| Uncertainty and tolerance analysis | **Designed, not implemented** | — |
| Japanese user interface | **Planned** | — |
| Enterprise identity, multi-factor authentication, TLS, database encryption | **Planned** | See security register |

## Consequences for the films

1. Scenes for the first eight rows may use **real screen recordings**.
2. Everything else must be visibly marked as concept ("in development" / "planned") — in picture, not only in a closing line.
3. Six of the sixteen requested feature films are about capabilities that are not implemented (Automated Drawing Reading; PDF and CAD Ingestion beyond storage; 2D-to-3D Reconstruction; Plate Division and Seam Planning; Manufacturing Intelligence; and most of Reducer *and Transition* Development). They can only be concept films, or should wait.
4. Of the ten how-to videos, which must use real screens, five can be recorded now (creating a project; uploading a PDF — storage only; starting an engineering job; generating and inspecting DXF; understanding verification results). Five cannot: reviewing extracted dimensions, AMOEBA investigations, inspecting 3D geometry, exporting *approved* deliverables, and reviewing mathematical calculations beyond the result tables.
5. Never state or imply: customers, testimonials, performance figures, certification, guaranteed accuracy, manufacturing validation, prices.
