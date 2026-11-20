# flagship3: progress

## Revision 5 (round-4 critics, the lead's four points, owner's contacts and 4K60 masters)
Picture only; audio, subtitles, narration and the slot grid unchanged. Finals and masters are in `final/` (see README.md for the list).

### The 12 MUST FIX items
1. Hook: done. Pipe slides in 0.0-0.5 s, branch lowers 0.5-1.0 s, 14 px gap 1.8-2.2 s with one flash on the seam (2.0-2.15 s), closed by 3.6 s.
2. Transitions: done. The outgoing scene fades and rises 12 px over four frames and is gone at bar - 0.1 s; the incoming element is on the bar at 60 % and full four frames later; one text zone per frame. The three paper-only frames before each bar follow from the rule (strips: 3.90-3.97, 7.90-7.97, 11.90-11.97, 19.90-19.97, 25.90-25.97, 29.90-29.97, 41.90-41.97, 45.90-45.97, 49.90-49.97 s).
3. Static holds: partly. One beat-locked action per hold where there is one: checks card green marks 23.5-26.0 s (half-second beats), DXF stamp 29.0 s, honesty 44.0 and 45.0 s, end card 52.0 and 53.0 s. The part-name holds (12-20 s), the question and the product statement have no beat action.
4. DXF scene: done. Real header flat pattern in both panels (outlines fitted to 560x380 px each, 1,120 px across); rows scroll at 6 lines/s 26.0-26.6 s (corrected in this pass from 240 px to 144 px, which was 10 rows/s); blue read-back 26.6-27.6 s; green tick 27.6-28.1 s; NOT RELEASED at 28.5 s.
5. Real-software moments: done. Re-cropped along the panel borders with the pattern and the job header in frame; held 2.0 s (10-12 s) and 3.0 s (37-40 s). The footer shows job #5 (see README).
6. Checks card: done. Job 6's own checkList (28 entries, all PASS, checkCounts PASS 28); the counter is the list length from the data. No 合格 in the film or in the on-screen strings.
7. Limits and labels: done. Limits caption 48 px, ink #15191F on #F5F5F7 (16.2:1), on screen 12-20 s (8 s), 曲げ代 in Japanese. Source labels 40 px (ink3 on paper, 5.2:1). Banners 40 px.
8. AMOEBA beat b: done. "Change one number." kept; "Correction recorded: 150 → 158 mm"; "11 raised · 8 discharged · 3 open"; node colours follow the application's legend (web/i18n.js dg.idle: blue = what a change reaches; the five downstream records are blue; the graph has no dependency nodes, so no amber).
9. Honesty: done. Development-build line on screen 42.8-46.0 s (3.2 s); the approval gate reads "Human approval (not implemented)" (JA 人による承認（未実装）).
10. End card: done. Disclaimer at 40 px for 50-54 s; the JA wording matches flagship2/NEW_STRINGS.md line 39 exactly; the tagline stays; the JA voice credit is on the Japanese film only.
11. One caption wording per element kind: done. Three fixed source labels in each language. Type sizes in code are 132, 48 and 40 px only; a stale 26 px source-label definition and an unused 22 px helper were removed in this pass.
12. Japanese check rows: done. 18 of 28 rows carry a Japanese label; the other 10 show identifiers (list below).

### The lead's four points
1. Empty frames: 9.0 s shows "Dimensions in. Flat pattern out." and the flat pattern; 30.0 s the AMOEBA heading and definition; 41.0 s "Replay: match." with "43 records checked, 0 failing."; 46.0 s the language grid. Of 54 one-second frames per film, every one has ink except 40.0 s (replay beat: the count and its sentence; 1.6 % EN, 2.0 % JA). The count and its sentence are on screen 40.0-42.0 s.
2. Contact sheets and the three paper-only frames: checked (see Reviews).
3. Checks card: the counter and rows come from job 6 (28 entries, PASS 28), not from a fixed text.
4. 20.0 s: the checks card has its heading and counter from the first frame ("It checks its own work.", "1 of 28 checks run").

### Owner additions
- End card contacts: sekishion@chuo-sogo.net and vishnu.s@chuo-sogo.net, exact, under the call to action, 40 px, both languages, on screen 50-54 s. Disclaimer and AMOEBA line kept. NEW_STRINGS.md has an "Owner-supplied contact" section.
- 4K60 masters: rendered from the same page at device scale factor 2 with `tools/render_master.js` (command in README.md); sizes and timings below.

### Fixes made in this pass (after the first revision-5 renders)
- film/core.js had a revision-4 `F3.src` (26 px) defined after the revision-5 one (40 px), so the 26 px version was in effect. Removed; labels are 40 px now.
- Unused 22 px caption helper (`F3.lab`) removed.
- DXF rows: 240 px in 0.6 s (10 rows/s) changed to 144 px (6 lines/s). The rows are clipped to y 934-1002 (they overlapped the panel edge and the source label), and the file-name line moved from y 1000 to 968.
- Replay count: the first frame at 40.0 s reads "1 record checked, 0 failing." (was "1 records").
- End card: the CTA pill enters at 50.4 s (was 51.6 s), so the addresses sit under it from 50.4 s.

### Correction to the earlier check record
- The one-second frames built before this pass came from an older build: against the page renders at 20.0 s and 40.0 s they scored 21.6 and 24.8 dB. They were rebuilt from the final MP4s with exact frame selection. The current final scores 44.1 dB at 20.0 s and 48.0 dB at 40.0 s at full resolution (page renders are JPEG, so the ceiling is below 50 dB). The neighbouring frames score 14-26 dB at 594, 597 and 606 (603 is 43.7 dB, because the checks card holds), so the alignment is exact to the frame. The 54-59 dB figures in the earlier notes were not reproduced and are not used.

### Figures
- 1080p finals: EN 10,444,271 bytes, JA 10,478,711 bytes; 54.000 s; 1,620 frames; H.264 High 1920x1080 yuv420p 30/1 CRF 12, bt709; AAC LC 48 kHz stereo at a 320 kbit/s target (measured averages 278 and 289 kbit/s). Render time 48 s (EN) and 52 s (JA); 0 identical frames; 0 page errors.
- 4K60 masters (`tools/render_master.js --scale 2 --fps 60 --crf 12 --preset slow --abitrate 320`; 0 identical frames; 0 page errors):
  - EN: 23,703,467 bytes; 3840x2160, H.264 High, yuv420p, bt709, 60/1, 3,240 frames, 54.000 s; video 3,221,165 bit/s; AAC LC 48 kHz stereo 278,002 bit/s; overall 3,511,624 bit/s; render 322 s.
  - JA: 24,160,470 bytes; same video parameters; video 3,277,999 bit/s; AAC LC 48 kHz stereo 288,872 bit/s; overall 3,579,328 bit/s; render 351 s.
  - Check frames (2.0, 20.0, 35.0 and 52.0 s) of both masters: layout and text correct; all disclosure captions present.
- Caption-free version: not made (see README.md, "Labelled version"). The coordinator's `labels=0` request was declined; no clean files exist.
- 4K60 test (0-2 s and 34-36 s, both languages): 120 frames each, 13-17 s per clip, sharp at 1:1 (checked at 1600x900 crops). Downscaled to 1080 against the final: 43.8 dB at 1.00 s, 39.6 dB at 35.00 s (no layout shift).

### Reviews made (revision 5)
- One-second contact sheets of both finals (54 tiles each), checked.
- 30 fps strips: 0-2.4 s and the nine slot boundaries (bar - 0.2 s to bar + 0.17 s), both languages, checked.
- Full-size stills at 2.0, 11.0, 27.5, 35.0, 44.0 and 52.0 s, both languages, checked; 390 px composites of the same six per language, checked.
- Full-size frames after the last fix: 26.3 s (DXF band), 29.8 s (file name), 40.0 s (count), 51.0 s (CTA), checked.

### Japanese check rows without an application label (10 of 28)
opening_centre_on_the_declared_delta_side; opening_matches_3d_intersection; opening_developed_length_equals_3d_arc_length; opening_area_equals_surface_area; header_circle_3d_arc_length; saddle_matches_3d_intersection; saddle_on_the_declared_delta_side; saddle_developed_length_equals_3d_arc_length; pattern_area_equals_surface_area; square_end_3d_arc_length.

### Open items
- Native review of the Japanese in `script/onscreen.js` (NEW_STRINGS.md has every key). Two lines need a decision: 曲げ代 in the limits line, and 人による承認（未実装）.
- EN voice credit: not checked against the model terms (not readable from here).
- Scratch clean-up: review frames, sheets, MP4 copies and the 4K test clips are under the session scratchpad; left for the owner to remove.

### Known weaknesses (revision 5)
- 40 px labels are about 8 px at 390 px wide; headlines and numbers read, the source labels and check rows do not read comfortably on a phone.
- Real footage text is small (about 20-30 px at 1080p); the footage shows job #5 in its footer.
- The real footage is one captured frame per moment (see README); the film does not show source motion in those windows.
- Flat-to-3D parts are approximations; the wrong-cut arc is schematic; the hero tee appears only in the hook and checks scenes.

## Revision 4 (history; picture only, audio and the slot grid unchanged)
Checks run: renders of both languages with `errors: 0`; one-second frames; 30 fps frames at the scene-7 beat changes and the 0-2 s opening; boundary frames at 0.1 s steps around nine bar lines and the beat changes (EN); full-size stills (see below); 390 px views of six stills per language; ffprobe on both finals. The finals were rendered three times; the last pass includes all changes listed here.

### Coordinator items 1-13
1. Hook: done. Branch lifts and its blue arc draws from frame 0; push-in 4 % over 0-4 s; words from 0.2 s; amber gap line 0.9-1.0 s; source label from 1.0 s; lockup from 2.0 s, top left, at 40 px (the brief's 28-32 px would break the 40 px rule).
2. Question: partly. The job 5 opening curve draws 4.0-5.0 s and holds; it does not loop or snap. The one-beat lift exit is replaced by the six-frame dissolve at 8 s.
3. Product: done. "Dimensions in. / Flat pattern out." with a 1.6x crop of result frame 33 (REVIEW REQUIRED tag and development-build banner in frame), 10.2-12.0 s.
4. Five parts: done. Light-metal header; the opening is an accent outline with no fill; limits line at 40 px, EN from the brief and JA with the second line from the catalogue (`cn.not`); quantities in whole millimetres.
5. Checks: done. Subtitle "Internal consistency checks — not an approval." at 40 px (JA 内部の整合チェックです。承認ではありません。); counter 1 to 28; rows in product order with ticks. The PASS word is not repeated in each row (the ticks carry it, and the long identifiers would have collided); long EN identifiers are narrowed horizontally to fit. The difference column is removed (SCRIPT: no accuracy figures).
6. DXF: partly. NOT RELEASED stamp at 52 px; file name at 40 px. The 14-line DXF text is removed (it cannot be read at 40 px).
7. AMOEBA: done. (a) 30-33 s: definition at 88 px and three linked records at 72 px values. (b) 33-37 s: "Change one number.", 150 to 158 mm at 160 px, six nodes at 40 px, one pulse, five dependents flagged amber in order, "11 obligations raised" (the 3 open and 8 discharged split is not shown). (c) 37-40 s: real graph core, pushed in from the whole frame. (d) 40-42 s: count to 43, then "Replay: match." after the count.
8. Honesty: done. Job 6 gates by their own results: PASS 5, INCONCLUSIVE 7, NOT IMPLEMENTED 1 (G16 human_approval, JA 技術者承認). Zero categories are not printed. REVIEW REQUIRED once. The 44 px development-build line is on screen for 2.5 s.
9. Language and offline: done. The JA DXF statement is one line ("出力・再読込・照合"; the old break before "・照合" is gone). The grid flips on the 47.0 beat.
10. End card: done, except the address. Disclaimer 40 px for 50-54 s; positioning line 40 px from 52.4 s; JA credit 40 px; CTA label 52 px. No demonstration address was supplied.
11. Repeated captions: done. The top-right development-build caption is removed; it remains inside the footage, in scene 8 and on the end card.
12. Source labels: done. 26 px, bottom left, for the life of each element.
13. Japanese labels: partly. Gate names: G16 only (技術者承認). Node labels: from the catalogue. Check names: 16 of 26 distinct names have a Japanese label (web/i18n.js via data/check-labels.js); the other 10 appear as English identifiers in the JA film: opening_centre_on_the_declared_delta_side, opening_matches_3d_intersection, opening_developed_length_equals_3d_arc_length, opening_area_equals_surface_area, header_circle_3d_arc_length, saddle_matches_3d_intersection, saddle_on_the_declared_delta_side, saddle_developed_length_equals_3d_arc_length, pattern_area_equals_surface_area, square_end_3d_arc_length.

Also: near-empty frames at slot changes are filled by the six-frame dissolve plus earlier entrances; the scene 7 beat changes are cuts.

### Japanese lines new in revision 4 (not reviewed by a native-speaking engineer)
- 板厚ゼロの基準面です。 (first limits line; my translation of "Zero-thickness reference surfaces.")
- 未考慮：板厚、各種の代、溶接ギャップ、カーフ (second limits line; the catalogue's `cn.not`)
- 内部の整合チェックです。承認ではありません。 (checks subtitle)
- 要精査の印：暫定規則、未レビュー (flag rule; "provisional, not yet reviewed")
- 件の精査項目が発生 (obligations raised)
- 件の記録を照合、不一致0件。 (records checked, 0 failing)
- 開発版です。製造用途での検証は未実施です。 (development build line and end-card disclaimer)

### Decisions the owner may want to change
- Lockup at 40 px instead of 28-32 px (the 40 px rule).
- Difference values and the DXF text rows are not shown (SCRIPT: no accuracy figures; 40 px).
- Figures in whole millimetres (a dimension of 3,141.593 mm is shown as 3,142 mm).
- The end card's "Product concept" line is replaced by the development-build line (critics' item; matches the job 6 state).
- The real crop of 37-40 s stops above the rule-table line that names EV-1 (provisional-0.1 (EV-1, unreviewed)).

### Finals and figures
- EN: 9,091,667 bytes; JA: 9,195,115 bytes; both 54.000 s, 1,620 frames, H.264 High 1920x1080 yuv420p 30/1, AAC 48 kHz stereo. Render time about 1 min 40 s each; `errors: 0`.
- `data/film-data.js`: 84,493 bytes. `NEW_STRINGS.md`: 36 keys.
- Folder: 121.4 MB in total (apparent size); 76.9 MB outside the ignored intermediates (`audio/mix-*.wav`, `audio/bed-*.wav`, `audio/vo-*.wav`, `stills/`, `frames/`, `review/`, `node_modules/`, `capture/frames/`).

### Reviews made
- One-second contact sheets of both finals (54 tiles each), checked; the 12-frame sheets are in `final/`.
- 30 fps strips: 0-2 s and the 30, 33, 37, 40, 42 s windows (EN, checked before the last timing pass; JA generated, not reviewed after the last pass).
- Full-size stills: EN 19, 23, 35, 44 and 52 s (checked on the render before the final timing pass; the layouts have not changed since); JA 35 and 44 s (checked on the final). EN and JA 1, 19, 23, 52 s were checked at 640 px tiles and at 390 px.
- 390 px: six stills per language (1, 19, 23, 35, 44, 52 s), checked.

### Open items
- Demonstration address or URL (the end card has none).
- Native review of the Japanese (list above; also the Japanese narration cuts, `use: cut`, awaiting the lead).
- EN voice credit: the model terms are not verified here.
- Scratch clean-up: review frames, contact sheets and two MP4 copies under /tmp/claude-0/-home-user-nexus-fabric-/7d5fb6f5-1c9e-5151-ae27-9132ba7d2536/scratchpad/flagship3/ (about 2.3 GB) are left for the owner to remove; a removal was blocked by the safety check and was not retried.

### Known weaknesses (picture)
- 40 px labels are about 8 px at 390 px wide.
- Real crops are small (application text about 20-30 px at 1080p).
- Header roll and eccentric reducer roll are approximations; the scene 5 joint and the hook curve are drawn for the film.
- The "A" mark is used in the lockup and the end card but is never introduced.
- The DXF content is shown only as plates, a stamp and a file name.
