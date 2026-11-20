# DXF metadata: what a NEXUS FABRIC DXF file states, and how

Engine 0.4.1 (20 Nov 2026). Every DXF the engine writes is an AC1009 (R12) file. It states two kinds of fact: the
cut geometry, and a statement about the part (its units, and for a header-with-branch part its joint). The cut
geometry is unchanged by the statement: the entities on the cutting layers are the same bytes as in 0.4.0.

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
