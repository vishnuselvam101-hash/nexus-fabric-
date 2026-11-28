# PZ-5a — independent marking expected values

`verify_marking.py` produces expected marking lines from the public formulas and public example
tables in `/root/zumen-kirizu/reader/research/spec_marking.md` (plus the tee hole dimensions printed
in `spec_branches.md`). It is intentionally separate from the NEXUS FABRIC Rust port: the Rust lane
PZ-5 must read `marking_golden.json`, not call back into this file or into the ZK JavaScript.

Provenance / limits:

- The partner's `verify_marking.py` was removed from the handover copy. It is not present, was not
  recovered, and none of its values are copied from any source other than the public spec tables.
- Examples 7-1..7-5 (partner parts) are absent from the handover spec. The script derives their
  families from the section 3 formulas and records the public qualitative checks (cylinder W/8,
  bend C/8, cone side-centre and four-seam skip, ecc generatrix lengths, ring seam skip).
- Public table values, not recomputed values: tee-main hole cuts (7-6), tee-branch line lengths
  and W (7-7), tee-pad centres and around/axial extents (7-8). Straight-line positions come from
  the section 3 formulas. The triangle lines in 7-9 are now derived from the 200x150 triangle and
  phi18 hole and compared to the printed table; the other 7-9 rectangle spans are derived by
  clipping.
- The script uses only the Python standard library, reads no network, no product source files and
  no private/partner data. It exits non-zero on any mismatch.

Run:

```sh
python3 validation/verify_marking.py                    # PASS n | NG 0
python3 validation/verify_marking.py --write-json validation/marking_golden.json
```

Current run: `PASS 217 | NG 0`. Tolerance 0.01 mm (the spec allows 0.1 mm for the line checks).
`marking_golden.json` is the input for the PZ-5 Rust golden test; it carries the real computed and
public values, not a hand-written "all pass" flag. Its `_meta` block records schema 1, millimetres,
the tolerance and the provenance boundary. Tuple layouts are per-shape; see the key map below and
the EN/JA table for layer/rule names.

| Key | Stored tuple / layout |
|---|---|
| `cyl.marks` | `(name, plate_index, u_mm)` |
| `bend.marks` | `(name, x_mm, y0_mm, y1_mm, length_mm)` |
| `cone.n2` / `cone.n4` | `(name, plate_index, xs, ys, xl, yl, slant_mm)` |
| `ring.n2` / `ring.n4` | `(name, plate_index, xs, ys, xl, yl)` |
| `tee_main` | `case -> plate_index -> {v: [(name, u, x_spans)], h: [(x, u_spans)]}` |
| `tee_branch` | `W`, quarter `u`, and public per-branch lengths |
| `tee_pad` | `C`, axial spans, around spans per pad case |
| `plate` | rectangle, holes `(cx, cy, diameter)`, derived line spans |
| `plate_triangle` | derived line spans `(coord, a, b)` |

## Named conventions (all UNVERIFIED)

Each below is a factory convention taken from the spec, recorded by name so PZ-5 can emit it in
`rules_out` with `source` and `status: UNVERIFIED`. None is confirmed by NF.

| Name | Definition (spec section) | Status |
|---|---|---|
| `MARK_SEAM_AZIMUTH` | Longitudinal seam azimuth `alpha_s = 180 + a` from top (default `a = 45`, i.e. 225 deg); n seams at `alpha_s + 360k/n` (2). | UNVERIFIED |
| `MARK_THICKNESS_BASIS` | Circumference uses the mid-thickness `Rm = (D - t)/2`; bend/cone/ecc use the selected basis (default centre). Marks share the developed outline basis (2). | UNVERIFIED |
| `MARK_OUTSIDE_FACE` | Layout is viewed from outside; marks go on the face that becomes outside when rolled. G02 marking-side stays INCONCLUSIVE (2). | UNVERIFIED |
| `MARK_SEAM_SKIP` | Do not draw a mark within 0.5 mm of a seam or split line; drop stubs shorter than 0.5 mm (1). | UNVERIFIED |
| `MARK_COINCIDENT_MERGE` | Branch-centre line within 0.5 mm of a quarter line is merged and gets both names (1). | UNVERIFIED |
| `MARK_LAYERS` | DXF layers are CUT (outline + holes, unchanged bytes), MARK (scribe lines), TEXT (part no, thickness, material, qty). Marks are never on CUT and never alter outline points, mass or piece count (1). | UNVERIFIED |
| `MARK_TEE_BRANCH_DIVISIONS` | Tee branch quarter lines at k = 4, 8, 12 of 16; k = 0 and 16 are the longitudinal seam (1, 7-7). | UNVERIFIED |
| `MARK_CONE_SEAM_START` | Cone seams start at top (azimuth 0) and are evenly spaced; n=2 gives top/bottom. No `coneSeam` value is encoded in NF (8). | UNVERIFIED |
| `MARK_TEE_BRANCH_POSITION_LINE` | Branch position line uses the branch centre for a right branch and the intersection `branch_x` for an oblique branch, not `xs` or hole mid-point (8). | UNVERIFIED |
| `MARK_TEE_E_SIDE` | Branch centre azimuth is `dir + asin(e/Ri)`; a positive offset `e` is treated as the `+` side because the handover reader has no `e_side` field (8). | UNVERIFIED |
| `MARK_CONE_QUARTER_ORIENTATION` | Cone quarter lines use azimuths 0/90/180/270 in the cone's own seam frame; no factory top/left convention is confirmed (8). | UNVERIFIED |
| `MARK_RING_QUARTER_ORIENTATION` | Ring radial lines use angles 0/90/180/270 from +x; bolt-hole phase and ring top are unknown (8). | UNVERIFIED |
| `MARK_ECC_FLAT_SIDE_ORIENTATION` | Eccentric cone is labelled by flat side / opposite side; no top/bottom value is present in the reader (8). | UNVERIFIED |
| `MARK_PLATE_HOLE_LINE_EXTENT` | Plate hole-centre lines run from outer boundary to outer boundary, with only the hole diameter cut out (8). | UNVERIFIED |

## EN/JA terms used by the golden fixture

| Key | EN | JA |
|---|---|---|
| `quarter` | quarter line | 四つ割の線 |
| `back` / `belly` | back / belly line | 背・腹の線 |
| `side` | side line | わきの線 |
| `branch_center` | branch centre line | 枝の中心の線 |
| `branch_position` | branch position line | 枝の位置の線 |
| `hole_center` | hole centre line | 穴の中心線 |
| `CUT` / `MARK` / `TEXT` | cut / mark / text layer | 切断・けがき・文字の層 |
| `UNVERIFIED` | unverified convention | 未確認の決まり |

The fixture file only stores numeric line coordinates plus short English names; the table above is
the EN/JA mapping PZ-5 must use for layer labels and rule text. No partner file was consulted.
