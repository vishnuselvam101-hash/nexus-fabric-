# UI/UX Audit

18 November 2026 · Reviewed by walking the real application as an engineer would (create project → upload → calculate → inspect → correct → resolve → administer) at 1360 px and 820 px, light and dark, keyboard only for the main flow. A self-review by the implementing system; no user research with engineers has been done.

## What was already sound

No decorative gradients, glass effects, hero sections, fake charts or invented metrics · every number on screen comes from the API · unimplemented features are labelled, not mocked · release state and reasons are prominent banners, not small badges · dense tables with monospaced, right-aligned numbers.

## Findings and actions

| # | Finding | Severity | Action | State |
|---|---|---|---|---|
| U-01 | Engineer decisions were collected through browser `prompt()` / `alert()` dialogs | High | Inline reason field per obligation; inline error region | Fixed, browser-tested |
| U-02 | No visible keyboard focus; no skip link; current page not exposed to assistive technology | High | `:focus-visible` outline, skip link, `aria-current`, focus moved to content on navigation, page title per view | Fixed |
| U-03 | Form labels not programmatically associated with inputs | Medium | `for`/`id` on generated fields and admin forms | Fixed for main forms |
| U-04 | No way to manage one's own account | High | Account page with password change | Fixed |
| U-05 | Access rules invisible | Medium | Project page shows members; administration shows the running security configuration and audit-chain state | Fixed |
| U-06 | Pill-shaped badges and soft rounded cards read as a generic dashboard | Low | Radii reduced to 3–4 px; status shown as flat rectangular tags; alert bars carry a left rule | Fixed |
| U-07 | Table headers scroll away on long tables | Low | Sticky headers | Fixed |
| U-08 | No print style for a job (engineers print) | Low | Print stylesheet hides chrome | Fixed (basic) |
| U-09 | Motion preferences ignored | Low | `prefers-reduced-motion` respected (the UI has almost no motion) | Fixed |
| U-10 | Pattern preview cannot be zoomed, panned or measured; no entity selection | **High** | — | **Open** |
| U-11 | Job page is one long scroll: request, patterns, checks, obligations, gates, edges, records | Medium | — | Open: needs sections/tabs after seeing real use |
| U-12 | Dependency edges shown as a table, not a graph | Medium | — | Open |
| U-13 | Engineering Workspace (drawing viewer, 2D/3D) does not exist | **High** | — | Open: blocked on parser and rendering decisions |
| U-14 | No Japanese interface | **High** for the first market | — | Open: vocabulary needs native review |
| U-15 | Colour contrast not measured with a tool; state is never colour-only (text always present) | Medium | — | Open: measure both themes |
| U-16 | No notifications, no job queue view | Medium | — | Open: no backend for it |
| U-17 | Correction flow navigates to the new job without summarising what changed | Medium | — | Open |
| U-18 | No confirmation step before deactivating a user | Low | — | Open |

## Steps for the common workflow (measured)

New single-part development from the dashboard: 3 clicks + parameter entry. Assembly correction with one obligation resolved: 4 clicks + three fields. Download of a DXF: 1 click from the job.

## Not evaluated

Long documents and large datasets (no viewer; job lists capped at 200) · performance under load · real users.
