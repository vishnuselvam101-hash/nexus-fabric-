# ADR-002: Rust and native-library integration

**Status:** Proposed · 2026-11-18 · Spec: [01 §3–4](../specifications/01_SYSTEM_ARCHITECTURE_SPECIFICATION.md)

## Context
The trusted core is intended to be Rust. The candidate geometry libraries are C++ (OCCT, CGAL); automated-reading/OCR tooling is predominantly Python. C++ exceptions crossing FFI are undefined behaviour; kernels can abort or corrupt memory; LGPL components should remain user-replaceable.

## Options
1. **In-process FFI via binding generators** (`cxx`, `autocxx`, bindgen). Fast calls; a native fault takes down the host; binding generators struggle with template-heavy APIs (CGAL).
2. **Hand-written C ABI shim, in-process.** Narrow, auditable surface; exceptions caught in the shim; still shares the address space.
3. **Hand-written C ABI shim in a separate worker process**, coarse-grained operations (e.g. "intersect these surfaces", not per-point calls), serialised geometry across the boundary.
4. **Third-party Rust bindings** (e.g. `opencascade-rs`) — its README currently calls it "a major work in progress"; licence not stated there (to verify).

## Decision (proposed)
Option 3 as default for kernel operations; option 2 permitted for small, audited, hot-path primitives after review. Python is confined to Z1 workers and never linked into the core. No dependence on immature third-party bindings for release-critical paths.

## Advantages
Fault and security isolation; clean LGPL boundary (library replaceable without relinking our code); kernel version pinned and recorded per calculation; kernel swappable behind `uem-kernel-port`.

## Disadvantages
Serialisation overhead; we maintain the shim; debugging across processes.

## Risks
Chatty interfaces making IPC cost dominant; shim growing into a de-facto binding of the whole kernel.

## Validation required
- Spike: shim + worker for 5 representative operations; measure IPC overhead versus in-process.
- Reproducible builds of the kernel worker on Windows, Linux, macOS.
- Fault-injection test: kernel abort is reported as a step outcome.
- Rust does not make numerical code correct: all UEM algorithms still require the independent validation defined in Specification 05.
