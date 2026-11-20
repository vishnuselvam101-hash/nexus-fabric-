# Interface changes needed after the joint and quality-gate pass (20 Nov 2026)

web/ was not edited in this pass. The browser suite (tests/ui_smoke.js, 151 checks) passes against this build; the changes below are what the interface should eventually show. Nothing here is a blocker.

| # | Where | Old | New | Why |
|---|---|---|---|---|
| 1 | Request, family `header_with_branch` (new calculation form) | `surface_reference` only | optional `header_surface_reference`, `branch_surface_reference` (`outside`, `inside`, `neutral`, `unknown`; absent = `surface_reference`) and optional `joint` (`"set_on"`, or absent) | RB-001: each part declares its own contact surface; the set-on joint needs the branch inside surface on the header outside surface. The form does not send the new fields, so it behaves as before. |
| 2 | Refusals, field names | `surface_reference`, `header_radius`, … | may also name `joint`, `header_surface_reference`, `branch_surface_reference`, `branch_dev_radius` | a set-on declared with a wrong pairing is refused with the field named. The field label needs a translation. |
| 3 | Result, record `interface`, payload `joint` (shown in records and the assembly view) | `"set-on (as entered)"` | `"single contact curve (joint type not declared): …"` or `"set-on (Spec 02 section 4.4: …)"` | An undeclared joint is no longer called set-on. |
| 4 | Result, record `interface`, new keys `declared`, `header_surface`, `branch_surface` | absent | present | shows which surfaces each part was declared against. |
| 5 | web/i18n.js, the static assumption text "Set-on joint assumed: opening defined by the branch contact surface meeting the header contact surface." (around line 933) and "Contact radii (which wall surfaces define the fit: set-on / set-in) are entered directly; no joint rule applied." (around line 928) | states a set-on joint for every assembly | say that the joint type is declared or not declared, and name the declared surfaces | the statement is wrong for an undeclared joint. |
| 6 | Result, `interface` checks: 5 checks instead of 3 (new names `opening_on_intersection_of_branch_geometry`, `saddle_on_intersection_of_header_geometry`) | 3 | 5 | independent 3D point checks of the joint. Labels for the two names are needed in the checks list. tests/ui_smoke.js and tests/e2e_smoke.sh now expect 5. |
| 7 | Result, `patterns[].quantities`: `polyline_segments` and `chord_error_limit` removed | listed (with no unit in the catalogue, so shown as "unit not stated") | not listed; both stated in `assumptions` ("Polyline density: N segments per curve…", "Chord error limit: 0.01 mm, an implementation limit…") | the catalogue has no unit for them. Showing them with units would need a label entry. |
| 8 | Result, `assumptions` of each part | 2 entries in addition to the pattern's own | up to 4 entries (the two density entries above) | as in 7. |
| 9 | Result, new record kind `dxf_quality_report` (assessment, one per part) | absent | payload `pass`, `rules_failed`, `findings`, `entities`, `outer_contours`, `inner_contours`, `segments_tested`, `rules_checked` | DXF quality gate. A failing file is withheld and gate G15 fails (state REJECTED). The records view shows it generically. |
| 10 | Gate G15 (output integrity): detail text names the quality gate; result can be FAIL when a file fails the quality gate | re-import only | re-import and quality gate | no layout change; the detail text is longer. |
| 11 | Gate G02 detail: adds a sentence when the joint type is not declared | | | as 3. |
| 12 | Convention record `surface_reference`: `value` is now `{"header": …, "branch": …}` and it carries `joint` | a string | an object | a records-view change only. |
| 13 | Engine identifier, `nf-engine 0.4.0+src.…` (was 0.3.0) | 0.3.0 | 0.4.0 | results of 0.3.0 are REJECTED with G21 "recompute"; the interface already shows that text. |

Not changed by this pass (so no UI work): DXF text, the marking side note, the request fields the form sends, the DXF download names and states.
