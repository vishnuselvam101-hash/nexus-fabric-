# DXF metadata: what a NEXUS FABRIC DXF file states, and how

Engine 0.4.5 (22 Nov 2026; work plan task 1.3). Every DXF the engine writes is an AC1009 (R12) file. It states three kinds
of fact: the cut geometry; a statement about the part (its units, for a header-with-branch part its joint, and for every
part the plate conventions of section "Plate conventions"); and the NOT RELEASED marking. The cut geometry of every part
is byte-identical to 0.4.4 except the RB-008 closed outer profile (section "RB-008"), whose vertices are unchanged.

A file states its facts in two places, so that a reader that ignores comments still sees them:

1. the leading group-999 comment block (the first comment lines of the file, before the first section). Application
   keys are written as `KEY: value` on one line;
2. TEXT annotations on the non-cutting layer `NF_NOT_RELEASED`, below the geometry, in capitals, at the size of the
   release marking (the joint lines) or in fine print (the rest).

## The 999 comment block

| line | meaning | values | written for |
|---|---|---|---|
| `NEXUS FABRIC DXF - units: millimetres - NOT RELEASED: development build, not validated, not for manufacturing` | the human statement of the units and of the release state. Not a key; kept as it was | fixed text | every file |
| `NF-UNITS: mm` | the unit of every length in the file, as a key | `mm` (the only value) | every file |
| `NF-JOINT: <joint>` | the joint of the header-with-branch part, as the job declares it | `set-on` or `not declared` | header-with-branch parts only |
| `NF-HEADER-SURFACE: <surface>` | the wall surface the header's contact radii refer to, as the job declares it | `outside`, `inside`, `neutral` or `unknown` | header-with-branch parts only |
| `NF-BRANCH-SURFACE: <surface>` | the wall surface the branch's contact radii refer to, as the job declares it | `outside`, `inside`, `neutral` or `unknown` | header-with-branch parts only |

Rules of the keys:

- A key is the text before the first `: `; its value is the rest of the line, with control characters replaced by
  spaces and the ends trimmed. Every key line is written once. A file that states any other `NF-` key is not one of
  this build's files and fails the quality gate (rule `readable`).
- `set-on` is the set-on joint of Spec 02 section 4.4: the branch inside surface meets the header outside surface, and
  the opening is the branch bore. `not declared` means the joint type was not declared: the branch end and the header
  opening are cut on one curve of the entered radii, a zero-thickness idealisation. Nothing else is written for the
  joint, and no output calls an undeclared joint set-on.
- A part of another family (cylinder shell, cone frustum, eccentric reducer, branch stub) states no joint: it has no
  `NF-JOINT`, `NF-HEADER-SURFACE` or `NF-BRANCH-SURFACE` line, and no joint TEXT.
- Other readers may ignore group-999 comments. Nothing in the cut geometry depends on them.

## The TEXT annotations on NF_NOT_RELEASED

| text | when |
|---|---|
| `MARKING SIDE: PATTERN DRAWN AS SEEN FROM THE ... OF THE FORMED PART - PROVISIONAL CONVENTION, NOT REVIEWED` | every part (as before) |
| `NOT RELEASED - DEVELOPMENT BUILD - NOT VALIDATED - NOT FOR MANUFACTURING` | every part (as before) |
| `UNITS: MILLIMETRES` | every part (as before) |
| `JOINT: SET-ON (SPEC 02 SECTION 4.4): BRANCH INSIDE SURFACE ON HEADER OUTSIDE SURFACE, OPENING IS THE BRANCH BORE` | header-with-branch part declared `set-on` |
| `JOINT: NOT DECLARED: BRANCH END AND HEADER OPENING ARE CUT ON ONE CURVE OF THE ENTERED RADII` | header-with-branch part whose joint is not declared |
| `HEADER SURFACE: <HEADER SURFACE> - BRANCH SURFACE: <BRANCH SURFACE>` | header-with-branch parts, in capitals (`OUTSIDE`, `INSIDE`, `NEUTRAL`, `UNKNOWN`) |

Each joint line is written once. The quality gate (rule `joint_statement`) compares the 999 keys and these lines with
the job's joint: a line that is missing, repeated, altered or contradicting makes the file fail the gate, and a file
that fails the gate is withheld and gate G15 fails.


## Plate conventions (engine 0.4.5)

Every part states what its plate coordinates mean on the formed part. The job declares the statement from the pattern
and the request (`nf_core::job::conventions_for`); the file, the job result (`dxf_output.statement.plate_conventions`,
and `plate_convention_texts`) state the same facts. The interface does not display the plate conventions yet; showing them is a later task.

### The 999 keys

| key | meaning | values |
|---|---|---|
| `NF-VIEW-SIDE` | the face of the formed part the pattern is drawn from (the marking side) | `outside` (the only value this build writes) |
| `NF-PLATE-AXES` | what the plate X and Y axes are, with the developed sizes in mm | sentence |
| `NF-ROLLING-AXIS` | the axis the plate is rolled about | `parallel to X` / `parallel to Y` / the cone axis through the origin |
| `NF-SEAM-EDGES` | which edges are the seam (the longitudinal joint) and where they lie | sentence with the curve names `seam_a`, `seam_b` |
| `NF-END-EDGES` | which edges are the ends (the open edges of the formed part) | sentence |
| `NF-ANGLE-DATUM` | where the angle is zero, and the direction in which it increases | sentence |
| `NF-ORIGIN` | the plate origin (0, 0) and where it lies | sentence (not always on the plate) |
| `NF-KERF` | the cut lines are the theoretical line; kerf is not compensated | fixed sentence: `cut lines are the theoretical line on the development surface: kerf is NOT compensated; no bevel, root gap or weld allowance is included` |
| `NF-STATUS` | the convention is provisional and not reviewed | fixed sentence: `provisional convention of this build, declared by the software, not reviewed by a fabrication engineer` |
| `NF-OFFSET-DELTA` | the offset of the part (`delta` for a branch, `e` for an eccentric reducer), in mm | e.g. `delta = 12.5 mm: ...`; only for a part with an offset |
| `NF-OFFSET-SIGN` | the sign of the offset and the direction in which it is positive | only for a part with an offset |
| `NF-BRANCH-ANGLE` | the lean `beta` of the branch, in degrees, measured from the direction of increasing station | only for a branch (the branch stub and the header plate) |

The 999 line is `KEY: value`, one line, at most 240 characters (`MAX_COMMENT_CHARS`). Each key is written once.

### Which keys each family states

| family (engine name) | part name in the file | keys |
|---|---|---|
| `cylinder_shell` | `shell` | the nine base keys |
| `cone_frustum` | `reducer` | the nine base keys |
| `eccentric_reducer` | `reducer` | the nine base keys, `NF-OFFSET-DELTA`, `NF-OFFSET-SIGN` |
| `branch_stub` | `branch` | the nine base keys, `NF-OFFSET-DELTA`, `NF-OFFSET-SIGN`, `NF-BRANCH-ANGLE` |
| `header_shell_with_opening` | `header` | the nine base keys, `NF-OFFSET-DELTA`, `NF-OFFSET-SIGN`, `NF-BRANCH-ANGLE` |

The base keys are `NF-VIEW-SIDE`, `NF-PLATE-AXES`, `NF-ROLLING-AXIS`, `NF-SEAM-EDGES`, `NF-END-EDGES`, `NF-ANGLE-DATUM`,
`NF-ORIGIN`, `NF-KERF`, `NF-STATUS`. A cone and a cylinder state no offset; a part that states an offset it does not have
fails the gate. The verifier's list is `uem::verify::required_convention_keys`.

### The TEXT annotations

Each statement is also drawn as one annotation line on `NF_NOT_RELEASED`, `LABEL: VALUE` in capitals, where the label is the key
without `NF-` and with its dashes as spaces (`NF-PLATE-AXES` → `PLATE AXES: ...`). A line longer than 250 characters is split at
a space into several annotation lines; the quality gate builds the same split. The annotation is English, as the rest of the
drawing (see "Language" below).

### Japanese labels (for the interface; not written into the DXF)

| key | label (EN) | label (JA) |
|---|---|---|
| `NF-VIEW-SIDE` | viewing side | 表示面 |
| `NF-PLATE-AXES` | plate axes | 板の座標軸 |
| `NF-ROLLING-AXIS` | rolling axis | 巻き軸 |
| `NF-SEAM-EDGES` | seam edges | 合わせ目の縁 |
| `NF-END-EDGES` | end edges | 端の縁 |
| `NF-ANGLE-DATUM` | angular datum | 角度の基準 |
| `NF-ORIGIN` | plate origin | 板の原点 |
| `NF-KERF` | kerf | カーフ（切りしろ） |
| `NF-STATUS` | status | 状態 |
| `NF-OFFSET-DELTA` | offset | オフセット |
| `NF-OFFSET-SIGN` | offset sign | オフセットの符号 |
| `NF-BRANCH-ANGLE` | branch angle | 枝の傾き |

Fixed statements, EN and JA: `NF-KERF` — EN: the cut lines are the theoretical line on the development surface; kerf is NOT
compensated; no bevel, root gap or weld allowance is included. JA: 切断線は展開面上の理論線であり、カーフ補正は行わない。
ベベル、ルート間隔、溶接代は含まない。 `NF-STATUS` — EN: provisional convention of this build, declared by the software, not reviewed
by a fabrication engineer. JA: 本ビルドの暫定規約であり、ソフトウェアが宣言したもの。製造技術者の確認は受けていない。
`NF-OFFSET-SIGN` (branch) — EN: POSITIVE = the branch axis is displaced to the left of the header axis, for an observer at the
station-0 end looking along the header toward increasing station with the branch pointing up. JA: 正 = 枝軸は幹軸の左側へ変位する
（幹の起点側から、枝を上に、増加ステーションの向きに見て）。

The family-specific sentences (`NF-PLATE-AXES`, `NF-ROLLING-AXIS`, `NF-SEAM-EDGES`, `NF-END-EDGES`, `NF-ANGLE-DATUM`,
`NF-ORIGIN`, the offset values) are written in `crates/nf-core/src/job.rs` (`conventions_for`). Their Japanese is not yet
provided: see the i18n keys listed in the engine report of task 1.3.

### Language

The DXF is an R12 drawing read by CAD programs. Its annotations stay English capitals, as the marking has always been; no
Japanese text is written into the file. The Japanese is carried by the interface and by the labels above.

## The quality gate: rule `convention_statement`

`nf_dxf::quality::quality_gate_full` compares the file with the job's statement. A file fails the rule when a declared key
is missing, stated more than once, or stated with a value other than the declared one; when a convention key the part does
not declare is present; when an annotation on `NF_NOT_RELEASED` is not one of the declared lines (after the fixed marking,
units, marking-side and joint lines); or when a declared line is missing or repeated. The job also fails the rule when its
own statement is not the complete set of its family (`uem::verify::convention_keys_complete`) or holds a value that is not
a number. A file that fails any rule is withheld and gate G15 fails.

## Units: why `$INSUNITS` is not written

The file is AC1009 (R12). The header variables that R12 defines for units are `$LUNITS` (linear format, 2 = decimal),
`$AUNITS` (angular format, 0 = decimal degrees), `$ANGBASE` and `$ANGDIR` (zero angle along +x, counter-clockwise).
R12 (AC1009) defines no unit-of-measure variable, so this build does not write `$INSUNITS` into an R12 file: it would
name a variable the format does not define. So the unit is stated in the three places above (the human 999
line, `NF-UNITS: mm`, and the TEXT `UNITS: MILLIMETRES`), not in the header. A CAD program that reads only the header
does not know the scale of a file produced by this build. The re-import check and the quality gate require the three
statements; they do not read `$INSUNITS`.

## What the independent reader checks

`nf_dxf::reimport_check_stated` reads the file back and compares it with the entities the job holds (the cut geometry,
the marking, and the extents) and with the statement: the units line, the joint keys and the joint TEXT lines. The
quality gate (`nf_dxf::quality::quality_gate_stated`) reads the same statement and reports each difference under the
rules `units_declared`, `joint_statement` and `readable`. The job's records carry the result as `reimport_report` and
`dxf_quality_report`, and the joint of the job is the `interface` record: its `declared` value is the `NF-JOINT` value.

## Changes from 0.4.0

0.4.0 files carry no `NF-UNITS` line and no joint statement. Results of 0.4.0 are REJECTED with gate G21
("recompute"), so no 0.4.0 file is offered as current; a recomputed result of 0.4.1 is.

## Changes from 0.4.1

Engine 0.4.2 (B2-005, B2-006) changes only which inputs are refused and two reported quantities (eccentric `area`,
branch `s_min`/`s_max`). The cut geometry of every previously accepted case is byte-identical. The ORIGIN note of a
branch stub (a TEXT line after the first TEXT) quotes the exact s_min. Results of 0.4.1 are REJECTED with gate G21
("recompute").

## RB-008: the outer profile is one closed polyline (engine 0.4.5)

The outer profile of a part made only of lines and polylines is written as ONE closed polyline on `CUT_OUTER` (DXF
polyline flag 70 bit 1). Its vertices are the vertices of its curves in the same order; each shared end point is written
once; the closing point is not repeated. Measured on the corpus: the eccentric reducer, the branch stub and the header plate
(the header and the branch of each header with branch) and the cylinder shell are grouped; the cone frustum is not, because its
outer profile has arcs. An arc would need a vertex bulge, an encoding this build does not write, and chords would change the
cut; so the cone profile stays one entity per curve, byte for byte. The openings (`CUT_INNER`) were already closed polylines and
are unchanged. The JSON curves of the pattern are unchanged. The test `rb008_*` in `crates/nf-core/tests/plate_conventions.rs`
checks the vertices against the engine 0.4.4 curves.

## Changes from 0.4.4

Engine 0.4.5 adds the plate conventions and the rule `convention_statement` (13 → 14 rules), and groups the outer profile
of a part with no arc (RB-008). The cut geometry of every other entity is byte-identical. The `$EXTMIN`/`$EXTMAX` of
a file change where the fine-print annotation lines extend the extents below the geometry. Results of 0.4.4 are REJECTED
with gate G21 ("recompute").
