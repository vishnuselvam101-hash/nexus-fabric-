# NEXUS FABRIC bug hunt, round 7 (BH-001 to BH-018)

Scope: brief items A to I (AMOEBA Control Centre, corrections and obligations, joint fields, language and theme, canvas and motion, DXF, robustness, desktop mode, console). Tester only: no product file was changed. This report is the only file written in the repository (this folder was created for it).

## 1. Setup

- Binary: a copy of `target/release/nf-server` (sha256 4d6be834ccac664fef22e69016aa3614b059352d8a71f04b2793075f369cecbc), engine `nf-engine 0.4.0+src.be53b63158593bf7`, banner "Development build. Not validated."
- Main test server: 127.0.0.1:8881, fresh data folder, seeded with `docs/design/tools/seed_demo.js`, then about 190 jobs added by the tests (projects 1 to 11). Empty server: 127.0.0.1:8882 (zero-job checks). Desktop mode: 127.0.0.1:8883, stopped by its 60 s timeout. Both 8881 and 8882 were stopped with SIGTERM at the end.
- Browser: Playwright Chromium with no options; viewport 1440x900 unless stated; locales en-US and ja-JP where stated.
- Accounts: admin / correct-horse-9, engineer1 / engineer-pass-1, viewer1 / read-only-pass.
- Scratch folder (scripts, shots, dxf, run logs): `/tmp/claude-0/-home-user-nexus-fabric-/7d5fb6f5-1c9e-5151-ae27-9132ba7d2536/scratchpad/bughunt/`.

## 2. Summary

Eighteen defects: **Critical 1, High 0, Medium 4, Low 13.** One tester incident is not counted as a defect and needs the owner's action (section 4).

Five most serious:

1. **BH-001 (Critical):** typing `1,5` into a dimension field is computed as 15 mm, and `0x10` as 10 mm; the job is stored with the altered value and nothing warns.
2. **BH-004 (Medium):** a declared set-on joint and an undeclared joint with the same surfaces produce byte-identical DXF files, and the DXF says nothing about the joint.
3. **BH-005 (Medium):** no DXF declares its units machine-readably (`$INSUNITS` is missing in all 8 files tested).
4. **BH-002 (Medium):** obligations of superseded jobs stay open; 9 of the 17 open obligations count against superseded results.
5. **BH-003 (Medium):** switching language clears the field validation error and the "Not sent" note, so an invalid form looks valid.

## 3. Defects

Each entry gives: severity, where, steps, expected, actual, evidence, and how often it reproduced.

### BH-001: Critical: number fields silently change what was typed

- Where: New calculation, `#/math/1`, family `cone_frustum`, field "Axial height" (`h`). Account: admin.
- Steps: 1) Open `#/math/1`. 2) Family: cone_frustum. 3) Click the Axial height field, select all, press Backspace. 4) Type `1,5` (second case: `0x10`). 5) Click "Develop, verify and generate DXF".
- Expected: 1.5 is computed as 1.5, or the field refuses the text with a message. A computed value must equal what was typed.
- Actual: the field shows `15` immediately after typing (`0x10` shows `010`). The job is computed and stored with `h = 15`: `GET /api/calculations/198` returns `"request":{"family":"cone_frustum","h":15,"r1":200,"r2":100}`. No warning is given: the plausibility note only covers values below 1 or above 20000. `0x10` gives job 202 with `h = 10`.
- Reproduces: `1,5` 4 of 4 (en-US twice, ja-JP twice; jobs 198, 199, 200, 201). `0x10` 3 of 3 (jobs 197, 202, 203).
- Cause (observed): Chromium's number input drops the comma and the letter, so the form's validation and the request only ever see `15` and `010`.
- Mitigation and rating: the altered value is visible in the field, and every such job is REVIEW REQUIRED and NOT RELEASED. Rated Critical because a dimension ten times too large is computed and written to the record without any warning. The owner may lower it to High if the visible field value is judged enough.
- Evidence: API responses above; scripts `comma_repro.js`, `second_repro.js`. No screenshot taken.
**Status:** already fixed before this pass (the dimension fields are text fields read by `parseDim`). Kept by section 14c of `tests/ui_smoke.js`.

### BH-002: Medium: open obligations of superseded jobs stay open and inflate the count

- Where: Verification & Quality, the AMOEBA obligation ledger, the status bar. Account: admin.
- Steps: 1) `POST /api/calculations/188/corrections` with `{"parameter":"header_radius","new_value":166,"reason":"stale repro rep 1"}` gives job 189; job 188 is superseded. 2) `GET /api/obligations`: obligations 54 and 55 (target 188) are still `open`. 3) Repeat on job 189 (`branch_radius` 102): job 190 is created and obligations 64, 65 and 66 (target 189) stay `open`.
- Expected: obligations raised for a superseded result are closed, or marked as superseded, and do not count as open work.
- Actual: they stay open. `GET /api/amoeba/summary` gives `obligations.open = 17`; 9 of those 17 target superseded jobs (ids 20, 22, 31, 33, 54, 55, 64, 65, 66). The status bar ("17 open scrutiny obligations"), the Verification panel (17 rows, badge 17) and the summary agree with each other, and all three count these.
- Reproduces: 2 of 2 (188 to 189, 189 to 190). Also seen earlier on 59 to 60 and 60 to 61.
- Evidence: API JSON above; `stale_repro.js`, `counts.js`. No screenshot taken.
**Status:** fixed. Open obligations on current results are the open count everywhere (AMOEBA summary `obligations.open`, dashboard `open_obligations`, status bar, Verification badge). Obligations on superseded results stay open (no record was changed) and are counted apart (`obligations.open_on_superseded`, dashboard `open_obligations_on_superseded`), listed with the label "on a superseded result". Tests: Rust `bh002_obligations_on_a_superseded_result_are_counted_apart_and_stay_open` (`crates/nf-server/src/tests.rs`); `tests/ui_smoke.js` section 14d, "BH-002: open obligations are split..." and "BH-002: the obligations badge counts current work only".

### BH-003: Medium: language switch removes field validation feedback

- Where: New calculation, `#/math/1`. Account: admin.
- Steps: 1) Family header_with_branch (header radius 150 by default). 2) Branch contact radius: 200. 3) Click "Develop, verify and generate DXF". 4) Click 日本語.
- Expected: after the switch the error still marks the field, in Japanese.
- Actual: before the switch there is one `.field-err` and the note "Not sent. 1 value cannot be computed; see the marked field." After the switch there are zero field errors, the note is empty, and `aria-invalid` is removed. The values (200 and 150) are kept, so the form looks valid while it is not.
- Reproduces: 2 of 2 (en-US, switch to 日本語 each time).
- Evidence: DOM readings from `lang_err.js`. No screenshot taken.
**Status:** fixed. After a language switch the field error and the note are drawn again in the new language, from the values on the screen; the checks run again and nothing is sent. Test: `tests/ui_smoke.js` section 14d, "BH-003: after the language switch...".

### BH-004: Medium: a declared joint changes no output and does not appear in the DXF

- Where: New calculation, family header_with_branch. Account: admin.
- Steps: 1) Project 1. Values: header_radius 161.95, header_dev_radius 156.795, header_length 1200, branch_station 600, branch_radius 77.095, branch_dev_radius 80.6, beta 60, delta 0, s_end 400, surface_reference unknown, header_surface_reference outside, branch_surface_reference inside. 2) Submit once with joint `set_on` (job 62) and once with the joint empty (job 63). 3) Download the DXF of both parts for both jobs.
- Expected: either the outputs differ, or the DXF states the joint type and the wall surfaces the opening was cut for.
- Actual: the DXF files of jobs 62 and 63 are byte-identical (header sha256 starts 0d9d0a80b901, branch d5b847fbd2f2). The pattern quantities are identical (header opening area 21523.033 mm2, perimeter 523.002 mm; branch square end 506.425 mm). Only the stored text differs: records say "set-on (Spec 02 section 4.4 ...)" for 62 and "single contact curve (joint type not declared)" for 63. The DXF TEXT entities mention neither the joint nor the surfaces.
- Same at radii 150/100: jobs 9 (undeclared) and 16 (set-on) give identical DXF (header 52cabe8d..., branch 3cd46836...).
- Reproduces: 2 of 2 radius sets.
- Note for the owner: whether identical geometry is right for these inputs is an engineering question, because the zero-thickness model uses the same contact radii in both cases. The defect as reported: the joint declaration and the surfaces that define the opening are not visible in the file a shop receives, and the stored sentence and the file are not linked.
- Evidence: `seton.js` output; `dxf/job9-header.dxf`, `dxf/job16-header.dxf`.
**Status:** fixed in engine 0.4.1 (the statement only). Both files now state their joint: `NF-JOINT: set-on` or `NF-JOINT: not declared`, the two surfaces, and two TEXT lines on NF_NOT_RELEASED (docs/backend/DXF_METADATA.md). The cut geometry is unchanged, so the two files still differ only in the statement. Whether identical geometry is right for these inputs remains an engineering decision for the owner. Tests: `crates/nf-core/tests/dxf_statement.rs` (every family and part, both joint states; altered statements fail the gate and the reader), `crates/nf-core/tests/regressions/005-joint-statement-and-units-in-the-dxf.json`.

### BH-005: Medium: DXF units are not declared machine-readably

- Where: Manufacturing, every DXF download. Account: admin.
- Steps: `GET /api/calculations/4/dxf/shell` (file `NOT-RELEASED_REVIEW_REQUIRED_calc4_shell_cfe162cc.dxf`). Repeat for jobs 1, 2, 3, 9 and 16.
- Expected: the HEADER section sets `$INSUNITS` to 4 (millimetres), or otherwise declares units in a form CAD software reads.
- Actual: none of the 8 files contains `$INSUNITS`. Units appear only in a group-999 comment ("units: millimetres") and a TEXT entity ("UNITS: MILLIMETRES"). The HEADER has `$LUNITS` (format only). A CAD program that reads `$INSUNITS` does not know the scale.
- Reproduces: 8 of 8 files (jobs 1, 2, 3, 4, 9 header and branch, 16 header and branch).
- Evidence: `dxf/job4-shell.dxf` (opened, full text read); grep of all 8 files for `INSUNITS` gives 0.
**Status:** fixed in engine 0.4.1 without `$INSUNITS`: R12 (AC1009) defines no unit-of-measure variable, so the file does not declare one. Every file states its unit in three places: the group-999 line `NF-UNITS: mm`, the existing human 999 line, and the TEXT `UNITS: MILLIMETRES`. Documented in docs/backend/DXF_METADATA.md. Tests: `crates/nf-core/tests/dxf_statement.rs` (`every_family_and_part_states_its_units_and_its_joint_exactly_as_declared`).

### BH-006: Low: status card text collision

- Where: pattern view title block (Part / Job / Engine / Marking side / Status).
- Steps: open `#/calc/9` (English, dark); also `#/calc/9` in light theme; `#/calc/60` in 日本語.
- Expected: no overlap between the engine string and the Marking side value.
- Actual: `0.4.0+src.be53b63158593bf7` wraps and its last characters run into the Marking side value (`outside`, or `外面` in Japanese); in light theme "outside" sits on the hash line.
- Reproduces: 3 of 3 (English dark, English light, Japanese).
- Evidence: `shots/motion-fit-job9.png` and `shots/corr1-job.png` (opened, overlap visible); `shots/card-light-job9.png` and `shots/card-ja-job60.png` (opened).
**Status:** fixed. A long value in the title block (the engine id) wraps inside its cell (`overflow-wrap: anywhere` on `.tblock .v`, `web/styles.css`). Test: `tests/ui_smoke.js` section 14d, "BH-006: the status card keeps each value inside its cell" (English dark, English light, Japanese).

### BH-007: Low: AMOEBA "Activities" badge counts activity types, not activities

- Where: `#/amoeba`, tab badge. Account: admin.
- Expected: the badge shows the number of activities, or its label says what it counts.
- Actual: the badge shows 8. The table below lists 8 activity types whose distinct-activity counts add up to 67 (`GET /api/amoeba/summary`). The Records badge (207) counts rows, which is consistent with its own table.
- Reproduces: 2 of 2 (the badge reads "Activities 8" in `shots/amoeba-activities-en-dark.png` and `shots/graph-job8-en-dark.png`, both opened).
**Status:** fixed. The Activities badge counts activities: the sum of the table's distinct-activity column, the same rule the Records badge uses. Test: `tests/ui_smoke.js` section 14d, "BH-007: the Activities badge equals the number of activities in its table".

### BH-008: Low: console error on every sign-in screen load

- Steps: open `http://127.0.0.1:8881/` while signed out.
- Actual: console "Failed to load resource: the server responded with a status of 401 (Unauthorized)" for `GET /api/me`, the session probe. Under brief item I this is a console error on the sign-in screen.
- Expected: no console error on any screen.
- Reproduces every time (every page load in every run, more than 20 times).
**Status:** fixed. The start-up probe is now `GET /api/session`, which answers 200 with `signed_in: false` for a visitor who is not signed in; the sign-in screen makes no failed request. `/api/me` keeps its 401, which the security checks rely on. Tests: Rust `bh008_the_start_up_probe_answers_200_for_a_visitor_who_is_not_signed_in`; `tests/ui_smoke.js` section 14d, "BH-008: the sign-in screen loads with no failed request and no console error". Not covered: a request that gets a 401 because a session ended while the user was working is still logged by the browser (the suite allows that case).

### BH-009: Low: misleading API message for an over-long project name

- Steps: `POST /api/projects` with a 10,000-character name and a valid description, as admin.
- Actual: `400 {"error":"project name is required (max 200 characters)"}`. The name is present, so "required" is wrong. The UI is not affected: the name input has `maxlength="200"`, so the browser caps the text and the project is created.
- Reproduces: 2 of 2 (name of `A` x 10000 and of `B` x 10000 / `C` x 10000).
**Status:** fixed. An empty name is "project name is required"; a name longer than 200 characters is "project name is too long (max 200 characters)". The limit is now counted in characters, as the field's maxlength is. Before, the server counted bytes, so a name of more than 66 Japanese characters was refused although the field allowed 200: that is a behaviour change beyond the message. Tests: Rust `bh009_project_name_refusals_say_what_is_wrong_and_count_characters`; the Japanese texts are in the `SERVER` table of `web/i18n.js`.

### BH-010: Low: unsent draft lost when the session expires

- Steps: admin; `#/math/1`; family cone_frustum; Axial height 333; clear the session cookies; click Run; sign in again with the same account.
- Expected: the draft comes back, or at least the chosen family and values remain.
- Actual: the banner "Your session has ended. Sign in again." is correct, but after sign-in the form is back at the default family (cylinder_shell) and the typed values are gone.
- Reproduces: 3 of 3.
**Status:** fixed. When the session ends while a form is open, the visible form's values are kept in this tab's `sessionStorage` (inside try/catch; never a password or a token) and restored after the sign-in. Test: `tests/ui_smoke.js` section 14d, "BH-010: after a forced sign-in the form comes back...".

### BH-011: Low: raw HTTP status shown for a malformed job id in AMOEBA

- Steps: `#/amoeba/dependencies/abc`.
- Actual: "Could not load the graph" followed by "Request failed (400)". The unknown id `#/amoeba/dependencies/999` shows the clearer "not found".
- Expected: a readable message, as for the unknown id.
- Reproduces: 3 of 3.
**Status:** fixed. A job id that is not a whole number gets the same "not found" message as an unknown id, and no request is sent for it. Test: `tests/ui_smoke.js` section 14d, "BH-011: a malformed job id shows the not-found message..." The request is no longer sent, so the suite's console check sees no 400 response.

### BH-012: Low: browser Back does not step through AMOEBA tabs

- Steps: `#/verification` → `#/amoeba` → click Lessons → click Mechanisms → browser Back.
- Expected: Back returns to Lessons.
- Actual: Back returns to `#/verification`. The tab code calls `history.replaceState` (web/app.js, wireTabs and the AMOEBA wire function), so tab changes never enter the history.
- Reproduces: 2 of 2 in the final run (and once earlier).
**Status:** fixed. A tab change in AMOEBA and in administration adds a history entry, so Back and Forward step through the tabs. Test: `tests/ui_smoke.js` section 14d, "BH-012: browser Back returns to the previous AMOEBA tab and Forward returns to the next".

### BH-013: Low: 40 px horizontal page scroll at 360 px width

- Steps: viewport 360x780, admin; open `#/calc/9`, `#/amoeba`, `#/math/1`, `#/verification`, `#/manufacturing`.
- Expected: no horizontal page scroll. The brief's requested widths (768 to 1920) are fine; 360 is outside them, so this is Low.
- Actual: `document.documentElement.scrollWidth` is 40 px wider than the viewport on all five screens. No overflow at 768, 1024, 1280 or 1920 px.
- Reproduces: 2 of 2 runs, 5 of 5 screens.
**Status:** fixed. Below 600 px the top bar wraps onto a second line (`web/styles.css`), so nothing runs past the right edge and the build notice stays whole. The rule is recorded in the breakpoint table of `docs/design/DESIGN_SYSTEM.md`. Test: `tests/ui_smoke.js` section 14d, "BH-013: no horizontal page scroll at 360 px wide on the five main screens".

### BH-014: Low: the form accepts values the engine then refuses

- Steps: the form's own validator (`validateRequest` from web/app.js, copied unchanged into a scratch module) against the engine, same values.
- Actual: the form accepts these, and the engine refuses them with a REFUSED job that stays in the list:
  - `s_end` 0.5: form gives only a warning; engine: "square end must lie beyond the saddle: s_end > 230.94".
  - `beta_deg` 10 or 170 with `s_end` 400: engine: "s_end > 1430.94".
  - `branch_station` 1199.9 with `header_length` 1200: engine: opening spans x = 1171.032 to 1401.973, outside the shell.
- Expected: the field stops these, as it does for the tangent limit, so no REFUSED job is created.
- Reproduces: the engine refusals for beta 10 and 170 and for station 1199.9 were seen in two separate runs. The form-side result came from the extracted validator in one comparison run covering all four surface values.
**Status:** partly fixed. Engine 0.4.1 gives every refusal a stable code, the fields it concerns and, where the engine computes them, the limit and the measured value (docs/backend/REFUSAL_CODES.md). A refused job shows its refusal on the named field, and "New from these values" marks the same fields, in English and Japanese. The client still has no new rule: the engine's square-end limit and its opening-inside-shell limit come from sampled geometry, so they cannot be stated exactly in the client (the brief allows no invented rule). The engine stays the judge. Its refusal names the field (`s_end`, `branch_station`) on the refused job, and the form still accepts such values, so a REFUSED job is still recorded (records are append-only). Test: `tests/ui_smoke.js` section 14d, "BH-014: the engine refusal of a square end names the s_end field...".
**Work plan 3.2 (22 Nov 2026, cloud session):** "New from these values" now marks every field of every stored refusal code (20 requests, all 19 codes a stored job can carry with a field) with the message of its code in English and Japanese, links the message to the field (`aria-describedby`) and says so in the form's message area (a live region), naming any field the form has no control for (the surface reference of a cylindrical shell). A refusal answered before a job exists (400: `field_missing`, `field_wrong_type`, `field_unknown`, `family_unknown`, `surface_reference_required`) and a coded refusal of a correction (`reason_required`, `correction_unknown_parameter` ...) mark their fields the same way. Two messages that described limits the engine no longer measures (`reducer_near_cylindrical`, `developed_angle_overlaps`, changed in engine 0.4.2) were corrected. Tests: `tests/ui_smoke.js` section 14g. The gap above (the form cannot state the sampled limits) is unchanged.

### BH-015: Low: API requires `surface_reference` even when both part references are given

- Steps: `POST /api/projects/3/calculations` with header_radius 150, header_dev_radius 150, header_length 1200, branch_station 600, branch_radius 100, branch_dev_radius 100, beta 60, delta 0, s_end 400, `header_surface_reference: "outside"`, `branch_surface_reference: "inside"`, `joint: "set_on"`, and no `surface_reference`.
- Actual: `400 {"error":"invalid request: missing field `surface_reference`"}`. With `surface_reference: "unknown"` added the same request gives 200 REVIEW_REQUIRED.
- Expected: per docs/backend/UI_CHANGES_NEEDED.md item 1, per-part references are optional and "absent = surface_reference". Per-part references should be enough.
- Reproduces: 2 of 2 (all 50 combinations without the shared field in the matrix, and the repeat).
**Status:** fixed. `surface_reference` may be left out when both `header_surface_reference` and `branch_surface_reference` are given; when a part surface is missing the request is still refused with the same message. Requests that carried the shared field are unchanged (it is written back as before). Tests: Rust `bh015_part_surfaces_are_enough_and_the_shared_one_is_needed_only_when_a_part_is_missing` (`crates/nf-core/src/job.rs`) and `bh015_header_with_branch_accepts_the_part_surfaces_without_the_shared_one` (`crates/nf-server/src/tests.rs`).

### BH-016: Low: joint text prints the parameter key inside a sentence

- Steps: English job page for job 63 (joint not declared) and job 62 (set-on), Gates / Records tabs, and the properties.
- Actual: "Joint type joint not declared" and "Joint type joint Set-on". Japanese shows "取合いの形式 joint 未宣言" with the Latin key.
- Expected: a sentence without the repeated word.
- Reproduces: 2 of 2 jobs.
**Status:** fixed. The joint row and the two surface rows show their label as a phrase; the parameter key moved into the tooltip (`title`) and is no longer printed in the sentence. Test: `tests/ui_smoke.js` section 14d, "BH-016: the joint row shows its label without the key...".

### BH-017: Low: desktop mode keeps the storage key next to the encrypted data

- Steps: start `nf-server --desktop --no-browser --port 8883` with a scratch HOME; list `~/.local/share/nexus-fabric`.
- Actual: the folder (mode 700) holds `nexus-fabric.sqlite`, `-wal`, `-shm`, `blobs/` and `storage.key` (32 bytes, mode 600). The server guide says `NF_KEY_FILE` should be outside the data folder; desktop mode gives no option to do that.
- Expected: the key is kept apart from the data, or the guide and the desktop behaviour say so.
- Reproduces: the same listing at first start and after setup (2 of 2). Rated Low because the key is readable only by the same local user.
**Status:** not changed. Where the storage key sits in desktop mode, and what the guide says about it, is the owner's decision.

### BH-018: Low: keyboard focus on the workspace splitters has no visible indicator

- Steps: `#/math/1`, click the family select, press Tab repeatedly.
- Actual: after the Run button, two `div.split[tabindex=0]` elements (workspace resize handles) receive focus with no outline or box-shadow.
- Expected: a visible focus indicator on every focusable element.
- Reproduces: 2 of 2 runs.
**Status:** fixed. The workspace splitters take the global 2 px focus outline (the rule that switched the outline off was removed). Test: `tests/ui_smoke.js` section 14d, "BH-018: a focused workspace splitter shows an outline or a box-shadow".


### NEW (found in the tester's incident): `nf-server --help` and unknown arguments start a server

- Where: `crates/nf-server/src/main.rs` (`main`, `invocation`, `help_text`, `version_text`).
- Steps: run `nf-server --help` (or any unknown argument).
- Expected: the usage prints (the environment variables and the `--desktop` options) and the program exits 0; no data folder, key, setup token or port is created.
- Actual (before this pass): a server started with the default data folder `./data` and listened on 127.0.0.1:8787.
- **Status:** fixed. `--help` and `-h` print the usage and exit 0; `--version` prints the version and exit 0; an unknown argument prints an error and exits 2. Nothing is created and no port is opened for these. Tests: `crates/nf-server/tests/cli.rs` (8 integration tests that run the binary) and `invocation_tests` in `crates/nf-server/src/main.rs` (4 unit tests).

## 4. Environment incident (not a product defect; needs the owner)

- Early in the run I started the copied binary with `--help`. The binary does not print help: it started a server in the foreground. That process is still running: **PID 5857**, working directory `/home/user/nexus-fabric-`, listening on **127.0.0.1:8787** (the default port).
- Its data folder is the default `./data`, which resolves to **`/home/user/nexus-fabric-/data`**, inside the repository tree and not my scratch folder. That folder already held `storage.key` and `nexus-fabric.sqlite` (timestamp 02:35, before my process started; I did not create them). The files `setup-token` (33 bytes, mode 600), `nexus-fabric.sqlite-wal` and `nexus-fabric.sqlite-shm` carry the timestamp 02:39, when my process started. I did not modify or delete them afterwards.
- The existence of `setup-token` means that database has no administrator yet. While PID 5857 runs, anyone who can reach 127.0.0.1:8787 and read that file can create the first administrator there. It also occupies the default port 8787, so a default-port start by anyone else will fail.
- I tried once to stop PID 5857 with `kill -TERM`. The command was denied by the permission classifier, and I did not try any other way to stop it.
- Action needed from the owner: stop PID 5857 (for example `kill 5857`), then decide what to do with `/home/user/nexus-fabric-/data` (in particular the one-time `setup-token`). I changed nothing else in the repository.
- My own servers on 8881 (PID 8374) and 8882 (PID 32602) were stopped with SIGTERM and confirmed gone. The desktop copy on 8883 stopped at its timeout.
- **Status (fix pass, not a product defect):** on checking, PID 5857 was no longer running and nothing listened on 127.0.0.1:8787. This pass did not stop or signal it, and the cause of its exit is not established here. `/home/user/nexus-fabric-/data/setup-token` (33 bytes, mode 600) is still in that folder; what happens to that folder is the owner's decision.

## 5. Tested and worked

- AMOEBA tabs: all six render; badges Records 207 (sum of rows), Activities 8 (see BH-007), Obligations 2, Lessons 1 agree with the API.
- AMOEBA obligations table: 11 rows, "2 open of 11" agrees with the summary at that time.
- Dependency graph, jobs 6, 7, 8 (18 nodes, 29 edges) and 1, 2 (3 nodes, 2 edges): node labels and edges match the edge table; no node outside the frame; no label overflow (`graph-job8-en-dark.png`, opened).
- AMOEBA deep links: a tab in the URL survives reload; the job selector changes the URL and the graph; unknown job id gives "not found" in English and 日本語; a bogus tab name falls back to Records.
- Correction 59 to 60 (header_radius 150 to 160): new job "corrects #59", 11 obligations (3 open for an engineer, 8 discharged by the recomputation); the dialog toast reads correctly.
- Second correction 60 to 61 by engineer1: allowed. Correcting a superseded job: 409 "already superseded; correct the current one".
- Refused corrections: 0, -5, 99 (tangent), 1e9 give 422 "nothing was changed", and job 60 stays current; 'abc', null, an unknown parameter, the same value and an empty reason give 400.
- Viewer: correction API 403; the UI hides "Record correction", replay and "New from these values" for viewer1; the math page says the role cannot run calculations.
- Obligation resolution: empty or 2-character reason 400; decision other than confirmed or waived 400; discharged obligation 409; unknown id 404; UI resolve with a valid reason works ("Obligation #53 confirmed"); a stale obligation can be waived.
- Replay audit: runs in a separate process (the response says so) for job 9 and for the superseded job 61.
- Joint matrix on the engine (5 header refs x 5 branch refs, absent included, x joint on/off x shared surface absent or outside = 100 requests): every set-on with a header not outside or a branch not inside is refused with field `joint`; header outside with branch inside is accepted; the undeclared joint is never described as set-on in English or Japanese records, gates, assumptions or the job header (checked on jobs 62 and 63 in both languages).
- The form's joint rule (web/app.js `validateRequest`: set_on needs header outside and branch inside) is the same rule the engine applies. It was read from the code and not run against every combination.
- Edge values for radii, beta, delta, station, s_end, lengths: no case where the form blocks a value the engine accepts (see BH-014 for the reverse gap).
- DXF names: all 8 files start with `NOT-RELEASED_REVIEW_REQUIRED`; the not-released text line is present; layer `NF_NOT_RELEASED` exists; units note present (but see BH-005).
- DXF geometry against the curve table: shell lines exact; branch-opening polyline (720 vertices) exact; cone reducer arcs within 0.0006 mm of the table points, inside the table's declared 0.034 mm.
- Cone reducer lengths: large end 1256.637 mm = 2 pi x 200; small end 628.319 mm = 2 pi x 100; seams 316.228 mm = slant length (sqrt(100^2 + 300^2)).
- Refused job 5: no DXF control in the UI; API 404 "nothing was computed". Superseded job 6: no DXF control; API 409 "files are withheld". Manufacturing shows text, not download links, for superseded and rejected rows.
- Duplicate submission: double-click on the new-calculation Run creates one job; Enter twice creates one job; double-click on the correction confirm creates one job; two tabs correcting the same job: the second gets 409 with a readable message.
- Session expiry: the sign-in screen appears with "Your session has ended. Sign in again."; browser Back after sign-out shows the sign-in screen, no job data.
- Hostile names: project names containing HTML/script, emoji, Arabic RTL text and Japanese are stored and shown as text; no injected element or script ran; no horizontal overflow on the projects page.
- 404 route: "Page not found" in English and 日本語.
- Language: all 14 routes in English and 日本語, dark and light: no page error, no failed request, no CSP message, no raw translation keys (only identifiers and the engine hash). Half-filled form keeps its values across the switch (170, 500 and set-on survive). Layout: no horizontal overflow at 1440 px in any language.
- `?lang=` junk values (`xx`, a script tag, `ja%00`, `JA`, empty) fall back to English; no script runs.
- Theme: toggling light and dark redraws the pattern fill (dark rgb(28,36,48) to light rgb(241,244,248)) and the status card keeps its contrast by eye.
- Pattern viewer: zoom buttons (100% to 195% in four steps, 195% to 21% in twelve), wheel zoom, drag pan; Fit gives 100% on job 9; read immediately after the click it shows 17%, and 100% after 300 ms.
- Hover read-out on the header top edge: "seam_b, length 1,200.000 mm, vertex 2/2 (0.000, 985.172)", matching the table; X and Y match the scale.
- Tab switching: 30 rapid clicks across six tabs leave exactly one panel visible, the selected one.
- Widths 1920, 1280, 1024 and 768: no horizontal overflow on the five main screens.
- Memory: performance.memory 10 MB before and after 60 job-view openings (no growth seen).
- Keyboard: tab order through the new-calculation form is sensible; correction dialog moves focus to the first field, Escape closes it and returns focus to the opener; AMOEBA arrow keys and End move focus and selection.
- Zero jobs (server 8882): every AMOEBA tab and dashboard, projects, workspace, manufacturing, verification and admin screens show proper empty states with no error.
- Desktop mode: data folder `~/.local/share/nexus-fabric` mode 700; database and key files mode 600; setup link carries a 64-character token in the fragment; wrong token 403; short user name 400; token reused after setup 409; a second copy on the same folder refused with "already running on the data folder".
- Setup: a second setup call on a configured server answers 409.

## 6. Not tested, and why

- Server restart in the middle of a session: not run. The only server processes I could stop are my own, and I did not restart one to test this.
- Desktop port fallback when the preferred port is busy: not run. The code falls back to any free port (desktop.rs, bind_candidates), which would take this run outside 8881 to 8889.
- Desktop clean stop on Ctrl+C: left to tests/desktop_smoke.js (it covers SIGINT). I stopped my desktop copy with its 60 s timeout only.
- Curve selection across a language switch: inconclusive; the scripted click did not select a curve.
- A job rejected by the DXF quality gate (not the engine's REFUSED): none found; only REFUSED (no file) and superseded (409) were checked.
- 200 job-view openings: 60 done. Memory from performance.memory, not process RSS.
- Dialog and reason field in the obligation UI with a short reason: the field-error text was not captured (selector); the API refusal was verified instead.
- Reduced motion: confirmed only in CSS (web/styles.css line 675 removes transitions and animations under prefers-reduced-motion: reduce); not run in a browser.
- Contrast: judged by eye on three screens; not measured. Dialogs in light theme not captured.
- Hover read-out: one edge checked, not every curve.
- Japanese layout by eye: the status card only; other screens by text sweep and zero overflow.
- Mid-task language switch with the correction dialog open: the dialog is modal, so the header language button is blocked by design; the keyboard path only typed the letter into the reason field.

## 7. Evidence index

- Scratch root: `/tmp/claude-0/-home-user-nexus-fabric-/7d5fb6f5-1c9e-5151-ae27-9132ba7d2536/scratchpad/bughunt/`
- Screenshots (opened and looked at): `shots/graph-job8-en-dark.png`, `shots/corr1-job.png`, `shots/motion-fit-job9.png`, `shots/card-light-job9.png`, `shots/card-ja-job60.png`, `shots/amoeba-activities-en-dark.png`, `shots/dxfui-job5-light.png`, `shots/dxfui-job6-light.png`, `shots/theme-toggle-job62.png`, `shots/robust-projects-long.png`.
- DXF files: `dxf/job1-reducer.dxf`, `dxf/job2-reducer.dxf`, `dxf/job3-branch.dxf`, `dxf/job4-shell.dxf`, `dxf/job9-header.dxf`, `dxf/job9-branch.dxf`, `dxf/job16-header.dxf`, `dxf/job16-branch.dxf`.
- Scripts: `scripts/` (for example `joint_matrix.js`, `seton.js`, `edge_cmp2.js`, `validator_client.js`, `comma_repro.js`, `second_repro.js`, `stale_repro.js`, `counts.js`, `dxfcheck.js`, `lang_err.js`).
- Server logs: `run1/server.log` and `run1/server2.log` (port 8881); `desk/run1.log` (desktop run).

## 8. Fix pass: status and checks

Each defect has a status line in its section (section 3). Fixed: BH-002, BH-003, BH-006, BH-007, BH-008, BH-009, BH-010, BH-011, BH-012, BH-013, BH-015, BH-016, BH-018, the new command-line defect, and BH-001 (fixed before the pass). Partly: BH-014. Fixed in engine 0.4.1: BH-004 and BH-005. Not changed: BH-017, and the environment incident (section 4) for the owner.

Checks at the end of the pass, on the release build: `cargo test --workspace` 157 passed, 0 failed (140 before the pass); `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all -- --check` clean; `bash tests/e2e_smoke.sh` 89 passed, 0 failed; `bash tests/security_smoke.sh` 66 passed, 0 failed; `tests/ui_smoke.js` on a fresh server 183 passed, 0 failed (169 before the pass, plus the section 14d checks); `node tests/desktop_smoke.js target/release/nf-server 8868` 18 passed, 0 failed; `bash tests/offline_check.sh` passed (its API, browser and desktop suites passed inside a network namespace with no route out); `node docs/design/tools/i18n_check.mjs` no problem (815 keys in each language).
