# NEXUS FABRIC bug hunt, round 8 (BH-R8-001 to BH-R8-010)

Scope: functional bugs only in the simplified UI (`web/`): broken flows, wrong or missing text in Japanese mode, console and page errors, failed requests, layout that cannot be read or used at 390px, WebGL and reduced-motion fallbacks, the job export lock. No restyling. The visual redesign is not touched.

## 1. Setup

- Branch `ui/fixes` in `/root/nf-wt-fix`, from `ui/apple` 883cd3f. Fix commits: 6e4d418, ad3a3b3, ca538a7, 15104c5.
- Binary: `cargo build --release -p nf-server` plus `cargo build --release -p nf-pdf` (the converter `nf-pdf2dxf` beside it), built inside `nf-sandbox`. Debug `nf-pdf2dxf` built with `cargo build -p nf-pdf` for `cargo test`.
- Server: port 127.0.0.1:8796 only, own temp data dir, started under `timeout 900` inside `nf-sandbox`. Seeded with `docs/design/tools/seed_demo.js` (a second server run with no seed for the first-run path).
- Browser: Playwright Chromium (`/root/node_modules/playwright`), viewports 1440x900 and 390x844, EN and JA, dark and light (`nf.theme`, `nf.lang` set before load).
- Scripts and raw output (not in the repo): `/root/nf-logs/bughunt-r8/` (`stage_run.sh`, `scripts/`, `*.jsonl`, `*.txt`).
- Evidence screenshots: `/root/nf-logs/bughunt-r8/shots/` (`<combo>-<route>.png`, the final run) and `/root/nf-logs/bughunt-r8/probe_setup.png` (BH-R8-001 before the fix).
- Host load was 13 to 20 from other processes during the run. Some browser checks timed out under that load (see BH-R8-010).

## 2. Summary

Ten items: **Critical 1, High 1, Medium 6 (4 fixed, 2 not fixed), Low 2 (not fixed).**

1. **BH-R8-001 (Critical, fixed):** a fresh install shows Sign in with no way to create the first administrator.
2. **BH-R8-002 (High, fixed):** after the setup form is submitted the account is created but the user is not signed in, and a page error is thrown.
3. **BH-R8-004 (Medium, fixed):** history, projects, audit and user tables and several workspace buttons are English in Japanese mode.
4. **BH-R8-005 (Medium, fixed):** at 390px the history and projects tables and the AMOEBA page are wider than the screen (page scrolls sideways; 561, 500 and 777 px).
5. **BH-R8-006 (Medium, fixed):** with WebGL unavailable every page with the steel stage logs three.js errors.
6. **BH-R8-007 (Medium, not fixed):** engine and server prose (job provenance, admin security, audit details) stays English in Japanese mode.

## 3. Defects

### BH-R8-001: Critical: a fresh install has no setup form

- Where: sign-in screen (`#/login`) on an empty server (no administrator yet). `web/js/app.js` boot, `web/js/views/login.js`.
- Steps: 1) Start a new server with an empty data folder. 2) Open the page.
- Expected: the setup form ("Setup", setup token field, "Create account") is shown.
- Actual: the sign-in form is shown. `setupRequired` was never set (only `health.setup_required` was stored), so the login view never switched to setup mode. The server log says "First-run setup: no administrator exists yet" and the sign-in attempts fail.
- Evidence: `/root/nf-logs/bughunt-r8/probe_setup.png` (before the fix). Reproduced 1 of 1.
- Status: **fixed** in 6e4d418 (boot sets `setupRequired` from `/api/health`). Test: `tests/ui_smoke_v2.js` section 2b, "first run: the form is the setup form (heading, token field)"; run on an empty server (32 passed, 0 failed on the final code; one earlier empty-server run had 1 failure in the unrelated job-details check, see BH-R8-010).

### BH-R8-002: High: setup does not sign the administrator in

- Where: setup form submit, `web/js/views/login.js`.
- Steps: 1) Empty server. 2) Fill username, password (15 characters) and setup token. 3) Click "Create account".
- Expected: the administrator is created and the convert screen opens signed in.
- Actual: the account is created, `/api/me` returns 401, the form view is left, and the submit handler throws `Cannot read properties of null (reading 'prepend')` (page error). The page stays on sign-in.
- Evidence: reproduced on the intermediate build with `scripts/probe_setup2.js` (output in this session). Not reproduced after the fix (`probe_setup2.js` run: "after setup url=#/", "errors []").
- Status: **fixed** in 6e4d418 (setup posts `/api/setup`, then `/api/login` with the same credentials; `showError` returns when the view is gone). Test: section 2b, "first run: creating the administrator signs in and lands on convert".

### BH-R8-003: Medium: setup texts overwritten or English in Japanese

- Where: setup mode of `web/js/views/login.js`.
- Steps: 1) Empty server. 2) Read the heading and the button (EN, then JA).
- Expected: "Setup", "Setup token", "Create account" in English; "セットアップ", "セットアップトークン", "アカウント作成" in Japanese.
- Actual: after mount the page translation re-applied `data-i18n`, so the heading and button read "Sign in" / "Sign in" in setup mode. The labels were hard-coded English in Japanese mode.
- Evidence: probe output on the intermediate build: `heading= Sign in token_field= true button= Sign in`. After the fix: `heading= Setup token_field= true button= Create account`.
- Status: **fixed** in 6e4d418 (setup mode removes `data-i18n` from the heading and button; labels use the existing `login.setup`, `login.token`, `login.createAccount` keys).

### BH-R8-004: Medium: English table headers and buttons in Japanese mode

- Where: `web/js/views/workspace.js` (history/calculations, projects, audit, users, obligations tabs; new project and create forms).
- Steps: 1) Japanese mode. 2) Open `#/workspace/history`, `#/workspace/projects`, `#/workspace/audit`, `#/workspace/users`.
- Expected: headers and buttons in Japanese.
- Actual: headers "Job, Family, Project, Release, Gates, Created", "ID, Name, Description, Docs, Calcs", "Timestamp, User, Action, Detail", "Username, Role, Active"; buttons "New calculation", "New project", "Create", "Back", "Add user", "Resolve"; headings "Calculations", "Projects"; empty texts "No calculations yet.", "No projects.".
- Evidence: `/root/nf-logs/bughunt-r8/hunt4.jsonl` (before) and `hunt_final.jsonl` (after), lines `ja-light-1440 history`.
- Status: **fixed** in ad3a3b3 (existing keys `th.*`, `ws.*`, `common.*`, `insp.gates`; new keys `th.id`, `th.desc`, `th.calcs`, `th.active`, `ws.obl.resolve`, EN and JA) and 15104c5 (buttons). Also fixed a local variable that shadowed the translate function in the table builders (found during this fix, before commit). Tests: section 7b "BH-R8-002: the history table headers follow the language", section 7b "BH-R8-004: the projects tab buttons are in Japanese".

### BH-R8-005: Medium: tables and AMOEBA page wider than a 390px screen

- Where: history and projects tables (no scroll container), AMOEBA panels (`.row > .panel` could not shrink below its table).
- Steps: 1) 390x844 viewport. 2) Open `#/workspace/history`, `#/workspace/projects`, `#/amoeba`.
- Expected: no sideways page scroll; the table scrolls inside its own box.
- Actual: `document.documentElement.scrollWidth` 561, 500 and 777 px (EN and JA).
- Evidence: `hunt4.jsonl` `overflowX` fields (before); `hunt_final.jsonl` shows none. Probe after the fix: `#/amoeba sw= 390`, `#/amoeba/records sw= 390`, `#/amoeba/obligations sw= 390`, `#/amoeba/dependencies sw= 390`.
- Status: **fixed** in ad3a3b3 (`.tbl-scroll` wrapper for workspace tables; `.panel-b.tbl-sticky` scrolls at 820px and below; `.row > .panel` gets `min-width: 0`). CSS only; no look change above 820px. Tests: section 7b "BH-R8-003: the history table scrolls inside itself at 390px" and "the AMOEBA page has no horizontal scroll at 390px".

### BH-R8-006: Medium: three.js errors in the console when WebGL is unavailable

- Where: `web/js/lib/steel.js` `createSteelStage`.
- Steps: 1) Start Chromium with `--disable-webgl --disable-webgl2`. 2) Sign in; open `#/`, `#/job/1`, `#/amoeba`, `#/amoeba/records`.
- Expected: the fallback text stays and no console error is logged.
- Actual: `THREE.WebGLRenderer: A WebGL context could not be created ...` and `Error creating WebGL context.` on every page; the stage is still set up as null.
- Evidence: `probe_fx.js` before the fix (3 errors per load); after the fix `webgl-off errors []` on all four routes.
- Status: **fixed** in ca538a7 (WebGL is checked before the renderer is constructed; `convert.js` already handles `null`). Test: section 10, "BH-R8-006: without WebGL the home page logs no WebGL or three.js error".

### BH-R8-007: Medium: engine and server prose stays English in Japanese mode

- Where: job view (provenance and gate texts, e.g. "provenance complete", "exact signed area of the drawn outline"), admin security tab (e.g. "session cookie is Strict", "connection peer address"), admin audit detail ("conversion ... not released", "scale ratio").
- Steps: 1) Japanese mode. 2) Open `#/job/1`, `#/admin/security`, `#/admin/audit`.
- Expected: Japanese text, or at least Japanese labels around English identifiers.
- Actual: these sentences come from the engine and the server as English prose. The catalogue has translation tables for some of them; these were not covered.
- Evidence: `hunt_final.jsonl` `englishInJA` lists for `ja-light-1440 job`, `admin-security`, `admin-audit`.
- Status: **not fixed**. Reason: it needs new entries in the engine and server template tables (`web/js/i18n.js`, the SERVER and ENGINE tables), which is a translation task of its own. Not counted as a regression.

### BH-R8-008: Low: unknown conversion shows its title twice

- Where: `#/r/nope` (unknown conversion id).
- Steps: 1) Open `#/r/nope`.
- Expected: one error title ("Conversion failed") and the message.
- Actual: the page text starts "Conversion failed Conversion failed Bad Request" (seen in the probe text; no screenshot saved).
- Status: **not fixed**. Low, cosmetic; not checked further in this round.

### BH-R8-009: Medium: legacy browser suite `tests/ui_smoke.js` is stale

- Where: `tests/ui_smoke.js` (test suite, not product code).
- Steps: `NF_NOSEED=1` empty server, then `node tests/ui_smoke.js`.
- Expected: passes on the current UI.
- Actual: `UI TEST FAILED page.waitForSelector: Timeout 30000ms exceeded. waiting for locator('#f')`. The suite targets the retired `web/app.js` UI with its first-run form. That file is not served any more (`index.html` loads `js/app.js`). The current browser suite is `tests/ui_smoke_v2.js`, which passes.
- Status: **not fixed**. Rewriting 1000 lines of a retired UI test is out of scope for a bug hunt. Owner decision needed: retire the file or rewrite it against `js/`.

### BH-R8-010: Low: intermittent failures in `tests/ui_smoke_v2.js` under host load

- Where: test timeouts (15 to 20 s waits), not a confirmed product defect.
- Observed: 4 runs out of 7 on the final code showed 1 to 3 failing checks (seeded and empty server), each a different one: "login lands on convert dropzone" (timeout), "language toggle switches to Japanese" (timeout), "B job pattern tab draws the pattern" (curves=0), "K PDF result screen" (view still AMOEBA after 15 s), "J ... empty view" (job details). The host load average was 13 to 20 at the time. The same code passed on the immediate rerun in the other 3 runs (seeded 31/0 and 31/0; empty server 32/0).
- Status: **not fixed**. Not a product fix. If it recurs on a quiet host, it needs a real investigation (the login timeout would be the first suspect).

## 4. Checked, no defect

- **Export lock:** jobs 1, 2 and 3 (REVIEW REQUIRED, gate G01 INCONCLUSIVE) show "Export DXF (locked)", the button is disabled and the reason is shown. No approve or release control found (`ui_smoke_v2` safety check). `web/js/views/job.js` was not changed (deferred to A-12 per the coordinator). The APPROVED-and-all-gates-PASS path cannot be reached with the seed data and was **not tested**.
- **WebGL off:** no console errors on AMOEBA, job, home or AMOEBA records (after BH-R8-006). The AMOEBA mount falls back to the static blob (`no-gl` class set).
- **Reduced motion** (`prefers-reduced-motion: reduce`): `fx-off` class set; no console or page errors on the four routes. Visual check not done.
- **Routes with no errors:** home (convert), job, job pattern, job details, refusal job, math, history, help, manufacturing, verification, AMOEBA and its five tabs, account, admin and its tabs, project, projects, `#/r/<id>` download links (`/api/pdf-conversions/N/dxf` and `/report` return 200). No console errors, page errors or failed requests after the fixes.
- **Upload:** a real PDF (`crates/nf-pdf/tests/fixtures/reportlab_lines.pdf`) with scale 1:10 converts and shows "CONVERTED FROM PDF — NOT VERIFIED — NOT RELEASED".
- Not counted as defects: English names of seeded projects and jobs (user data); AMOEBA identifiers shown in monospace next to Japanese labels (deliberate); formula parameter names on the help page.

## 5. Not tested

- `tests/e2e_smoke.sh`, `tests/offline_check.sh`, `tests/desktop_smoke.js`, `tests/install_check.sh`, `tests/security_smoke.sh`, `tests/security_r6.sh`, `tests/release_verify.js`, `tests/bound_hp_check.py`: not run. The shell suites and the desktop suite stop their own server with `kill` and some need root or `unshare`; the process rules for this round forbid that path here.
- Desktop mode (`--desktop`).
- APPROVED job path and the export after approval.
- Visual review of every dark and light screenshot; the screenshots were produced (`shots/`), and only samples were looked at.
- Account and admin write actions (password change, add user, deactivate) beyond what `ui_smoke_v2` already does.
