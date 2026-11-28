# Order review catalogue (PZ-1, 28 November 2026)

This document describes the review path built on 28 November 2026. It is a development-build record, not an engineering approval. A hold never releases a cut file, and no route in this path approves a job.

## Catalogue

`GET /api/review/catalogue` returns `{ "catalogue": [...], "never_wave": [...] }` to an authenticated user. `GET /api/review/shapes` returns `{ "shapes": [...] }`. The catalogue is generated from `nf_core::order_compute::HOLD_TEXT` plus the review-only and alias codes, so every hold code has exactly one row. The build has 77 hold codes, 81 catalogue rows and 18 never-wave codes. Alias and case variants (`flat stop`, `plate-c-big`, `FLAT_STOP`) are folded by `nf_core::review::canonical()` before lookup, refusal and decision hashing; the same function maps ZK's `seam_from` and `unsupported` to the emitted `seam_from_unknown` and `shape_unsupported`.

Each row carries:

- `code`, `level`, `scope`, `actions`, `may_accept`, `never_wave`
- `title_en`, `title_ja`, `answers_en`, `answers_ja`

The answers say what a person may do: enter a corrected value with a reason, confirm a read value, choose a split or seam side, undo a decision, skip a page with a reason, or take the hand-drawn exit. `may_accept` is false for a never-wave kind and for `page_empty`/`empty`, `unreadable`, `row_unmatched` and `mass_total`. A row is not an approval. A page-level `need` is carried by the first part in the compute result, becomes a hold on that part and enters `dxf_blocked_by`; an `ok` mass row is informational only.

The never-wave list is `FLAT_STOP`, `split_ring`, `seam_from`/`seam_from_unknown`, `missing`, `unsupported`/`shape_unsupported`, `empty`, `split_cone`, `tee_bad`, `tee_branches`, `plate_bad`, `plate_off`, `plate_C_big`, `rib_count`, `rib_n1`, `bend_bad` and `wye_bad`. `accept` names `code`; `accept_flag` names `flag`; `claim` and `hand_drawn` also name `code` when they answer one. Every decision that names a catalogued never-wave code (or an alias or case variant of one) is refused with HTTP 422 and code `review_never_wave`, for an engineer/admin and for the AI operator alike. The refusal is audited as `order_refused`, and its `fields` array carries the canonical code so the web's `{fields}` message is not blank. No decision row is written. Every named catalogue code must also be open on the named part in the current compute; `accept_flag` flags that are not catalogue rows stay as the older free-text flags.

## Page inputs

Two order inputs were added for the review; both are project-guarded and write-restricted to `admin`/`engineer`. Every PUT, by a person or by an AI operator, must give a `reason` (the earlier `note` field is still read as a reason):

- `GET|PUT /api/orders/{oid}/materials`: `rows` of `{ page, no, name, size, length, qty, mass }`. A row is owned by the first part whose `table_rows` names it. An ownerless row is still stored and is reported as `row_unmatched`.
- `GET|PUT /api/orders/{oid}/notes`: `notes` of `{ page, kind, text }`, where `kind` is `note`, `seam_from` or `unit_mass_kg`. `unit_mass_kg` is the page's total unit mass; `unreadable` in a note text is reported as an `unreadable` review issue.

The order GET response exposes both as `material` and `notes`. Compute responses expose page-level issues as `review`; a page `need` also appears as a hold on the first part and withholds its DXF. Material rows and page notes join the compute input fingerprint, so changing either makes the next compute a new result row instead of reporting `unchanged`.

A PUT is not an unchecked clearing of a hold. It writes a `review_context` decision with the reason and a before/after snapshot of both inputs; the audit log names both values. Undoing that decision restores the earlier inputs only while they are still the ones the decision was made against, so a later edit is never overwritten by undoing an older one. Page-input decisions must be undone in reverse order (the latest still-active one first); an out-of-order undo is refused with `context_undo_out_of_order`. A successful restore writes its own audited `review_context` decision ending in `_restore` and records the restored old→new values in the undo audit, so a cleared hold is never left with no effective, reasoned decision behind it. A restore row is the audit record of that undo and cannot itself be undone; replaying it would re-clear the hold while the original PUT stayed undone. Undoing a page input needs a non-empty note, which is carried into the restore decision's reason. Page skips are separate audit events and do not change the material/note binding used by that undo; if those inputs did change, the undo is refused with `context_undo_stale` and the decision stays active.

## Review kinds and tolerances

The compute raises the listed ZK kinds on the same part as ZK. The golden fixture `tests/golden/zumen/review_kinds.json` has 63 listed rows over 22 files, replayed against 23 reading files; the test checks both that each listed row is raised and that no listed kind is raised on a ZK-clean reading. New codes include `mass_total`, `row_unmatched`/`row`, `page_empty`/`empty`, `shape_unsupported`/`unsupported`, `seam_from_unknown`/`seam_from`, `tee_angle_obtuse`, `tee_flange_face`, `tee_mount_note`, `pad_qty`, `tee_qty`, `split_cone`, `split_ring`, `pad_split`, `size_vs_table`, `cyl_len`, `bend_len`, `tee_owner`, `stock_rule` and `unreadable`.

Two catalogue rows are review vocabulary rather than new compute emissions. `shape_unsupported` is the canonical API form of ZK's `unsupported`; the compute blocks an unrecognised shape as `shape_not_computed` (and a purchased/other shape as `purchased_not_computed`), and a note or decision using the alias is stored and refused under `shape_unsupported`. `tee_owner` is the catalogue row for a shared tee value without an identified owner; the compute emits `tee_group_no_main` (and `cone_owner` for the cone-branch case). Both rows carry EN/JA titles and answers; neither is a release path.

Tolerances are deliberately separate from the calculation tolerances: `mass_total` is the larger of 1 kg and 0.2 %; `size`, `cyl_len` and `bend_len` are 1 mm; `mass` is 0.5 %. Table comparisons are skipped for a part that could not be unfolded, matching ZK. Bend comparison uses the internal sum of bend-piece centre lengths and is not a stored result field.

## Decisions that are not approvals

`POST /api/orders/{oid}/decisions` accepts `hand_drawn` and `claim` in addition to `confirm`, `accept_flag`, `accept` and `mark_checked`. Both new kinds require a part and a reason; `claim` also requires a catalogue `code` and non-empty `claim` text. Any decision that names a catalogue code needs a current compute with that hold open on the named part, and no decision may name a never-wave code. Both new kinds are tied to `order_fingerprint()`, so an edit to the order or the page inputs makes them stale and ineffective. They are undoable through the existing decision undo route, remain in the decision history and never change a value or release a DXF. An AI operator cannot confirm or approve by waving a hold.

An `accept` or `accept_flag` is bound to the read value, the canonical review code and the page inputs (`review_context_json`) it answered. Both accept kinds join `order_fingerprint()`, so a new accept, its undo or a material/note edit makes an earlier "checked" mark, claim or hand-drawn exit stale. The decision view recomputes staleness from the same three inputs; undoing an accept does not revive an older answer.

The job ZIP gained `designer.txt` and a `designer` object in `inputs.json`. Both say `NOT RELEASED` / `未リリース`; a designer claim is a record, not a release control. The ZIP writer lives in `nf-jobout`; PZ-1 adds no server ZIP route, so a server caller does not populate `JobInput.designer` in this build. The server exposes the decisions and their codes, and any caller that builds the ZIP supplies those notes to `JobInput` without gaining an approval control.

## Storage

`SCHEMA_VERSION` is 8. Schema 8 is additive over 7: `order_material_rows` and `order_page_notes` tables, `order_decisions.review_code` and `order_decisions.claim_text` columns. Existing databases migrate without rewriting earlier results.
