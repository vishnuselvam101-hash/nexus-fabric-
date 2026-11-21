# ADR-007: Desktop, tablet and mobile architecture

**Status:** Proposed · 2026-11-18 · Spec: [01 §5](../specifications/01_SYSTEM_ARCHITECTURE_SPECIFICATION.md)

## Context
Engineers need a dense workstation UI with large 2D drawings and 3D models; reviewers and approvers need tablet/mobile access; factories may be offline. All calculations must be identical regardless of device.

## Options
1. **Separate native apps per platform.** Best platform fit; three codebases; divergence risk.
2. **Fully native desktop (Rust/C++ UI) + separate mobile.** Maximum rendering performance; slow UI iteration; small talent pool.
3. **Shared web-technology UI:** desktop in a shell (with native integrations for files, local backend, GPU), tablet/mobile as responsive web or wrapped app; all clients thin — authoritative computation only in the backend; rendering via WebGL/WebGPU with server-prepared tessellations and tiled drawing renderings.

## Decision (proposed)
Option 3. Clients never compute release geometry. Offline mode is read/annotate/draft only; offline artefacts are labelled unsynchronised and cannot be Approved.

## Advantages
One UI codebase and one localisation system; no geometry code on clients (which also keeps LGPL/GPL native libraries off app-store binaries); consistent results.

## Disadvantages
Large-drawing and large-model rendering in web technology must be proven; dependence on backend reachability for anything authoritative.

## Risks
Performance on very large vector PDFs; WebGPU availability on customers' managed devices; approval from mobile needs strong authentication and an unambiguous view of what is being approved; desktop shell choice has its own security and licence profile.

## Validation required
- Rendering spike with the largest authorised drawings and models (open requirement: samples needed).
- Shell candidates compared (footprint, security model, WebView consistency on Windows, which is presumed the dominant factory OS — to confirm with the factory).
- Approval UX and step-up authentication reviewed in Specifications 07 and 08.
- Device and OS inventory from pilot factories.
