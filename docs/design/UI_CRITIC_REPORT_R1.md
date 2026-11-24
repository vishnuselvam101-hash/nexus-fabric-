# UI Critic Report — Round 1

19 November 2026 · Independent review of the redesigned NEXUS FABRIC web client (commit `ee3db2d`). The reviewer did not build this client.

## Method

- Ran `target/release/nf-server` on its own port, 127.0.0.1:8795, with an empty data directory, then loaded `docs/design/tools/seed_demo.js`. Stopped the server afterwards.
- Drove the running app with Playwright (Chromium, headless) and took screenshots at 1920×1080, 1366×768, 1024×768 and 768×1024. I read every screenshot cited in this report.
- **Workflows tested:**
  - sign in, including a wrong password
  - create a project, with an empty name and then a valid one
  - new calculations: header-with-branch, concentric reducer, eccentric reducer and cylinder
  - invalid inputs: an empty field, a negative radius, branch radius larger than header radius, and r_dev = 1e9
  - pattern canvas: hover read-out, click-select, wheel zoom, drag pan, F / + / − / arrow keys, the Fit button, two-point measure with snapping, switching parts with the tab and with key `2`
  - all six results tabs
  - DXF download, then inspection of the file contents
  - recording a correction, with an empty value and then a valid one
  - resolving an obligation, with an empty reason and then a reason
  - replay audit
  - keyboard: `?` overlay, Tab order on the dashboard and on a job, `g w` navigation, tab-row arrow keys, keyboard resize of the splitter, `[` to toggle the rail
  - empty project, filter with no matches, unknown job id, unknown route
  - light theme
  - signed in as the viewer role
- **Measured:**
  - text contrast across four views in both themes
  - smallest font sizes
  - load time and frame time
  - horizontal overflow at each width
- **Compared against:**
  - the before screenshots in `docs/design/screenshots/before/`
  - `docs/design/DESIGN_SYSTEM.md`

## Scores (1–10)

| Area | Score | Justification |
|---|---|---|
| Visual quality | 7 | The dark graphite shell, the 1 px rules, tabular monospace numbers and the canvas with rulers, extent dimensions and scale bar are coherent and calm. Several weaknesses hold it back: unformatted full-precision numbers, a native file input, native validation bubbles, a cramped dependency graph clipped in the side panel, and toasts that cover content. |
| Professional engineering credibility | 5 | Several things contradict the gate story: the dashboard counts superseded jobs as "review required", a superseded pattern is watermarked "REVIEW REQUIRED", a negative radius becomes a persisted REVIEW REQUIRED job with a green success toast, and absurd sizes are accepted. The UI also shows unit-less quantities with up to 12 significant figures. |
| Usability | 6 | The viewer interactions are good: snapping measure, a hover read-out and keyboard parity. Around it, the workflows are weak: no parameter sketch, the run button sits below the fold, the correction form is buried at the bottom of a scrolling side panel, there is no diff after a correction, DXF downloads one part at a time, and a failed sign-in clears the username. |
| Information architecture | 5 | The same generic jobs table appears on Dashboard, Workspace, Manufacturing and Verification, without the columns each page needs. "Project 1" is shown instead of project names. Jobs with no pattern are listed under "Jobs with DXF output". The breadcrumb, rail and page title disagree. Records are listed as eleven identical "claim · parameter" rows. |
| Consistency | 6 | Tokens and tags are applied consistently. The stated rules are broken in visible places: native validation bubbles where the system requires inline errors, a filter with no matches that has no "no matches" state, three words for the replay result (AUDITED, PASS, "reproduced the closure"), raw snake_case codes next to human labels, and three different decimal precisions for the same lengths. |
| Accessibility | 7 | Focus rings are visible and consistent, text contrast passed the automated scan in both themes, there is a skip link, tabs follow ARIA conventions, the splitters work with keys, every icon button has a name, and reduced motion is respected. It loses points for 9–10.5 px text on graph labels, rulers and tags, Tab escaping the shortcut dialog to `body`, the native browser validation popups, and no forced-colors pass. |
| Performance | 9 | A job workspace loads in about 105 ms. Zoom and hover hold one frame (16.5 ms) per event. The assets total 148 KB with no third-party code, and a job computes in about 64 ms. |
| Responsiveness | 6 | There is no horizontal page overflow at any width, and the workspace stacks sensibly at 1024. At 768 px the New calculation page puts about 560 px of empty canvas above the inputs. At 1366 the job header truncates the "corrects job #N" provenance link and the curve table clips its LENGTH column. At 768 the jobs table clips columns without any scroll affordance. |

**Summary gates**

- **Visual design: 7/10**
- **Engineering workflow usability: 5/10**

## Did every critical workflow work?

**Mechanically, yes.** Every action I tried completed:
- sign-in, project creation, calculations of four families including header-with-branch
- canvas zoom, pan, fit, hover, select and measure; part switching; all six tabs
- DXF download (`NOT-RELEASED_REVIEW_REQUIRED_calc10_header_….dxf`, layers CUT_OUTER and CUT_INNER)
- correction (it created job #13 and raised 11 obligations, 3 of them open)
- obligation resolution, with the inline error shown for an empty reason
- replay audit (PASS)
- help overlay, theme switch, and viewer-role restrictions

There were no console errors apart from the expected 401 before sign-in and 404 for a missing job.

**Three workflows produce wrong outcomes**, and an engineer would count these as failures:

1. **Invalid input is not rejected; it becomes a job.**
   - Steps: New calculation → Assembly: header with branch → Header contact radius `-150` → Develop.
   - Result: Job #9 is created and stored as **REVIEW REQUIRED** with a green toast, "Job #9 computed: REVIEW REQUIRED". The gates read "1 PASS · 1 INCONCLUSIVE". The job then appears on the dashboard, in Manufacturing under "Jobs with DXF output", and in the review-required count.
   - The same happens with branch radius 400 > header radius 150 (job #14, which also shows a spurious "repeats something that went wrong before" advisory).
2. **Dashboard state counts are wrong.**
   - Steps: after any correction, open the Dashboard.
   - Result: the stat strip shows REVIEW REQUIRED 13 of 13, and the release bar shows "13 review required · 0 rejected or superseded". The table directly beside it shows two SUPERSEDED jobs (#6 and #10).
3. **A superseded job is labelled as under review on its own drawing.**
   - Steps: open `#/calc/6`.
   - Result: the header and banner say SUPERSEDED and the DXF is withheld. The canvas watermark still reads "NOT RELEASED · REVIEW REQUIRED", the pattern is drawn at full strength, and "Run replay audit" and "Record a correction" are still offered.

A minor finding: after a failed sign-in, the username field is cleared and the browser's "Please fill out this field" bubble appears.

## Before and after

The before build was a generic light admin page: unformatted request key/value tables and a bare SVG outline with no rulers, scale or interaction. The redesign is a large step up. It has a real three-pane engineering workspace, a measurable pattern viewer, gates and checks next to the geometry, provenance and obligations, and a build notice that is always on screen.

Most remaining problems are not styling. They are about whether state and numbers can be trusted, and about the workflow around the canvas.

## Defects, worst first

### Critical

1. **Invalid geometry is stored as a reviewable engineering job.**
   - **Where:** New calculation → Develop; then the job view, dashboard and Manufacturing.
   - **What's wrong:**
     - Negative radii and impossible intersections (|δ| + r ≥ R) are sent to the server and stored as immutable jobs.
     - They are labelled REVIEW REQUIRED, with gates such as "0 PASS · 2 INCONCLUSIVE", and announced with a green success toast.
     - These jobs are counted as pending review, listed as "Jobs with DXF output", and offered "Record a correction" and "Run replay audit".
   - **Fix:**
     - Validate domain constraints on the client before submit (positive radii, branch ≤ header, |δ| + r < R, station within length, angle range), with an inline error on the offending field. The server should return 400 for these rather than create a job.
     - If a no-pattern job must still be recorded, give it its own state: REJECTED, or "NO RESULT — input out of range". Show it with a red toast, exclude it from review counts and DXF lists, and hide the correction and replay actions.
2. **The dashboard contradicts itself on release state.**
   - **Where:** the dashboard stat strip "REVIEW REQUIRED" and the "Release state of all jobs" bar.
   - **What's wrong:** superseded jobs are counted as review required, and "0 rejected or superseded" is shown beside two SUPERSEDED rows.
   - **Fix:** derive every count from one function that uses the same state precedence as the tags (superseded > rejected > review required). Show superseded as its own neutral segment in the bar and its own stat tile.
3. **The canvas watermark on superseded jobs says "REVIEW REQUIRED".**
   - **Where:** the job workspace canvas for a superseded or rejected job.
   - **What's wrong:** the drawing carries a different state from the header. The pattern is drawn at full strength, and the replay and correction actions remain.
   - **Fix:**
     - Bind the watermark to the job's effective state ("SUPERSEDED — DO NOT USE").
     - Dim the strokes to about 40 %, with a diagonal hatch or a red frame.
     - Disable or hide correction and replay on superseded jobs, and put "Open current job #N" as the primary action in the banner.

### Major

4. **No geometry sketch or parameter legend on New calculation.**
   - **Where:** New calculation, especially for header-with-branch.
   - **What's wrong:** nine numeric fields (β, δ, station, s_end, contact vs development radius) have no figure showing what each dimension means. The empty canvas sits idle until after the run, and the Develop button is below the fold at 1920×1080.
   - **Fix:** draw a schematic per family in the centre canvas with dimension callouts that highlight as each field is focused, plus a live preview where it is cheap. Make the run button sticky at the bottom of the inputs pane.
5. **Engineering quantities have no units and inconsistent precision.**
   - **Where:** Pattern quantities, the Selection panel, Checks.
   - **What's wrong:**
     - Values such as `area 1130973.35529`, `developed angle 1.86189128922` (radians, unlabelled), `rho at 180 721.110255093` and `s at 0 129.099444874` have no units and mix lowercase raw keys with capitalised labels.
     - The same length shows as 3 decimals in the table, 9 in Selection and 12 in Checks.
     - Checks mix `1.421e-13` with `942.477796077`.
   - **Fix:**
     - Add a unit column or suffix on every quantity (mm, mm², rad and deg, with degrees shown alongside).
     - Use one display precision (3 dp for mm) and show full precision on hover or copy.
     - Use thousands separators for areas, and humanise every key.
6. **No difference view after a correction.**
   - **Where:** the corrected job (for example #13, "corrects job #10").
   - **What's wrong:** an engineer cannot see what changed: which parameter, which curve lengths, by how much. They must open two tabs and compare by eye.
   - **Fix:** add a "Changes from job #N" section to the job: parameter before → after, a Δ for each curve length and extent, and an optional overlay of the previous pattern in a ghost colour on the canvas.
7. **The correction workflow is buried.**
   - **Where:** the left pane of a job, below Request, Selection, Curves, Pattern quantities and Assumptions.
   - **What's wrong:** the main corrective action sits about 1.5 screens down a scrolling side panel. Its parameter select uses raw codes ("beta_deg (now 60)") while the rest of the page uses labels.
   - **Fix:**
     - Add a "Record correction" button in the job header that opens a dialog or drawer.
     - Allow "Correct…" from each row of the Request list, pre-selecting that parameter.
     - Use human labels with the code as secondary text.
8. **Manufacturing, Verification, Workspace and Dashboard all reuse one generic jobs table.**
   - **Where:** those four pages.
   - **What's wrong:**
     - Manufacturing lists 13 "jobs with DXF output", including jobs with no pattern and jobs whose DXF is withheld, with no download links, parts, file sizes or hashes.
     - Verification adds nothing beyond the shared table.
   - **Fix:**
     - Give Manufacturing one row per part: part, layers, extents, DXF hash, download, withheld reason.
     - Give Verification gate counts and open checks per job.
     - Show only jobs that actually have output.
9. **DXF is downloaded one part at a time, and the file carries no in-drawing marking.**
   - **Where:** the canvas toolbar "DXF · not released"; the DXF itself.
   - **What's wrong:**
     - An assembly needs a separate download for each part.
     - The file (R12, AC1009) gives units only in a `999` comment.
     - The file has no TEXT entity carrying job, part, state or "NOT RELEASED", so the warning is lost once the file is renamed in CAM.
     - Both layers use colour 7 even though the UI distinguishes blue and amber.
   - **Fix:**
     - Add "Download all parts (.zip)".
     - Write a MARK or TEXT layer with job, part, state and hash, plus a visible NOT-RELEASED annotation.
     - Use distinct layer colours (CUT_OUTER 5, CUT_INNER 2) to match the viewer.
10. **Inputs have no plausibility guard.**
    - **Where:** New calculation.
    - **What's wrong:** r_dev = 1e9 mm (a plate 6,283 km wide) is accepted silently. The y-ruler labels are then clipped to "0000000" and a DXF is offered.
    - **Fix:** show soft warnings above per-family plausible ranges (for example radius > 10 m), requiring confirmation. Abbreviate ruler labels (k, M) or widen the gutter.
11. **Projects appear by number ("Project 1") in the breadcrumb, job header and AMOEBA snapshots.**
    - **Where:** job breadcrumb, the job meta line, AMOEBA "Recent project snapshots", Verification lessons.
    - **What's wrong:** the meaningful name ("Header H-204 spool study") appears only in tables.
    - **Fix:** always render the project name, with `#id` as secondary text.
12. **The dependency graph is clipped in the job side panel.**
    - **Where:** job → Graph tab at 1920 and below.
    - **What's wrong:** the third column ("intersec…", "PART branch") is cut off with no scroll affordance. The edges are a tangle of thin, low-contrast curves, and node-type labels are 9 px.
    - **Fix:** fit the graph to the panel width or use a vertical layer layout. Make it pannable with a visible scroll shadow, bundle edges, and use at least 11 px labels.
13. **Records are not identifiable without expanding each one.**
    - **Where:** job → Records tab.
    - **What's wrong:** eleven identical rows read "claim · parameter <hash>".
    - **Fix:** show the subject in the row (`parameter · header_radius = 142 mm`) with the hash right-aligned and muted.
14. **New calculation at 768 px wide.**
    - **Where:** the `#/math` view at 768×1024.
    - **What's wrong:** an empty "No pattern computed yet" canvas of about 560 px comes before the inputs, so the form is below the fold.
    - **Fix:** below 1100 px, put the inputs first and collapse the empty canvas to a one-line note, or hide it until there is a result.
15. **Native browser validation bubbles.**
    - **Where:** Sign-in, New project.
    - **What's wrong:** the Chrome "Please fill out this field" popup contradicts the design system rule (inline message, `aria-invalid`, red border). New calculation, correction and obligation forms already do this correctly.
    - **Fix:** add `novalidate` and use the shared `fieldError()` everywhere.
16. **A failed sign-in clears the username.**
    - **Where:** Sign-in.
    - **What's wrong:** after "invalid username or password", the username field is empty. The message is also a lowercase raw server string.
    - **Fix:** keep the username, focus and select the password, and write "Incorrect username or password."

### Minor

17. **A filter with no matches shows nothing.**
    - **Where:** Projects, jobs tables.
    - **What's wrong:** typing a non-matching term leaves only the header row, the count still says "3", and the column widths jump.
    - **Fix:** show "No projects match 'zzzz' · Clear filter", update the count to "0 of 3", and fix column widths.
18. **The job header truncates provenance at 1366.**
    - **Where:** job header meta line.
    - **What's wrong:** "corrects job #6" is cut to "correc…", hiding the most important link.
    - **Fix:** move supersedes / corrects into a tag next to the state, or wrap the meta line.
19. **The left pane clips at 1366 and after resizing.**
    - **Where:** Curves table and Pattern quantities.
    - **What's wrong:** the "LENGTH (MM)" header is clipped, "Curve representati on error" breaks mid-word, and the part tab "branch E-C" is cut by the toolbar.
    - **Fix:** set a minimum pane width (about 280 px), use `hyphens:auto` or shorter labels, and let the part tabs overflow into a menu.
20. **The status bar read-out truncates at 1920.**
    - **Where:** under the canvas.
    - **What's wrong:** "from bottom vertex 1 to bottom verte…" is cut despite space being available.
    - **Fix:** let the read-out take the free width, or put measurement results in the Selection panel as a "Measurement" block.
21. **Selection and measurement use colours that carry no state meaning.**
    - **Where:** canvas.
    - **What's wrong:**
      - A selected curve turns white and loses its layer colour.
      - The measure label is green, the `--ok` state colour, which contradicts "colour means state".
    - **Fix:** draw the selection as a thicker stroke in its layer colour plus an accent halo, and draw the measurement in `--accent`.
22. **Toasts stack over content and linger.**
    - **Where:** bottom-right of every view.
    - **What's wrong:** two or three toasts cover the gate list and obligation items for several seconds and survive navigation.
    - **Fix:** limit to one toast and replace it, auto-dismiss success toasts after 4 s, and offset them above the status bar outside the results pane.
23. **The replay result has three names.**
    - **Where:** jobs tables ("AUDITED"), the Records tab ("PASS"), the toast ("reproduced the closure").
    - **Fix:** use one term, for example "REPLAY MATCH" with ✓.
24. **Checks tab layout.**
    - **Where:** job → Checks.
    - **What's wrong:**
      - The label/value pairs interleave as "Primary 1.421e-13 Reference 0", which is hard to scan.
      - The tab has no summary count, unlike Gates.
      - Checks are not filtered by the selected part.
    - **Fix:** use a small two-by-two grid with labels above values, add a count strip (n PASS), and filter by the active part with an "All parts" toggle.
25. **The request list order differs from the form order.**
    - **Where:** job → Request (alphabetical) vs New calculation (logical order).
    - **Fix:** use the family's field order in both places.
26. **Unknown routes and job ids.**
    - **Where:** `#/nonsense` redirects silently to Dashboard; `#/calc/9999` shows "not found" with a primary **Retry**.
    - **Fix:** show a "Job #9999 does not exist" empty state with "All jobs" as the action. Show a 404 view for unknown routes.
27. **A tooltip sticks after a click.**
    - **Where:** top-bar Theme button ("Theme T").
    - **What's wrong:** the tooltip stays over "Run replay audit" until the pointer moves.
    - **Fix:** hide tooltips on click and on route change.
28. **The shortcut dialog's focus loop.**
    - **Where:** the `?` overlay.
    - **What's wrong:** Tab alternates between the Close button and `body`.
    - **Fix:** make the dialog body focusable, or trap focus on its focusable elements explicitly.
29. **Small type.**
    - **Where:** graph node kinds (9 px), graph column heads (9.5 px), rulers and watermark (10 px), tags and table heads (10.5 px), rail badge (9 px).
    - **Fix:** use at least 11 px for any text an engineer must read, and 10.5 px only for uppercase tags.
30. **Unstyled native file input.**
    - **Where:** Project → Source documents.
    - **Fix:** use a styled "Choose file…" button plus a file name label, consistent with the other controls.
31. **Project member management is free text.**
    - **Where:** Project → Access.
    - **What's wrong:** you type a username to add or remove, and usernames are shown uppercased in tags (identifiers are case-sensitive).
    - **Fix:** use a user picker, a per-member remove (×) with confirmation, and the original casing.
32. **Redundant columns.**
    - **Where:** Project page jobs table.
    - **What's wrong:** a "Project" column repeats the current project on every row.
    - **Fix:** drop the column inside a project; add parts, gates and a "no pattern" indicator instead.
33. **Dates are US-locale.**
    - **Where:** throughout ("Nov 19, 2026, 2:20 AM").
    - **Fix:** use ISO 8601 (`2026-11-19 02:20`), which fits engineering records and the planned Japanese locale.

## What is good (for calibration, not padding)

- The pattern viewer:
  - snaps the measure tool to computed vertices
  - shows a hover read-out with vertex index
  - zooms at the cursor
  - matches mouse controls with keys (F, +/−, arrows, M, 1–9, Esc)
  - runs smoothly at 60 fps
- The build-status notice is always visible and never noisy.
- Obligations require a reason, give an inline error, and record the name.
- Keyboard focus is visible throughout.
- Text contrast passed in both themes.

These are close to professional-tool quality. What keeps the product from 8 or more is the numbers and state around the canvas, not the canvas itself.
