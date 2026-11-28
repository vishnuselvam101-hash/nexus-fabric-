# Order review catalogue (PZ-1, 28 November 2026)

This document describes the review path built on 28 November 2026. It is a development-build record, not an engineering approval. A hold never releases a cut file, and no route in this path approves a job.

## Catalogue

`GET /api/review/catalogue` returns `{ "catalogue": [...], "never_wave": [...] }` to an authenticated user. `GET /api/review/shapes` returns `{ "shapes": [...] }`. The catalogue is generated from `nf_core::order_compute::HOLD_TEXT` plus the review-only and alias codes, so every hold code has exactly one row. The build has 77 hold codes, 81 catalogue rows and 16 never-wave codes.

Each row carries:

- `code`, `level`, `scope`, `actions`, `may_accept`, `never_wave`
- `title_en`, `title_ja`, `answers_en`, `answers_ja`

The answers say what a person may do: enter a corrected value with a reason, confirm a read value, choose a split or seam side, undo a decision, skip a page with a reason, or take the hand-drawn exit. `may_accept` is false for a never-wave kind and for `page_empty`/`empty`, `unreadable`, `row_unmatched` and `mass_total`. A row is not an approval.

The never-wave list is `FLAT_STOP`, `split_ring`, `seam_from`, `missing`, `unsupported`, `empty`, `split_cone`, `tee_bad`, `tee_branches`, `plate_bad`, `plate_off`, `plate_C_big`, `rib_count`, `rib_n1`, `bend_bad` and `wye_bad`. `accept` names `code`; `accept_flag` names `flag`. If either names a catalogued never-wave code (or `FLAT_STOP`), the API refuses with HTTP 422 and code `review_never_wave`, for an engineer/admin and for the AI operator alike. The refusal is audited as `order_refused`; no decision row is written.

## Page inputs

Two order inputs were added for the review; both are project-guarded and write-restricted to `admin`/`engineer`, and an AI operator must supply a reason:

- `GET|PUT /api/orders/{oid}/materials`: `rows` of `{ page, no, name, size, length, qty, mass }`. A row is owned by the first part whose `table_rows` names it. An ownerless row is still stored and is reported as `row_unmatched`.
- `GET|PUT /api/orders/{oid}/notes`: `notes` of `{ page, kind, text }`, where `kind` is `note`, `seam_from` or `unit_mass_kg`. `unit_mass_kg` is the page's total unit mass; `unreadable` in a note text is reported as an `unreadable` review issue.

The order GET response exposes both as `material` and `notes`. Compute responses expose page-level issues as `review`. Material rows and page notes join the compute input fingerprint, so changing either makes the next compute a new result row instead of reporting `unchanged`.

## Review kinds and tolerances

The compute raises the listed ZK kinds on the same part as ZK. The golden fixture `tests/golden/zumen/review_kinds.json` has 63 listed rows over 22 files, replayed against 23 reading files; the test checks both that each listed row is raised and that no listed kind is raised on a ZK-clean reading. New codes include `mass_total`, `row_unmatched`/`row`, `page_empty`/`empty`, `shape_unsupported`/`unsupported`, `seam_from_unknown`/`seam_from`, `tee_angle_obtuse`, `tee_flange_face`, `tee_mount_note`, `pad_qty`, `tee_qty`, `split_cone`, `split_ring`, `pad_split`, `size_vs_table`, `cyl_len`, `bend_len`, `tee_owner`, `stock_rule` and `unreadable`.

Tolerances are deliberately separate from the calculation tolerances: `mass_total` is the larger of 1 kg and 0.2 %; `size`, `cyl_len` and `bend_len` are 1 mm; `mass` is 0.5 %. Table comparisons are skipped for a part that could not be unfolded, matching ZK. Bend comparison uses the internal sum of bend-piece centre lengths and is not a stored result field.

## Decisions that are not approvals

`POST /api/orders/{oid}/decisions` accepts `hand_drawn` and `claim` in addition to `confirm`, `accept_flag`, `accept` and `mark_checked`. Both new kinds require a part and a reason; `claim` also requires a catalogue `code` and non-empty `claim` text. Both are tied to `order_fingerprint()`, so an edit to the order makes them stale and ineffective. They are undoable through the existing decision undo route, remain in the decision history and never change a value or release a DXF. An AI operator cannot confirm or approve by waving a hold.

The job ZIP gained `designer.txt` and a `designer` object in `inputs.json`. Both say `NOT RELEASED` / `未リリース`; a designer claim is a record, not a release control.

## Storage

`SCHEMA_VERSION` is 8. Schema 8 is additive over 7: `order_material_rows` and `order_page_notes` tables, `order_decisions.review_code` and `order_decisions.claim_text` columns. Existing databases migrate without rewriting earlier results.
