# NEXUS FABRIC flagship film 2 — treatment

50.0 s · 1920×1080 · 30 fps · English and Japanese with identical picture timing.

## The idea, in two sentences

Where a branch meets a header there is one curve, and the film never leaves it: one unbroken move follows that curve from the numbers typed into the real application, through the equation that defines it, to the check that both plates agree on it, the record, the gates and a DXF that still says *not released*.
Everything on a product screen is the application running, photographed frame by frame; every number is the product's own output; and the narration claims only what the software does today.

## Narration (recorded first; the picture is cut to it)

| # | English | Japanese |
|---|---|---|
| 1 | Where a branch meets a header, there is one curve. Get it wrong, and nothing fits. | 枝管と母管が交わる、一本の曲線。誤れば、合いません。 |
| 2 | NEXUS FABRIC takes the dimensions of the joint, and calculates both flat patterns. | NEXUS FABRIC。寸法から、二枚の板展開を計算します。 |
| 3 | One equation gives every point on the cut. | 切断線は、ひとつの式で決まります。 |
| 4 | Then it checks its own work. Folded back, the two edges must be one curve. | そして、自ら検算。立体に戻し、取合いの一致を確かめます。 |
| 5 | Every step is recorded, and can be replayed. | すべて記録し、再実行できます。 |
| 6 | What it cannot confirm, it shows. The result stays: review required. | 確認できないことは隠しません。結果は「要レビュー」のまま。 |
| 7 | The DXF is written, read back, compared — and marked: not released. | DXFは再読込して照合。「未リリース」と明記します。 |
| 8 | Reading drawings is planned. The calculation, the checks, the record: running today. | 図面の読み取りは計画中。計算・検証・記録は、いま動いています。 |
| 9 | NEXUS FABRIC. Engineering, reimagined. | NEXUS FABRIC。設計を、再定義。 |

What the voice never says: automated intelligence doing anything, drawings being read, 3D reconstruction, manufacturing constraints, nesting, CNC, approval, thickness, "zero errors", customers. The one planned item (reading drawings) is said once, as planned, in line 8, and is on screen as "Automated drawing reading: planned" for exactly as long as the owner's positioning line is.

## Shot list

Times are those of the locked voice slots (`audio/vo-timing.json`); the film page reads them from `audio/timing.js`.
"Live" means frames photographed from the running application by `capture/capture.js` (take name in brackets).

| # | Time, s | Voice | Picture | Real footage or data used | Status shown |
|---|---|---|---|---|---|
| 1 | 0.0–6.3 | 1 | Darkness; a line of light becomes two thin steel shells, a branch standing on a header. The line where they meet ignites. On "get it wrong" a second, wrong cut drifts off the surface and the gap shows. | Geometry from the same governing equation, with the values the form starts from (R 150, r 100, 90°). No product screen. | Visualisation made for this film · not a product view |
| 2 | 6.3–7.4 | — | The view turns to a pure side elevation and its lines land exactly on the application's own to-scale parameter schematic: match cut into the product, which pulls back into a lit, tilted window. | Schematic geometry read from the application's SVG at capture time [form]. | Actual software · development build |
| 3 | 7.4–10.9 | 2 | Close on the form. "600" is typed into the station field; the angle is stepped from 90 to 60 with the arrow key and the schematic's branch tilts with every degree. | Live [form]: pointer, focus ring, typed digits, 30 arrow-key steps, the schematic redrawn by the application each time. | Implemented: entering dimensions |
| 4 | 10.9–12.4 | 2 | The pointer presses "Develop, verify and generate DXF". The job page arrives: header plate with its opening, the application's own REVIEW REQUIRED banner and "NOT RELEASED" watermark, its notification "computed and verified". | Live [form → result]. The request is real (33 ms on this machine); the busy state is shown for two frames. | Implemented: pattern mathematics |
| 5 | 12.4–14.3 | — (no voice) | Up close on the canvas: the pointer runs along the opening and the read-out counts real vertices; wheel zoom; back to fit; the branch part is selected and its saddle cut appears. | Live [canvas, branch]: hover read-out, zoom, fit, part switch. | Actual product output |
| 6 | 14.3–18.4 | 3 | The camera goes through the canvas onto the saddle cut. The application's curve becomes the film's curve, plotted from the product's vertex list at the same place, while the pointer is still tracing it. The governing equation sets itself in large type; φ and s(φ) run live. | Product vertices (721 of the saddle cut) from the job's API result; the equation as specified; the live values are the vertices under the pointer. | Implemented: pattern mathematics · the 3D inset is a film visualisation |
| 7 | 18.4–24.3 | 4 | The header plate arrives beside the branch plate (both from product vertices). Both roll back into shells; the two cut edges close on one another and become one line. The distance read-out is the product's. Then the same numbers in the real Checks tab, filtered to the interface checks. | Product vertex lists; the product's check `branch_cut_lies_on_header_opening` (difference, tolerance, verdict). Live [checks]. | Implemented: verification, interface check · the fold is a film visualisation |
| 8 | 24.3–27.8 | 5 | The pointer presses "Run replay audit". The Records tab answers REPLAY MATCH; the list of records; one record opened to its stored payload. | Live [records]; record count and closure hash from the job. | Implemented: record of every step, replay |
| 9 | 27.8–32.8 | 6 | Gates tab. The summary as the application shows it (5 pass · 7 inconclusive · 1 not implemented), down the list to "human approval — NOT IMPLEMENTED", up to the job's state tag: REVIEW REQUIRED. | Live [gates], captured after the replay so the summary is the complete one. | Implemented: gates · engineer approval is not implemented and is shown as such |
| 10 | 32.8–38.2 | 7 | The pointer presses "DXF · not released". The real file streams past as text; its own coordinates are read back and drawn over the pattern; the file's NOT RELEASED annotation and its name stay. | Live [dxf]; the DXF downloaded in that capture, parsed in the page. | Implemented: DXF written, re-read, compared, marked not released |
| 11 | 38.2–44.4 | 8 | The drawn curve tightens into the product's mark; the wordmark sets beside it. The owner's positioning line appears once, with "Automated drawing reading: planned" at the same size beside it for the same time. Three words — calculation, checks, record — are ticked as running. | The application's own mark glyph. | Planned: automated drawing reading (once, on screen and in the voice) |
| 12 | 44.4–50.0 | 9 | End card: NEXUS FABRIC — Engineering, Reimagined. · Request a demonstration · the disclaimer, legible from the first frame of the end card (38.2 s) to the last. | — | Product concept. Capabilities in development. Not validated for manufacturing. |

## Continuity (how each shot hands over)

1→2 match cut on the drawing · 2→3→4→5 one camera inside one window, following the pointer · 5→6 through the canvas, curve to curve · 6→7 the same curve stays on screen and gains its partner · 7 the number in the illustration lands on the same number in the application · 7→8→9 pointer-led inside the window · 9→10 the DXF text comes out of the pressed button · 10→11 the read-back curve becomes the mark. No fades to black, no title cards between scenes.

## Sound

Voice on top (at least 12 LU over music and effects together, master −14 LUFS, true peak under −1 dBTP). Original score synthesised in code: one slow pulse in D that the picture's moves land on, opening up at the equation and resolving at the mark. A designed sound on every visible move: key clicks for the typed digits, a detent tick for each degree of the angle, the press, the result arriving, vertex ticks under the pointer, the roll of steel sheets, the two edges locking, the replay stamp, the DXF text and the pen redrawing it, the mark.

## Honesty rules applied

- Product screens are never redrawn or retouched; they are cropped and moved only. The pointer is drawn by the film at the recorded positions of the real pointer events (headless browsers do not paint one).
- The application's development-build notice, REVIEW REQUIRED banner, NOT RELEASED watermark and status bar are in frame in shots 3, 4, 5, 8, 9 and 10.
- The gate summary is shown whole, including "7 inconclusive" and "1 not implemented".
- The reference job is an idealised zero-thickness case; the shells are drawn as thin surfaces with no wall.
