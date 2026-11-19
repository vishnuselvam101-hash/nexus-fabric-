# Frontend Architecture Audit and Stack Decision

19 November 2026 · Written after reading `web/`, the routes in `crates/nf-server/src/main.rs`, the three smoke tests and the two existing design documents, and after running the real application with data created through its API. Numbers are measured (`docs/design/tools/capture_screens.js`, medians of 7 cold loads on the development machine, loopback).

## 1. What was there

| Aspect | State before this work |
|---|---|
| Delivery | `web/index.html`, `web/app.js`, `web/styles.css` compiled into the server binary with `include_str!`; three routes; `Cache-Control: no-store` |
| Security envelope | `default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'`; same-origin; `X-NF-Request` anti-forgery header; no external resources |
| Client | One ES module, 306 lines / 39.3 kB, no dependencies, no build step. Hash router, one `async` function per view returning an HTML string, handlers attached afterwards |
| Rendering | The **whole shell** (navigation, top bar, content) rebuilt with `innerHTML` on every navigation and after every action |
| Geometry | One static `<svg viewBox>` per pattern; curve data only reachable through a `<title>` tooltip |
| Size / speed | 45.4 kB total, uncompressed. DOMContentLoaded 34 ms; first view rendered 92 ms (dashboard), 116 ms (heaviest job page, 892 DOM nodes). 0 console errors |
| Tests | `tests/ui_smoke.js`: one Playwright script, 12 assertions in one boolean |

## 2. Concrete problems found

| # | Problem | Consequence |
|---|---|---|
| A-01 | The engineering result was a read-only picture: no zoom, pan, fit, selection, coordinates or measurement (audit U-10, U-13) | The product's main output could not be inspected |
| A-02 | A job was one long scroll of nine cards (U-11); the pattern scrolled away from its checks and gates | No relation between geometry, inputs and verdicts on one screen |
| A-03 | "Engineering Workspace" was a navigation entry leading to a not-implemented notice while the real work lived under two other entries | Navigation did not match the work |
| A-04 | Shell rebuilt on every render | Focus, scroll position and any future long-lived widget are lost on each action |
| A-05 | Feedback was a single `#flash` bar prepended to the page; no loading state; an error replaced the page with one red line and no way to retry | Slow or failed requests look like a broken page |
| A-06 | Dependencies shown only as a three-column table (U-12); obligations and lessons only inside other pages | AMOEBA model present in data but not legible |
| A-07 | Tokens were ad-hoc (15 colours, no type or spacing scale, no states); contrast unmeasured (U-15) | Additions drift; accessibility unverified |
| A-08 | No keyboard shortcuts, no tooltips, icon-less navigation that cannot collapse; one breakpoint | Poor fit on a 1366 px laptop next to a large canvas |
| A-09 | `fmt(0)` printed `0.000e+0`; deactivating a user had no confirmation (U-18) | Small correctness and safety defects |
| A-10 | The UI test covered one path and could not tell which assertion failed | Regressions in other workflows invisible |

None of these is caused by the absence of a framework. They are missing features and missing structure.

## 3. Stack options evaluated

Constraints that any option must meet: single self-contained binary; strict CSP unchanged; works offline in a factory network; no secret or third-party code path in a product whose value is verifiability; small enough for one engineer to audit.

| Option | Embedded binary / CSP | Maintenance | Licence | Size | Security | Verdict |
|---|---|---|---|---|---|---|
| **React 18/19 + TypeScript** | Works with CSP once pre-built (no inline script needed), but requires a Node toolchain (bundler, tsc) in the build and CI of a Rust workspace; `include_str!` would embed generated files that are no longer the reviewed source | Adds package.json, lockfile, bundler config, upgrade cadence | MIT | ≈45 kB gzip runtime (react + react-dom) before any application code; today the whole client is 44 kB gzip | ≈ 5–10 direct, several hundred transitive npm packages at build time: a supply chain to audit for an offline engineering product | **Rejected.** 16 views and one canvas do not need a virtual DOM; it would add a build system to solve problems (A-01…A-10) it does not address |
| **TypeScript alone** (`tsc` or JSDoc + `checkJs`) | Fine; output is plain modules | One dev dependency; a compile step or editor-only checking | Apache-2.0 | 0 at run time | Build-time only | **Not adopted now, first candidate later.** Worth it when the client passes ≈2 000 lines or a second developer joins; JSDoc `checkJs` needs no build step |
| **Tailwind CSS** | Needs its compiler (JIT) at build time; the CDN build is forbidden by CSP and policy | Class strings inside JS template literals are hard to review; design tokens end up in a config file instead of CSS | MIT | 10–30 kB CSS generated | Build-time dependency tree | **Rejected.** The design system is ≈60 custom properties in one 33 kB stylesheet |
| **shadcn/ui + Radix** | Require React | Copy-in components, each pulling Radix packages | MIT | per component | — | **Rejected with React.** What we need from them (dialog focus trap, tabs, tooltips) is provided here by native `<dialog>`, ARIA tab pattern (≈15 lines) and CSS |
| **Three.js / React-Three-Fiber** | Works under CSP; WebGL allowed | Large API surface | MIT | ≈170 kB gzip (core) | Low | **Rejected.** The engine's output is 2D developed patterns of at most a few thousand vertices. SVG gives exact hit-testing, crisp strokes at any zoom, accessibility hooks and print for free. A 3D view of the folded surface has no backend data today; adding a renderer for it would be decoration |
| **Lucide icons** | Fine as inline SVG | 1 500 icons, we need 33 | ISC | tree-shaken only with a bundler | Low | **Rejected.** 33 hand-drawn icons in one consistent style (16 px grid, 1.5 px round stroke) live in `app.js` as path strings: ≈4 kB, no dependency |
| **Zustand** | Requires a bundler in practice | — | MIT | ≈1 kB | Low | **Rejected.** Client state is: the signed-in user, the last summary, the active viewer, three workspace fields and a notification list. The server is the store; views refetch |
| **Vitest** | Dev only | Node test runner + config | MIT | 0 at run time | Dev-time tree | **Rejected for now.** Logic worth unit-testing is on the server (Rust tests). The client is verified end-to-end in a real browser by `tests/ui_smoke.js` (83 checks). If pure client logic grows (e.g. the viewer's maths), `node --test` covers it with zero dependencies |

## 4. Decision

**Stay dependency-free: native ES modules, hand-written CSS with custom properties, inline SVG. No framework, no bundler, no npm dependency at run time or build time.** The redesign is delivered inside the existing architecture with three structural changes:

1. **Two modules instead of one.** `web/app.js` (shell, router, views, 843 lines) and `web/viewer.js` (the pattern viewer, 231 lines, no knowledge of the application). The server gained exactly one route, `/viewer.js`, served through the same `asset()` function and security layer as `app.js`. CSP and every header are unchanged.
2. **Persistent shell.** The shell is mounted once per session; navigation replaces only `<main>`. Views return `{ title, crumbs, html, wire }`; a sequence number still guards against a slow response overwriting a newer view; re-renders after an action keep scroll positions.
3. **One design-token layer** (`web/styles.css`, documented in `DESIGN_SYSTEM.md`) with measured contrast.

Reasons, in order of weight: (a) the binding constraints — one binary, strict CSP, offline, auditable — are met by construction today and every option above weakens at least one of them; (b) measured cost of the current approach after the redesign is 148 kB uncompressed (44 kB if the server compressed) and a first view in ≈117 ms; a React baseline alone would exceed that; (c) no listed dependency addresses the actual problems in §2; (d) a migration "for looks" would have consumed the effort that went into the viewer and the tests.

When to revisit: client above ≈2 500 lines or three modules with shared state → add JSDoc `checkJs` first, then consider a typed build; a real need to render folded 3D surfaces **with server data for them** → evaluate a small WebGL layer for that view only; more than one frontend developer → add a formatter and a zero-dependency `node --test` suite for `viewer.js`.

## 5. What changed, problem by problem

| Problem | Resolution |
|---|---|
| A-01 | `viewer.js`: wheel zoom at the cursor, drag pan, fit, keyboard control, adaptive mm grid and rulers, extent dimensions computed from the vertices, hover read-out of the curve's engine-computed length and nearest real vertex, click selection with a property sheet, two-point measurement that snaps to computed vertices |
| A-02, A-03 | The job page is the Engineering Workspace: inputs/properties panel · canvas · results panel (Gates, Checks, Scrutiny, Graph, History, Records) with resizable, persisted panel widths; "New calculation" uses the same layout; the navigation entry lists jobs |
| A-04, A-05 | Persistent shell; skeleton after 200 ms; error state with Retry; progress line while requests run; connection state in the status bar; toasts with a session log; inline field errors |
| A-06 | Layered dependency graph drawn from the real edge list with upstream/downstream tracing; AMOEBA views for records, activities, obligation ledger, lessons, dependencies, mechanisms |
| A-07 | Token system, type and spacing scales, state rules; contrast measured (see `DESIGN_SYSTEM.md`) |
| A-08 | Collapsible icon rail, shortcut set with `?` overlay, tooltips on every icon control, breakpoints at 1500 / 1180 / 1100 / 820 px |
| A-09 | Fixed; deactivation needs a second explicit click |
| A-10 | 83 named checks, exact pass/fail count, covering every critical workflow, zero-console-error and overflow assertions at 1366, 1024 and 768 px |

## 6. Measured before / after

| Metric | Before | After |
|---|---|---|
| Assets (uncompressed; server sends no compression) | 45.4 kB (html 0.3, js 39.3, css 5.7) | 147.9 kB (html 0.7, js 99.1 + 15.2, css 33.0); ≈44 kB if gzip were enabled |
| Requests to first view | 3 static + 3 API | 4 static + 5 API |
| DOMContentLoaded (median) | 34 ms | 46 ms |
| First view rendered — dashboard | 92 ms | 117 ms |
| First view rendered — heaviest job | 116 ms | 124 ms (124–138 across runs) |
| DOM nodes — dashboard / job | 164 / 892 | 355 / 1 402 |
| Console errors | 0 | 0 |
| UI test assertions | 12 (one boolean) | 83 named checks |

The client is three times larger and 10–25 ms slower to first view. That is the cost of the viewer, the graph, the shell and the states; it stays far below any budget that matters on a local network. Enabling response compression in the server would recover most of the transfer size; it was not done here because it is outside the minimal server change allowed for this work.

## 7. Known limits of the frontend (honest list)

Touch gestures (pinch zoom) are not implemented in the viewer · curve hit-testing is pointer-based; the keyboard path to a curve is the curve table · measurement is planar distance in the development plane only — no arc-length-along-curve, angle or area tools · job lists are whatever the API returns (capped at 200 by the server), filtered client-side, no paging · the dependency graph uses a simple layered layout without crossing minimisation beyond one barycentre pass · Japanese interface still missing (U-14) · no client unit tests (browser test only) · static assets are sent uncompressed with `no-store`.
