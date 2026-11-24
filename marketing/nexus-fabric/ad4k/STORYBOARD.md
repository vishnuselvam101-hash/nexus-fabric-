# NEXUS FABRIC — 30 s storyboard (ad4k)

**Format** 3840×2160, 30 fps, 30.000 s (900 frames), two cuts: EN and JA, identical picture timing.
**Audience** production engineers and buyers at Japanese plate/duct fabrication shops (first customers); English-speaking fabricators second.
**Assets** real captures in `ad4k/screens/` (development build, engine 0.4.10). Nothing on a screen is redrawn, recoloured or retouched; the camera moves and a cyan frame marks the element a caption refers to. Highlight rectangles are measured from the captures into `ad4k/screens/rects.json` before animation — no guessed coordinates.
**Status caption** every frame containing software carries, bottom-right, 34 px: `Actual software · development build` / 「実際の画面・開発版」. Where a JA cut must use an English-only capture, append 「・画面は英語版」.

## The cut

| Time (s) | Picture · camera | EN on-screen text | JA on-screen text | Motion notes | Sound cue |
|---|---|---|---|---|---|
| 0.0–3.0 | **Graphic.** Dark steel field, thin cyan ruler draws across the lower third. Three word slams at 0.0 / 1.0 / 2.0. No software frame, no status caption. | ENTER THE DIMENSIONS.<br>GET THE FLAT PATTERN.<br>CHECK EVERY VALUE. | 寸法を入力。<br>板展開を得る。<br>すべて検算。 | Mask reveal left→right, 0.28 s fast-in hard stop; slam 2 vertical stretch with 0.34 s damped bounce; slam 3 letter-spacing collapse 0.34em→−0.03em (JA →0.02em). Ruler ticks snap on each hit. Exit 0.14 s up-fade. | Three sub hits (0.0/1.0/2.0), steel tick per ruler mark, no music yet |
| 3.0–6.0 | **04-new-calculation.png** / **20-ja-new-calculation.png**. 1.0× full frame → push to 1.9× on the nine dimension fields by 3.9, 0.4% drift to 6.0. Cyan frame on the field group; second frame on the "scope of this build" line at 5.2. | Nine dimensions in.<br>Header shell with branch. | 寸法は9項目。<br>母管と枝管の取合い。 | Camera ease out4, no overshoot on screens. Caption settles from +30 px, 0.34 s. Field frame grows 26 px→10 px padding with damped bounce. Panel boundaries respected at rest; text clipped only while moving. | Bed enters at 3.0 (120 BPM), servo whirr under push, one tick per framed field |
| 6.0–9.5 | **Graphic → 06-job-flat-pattern.png** / **16-ja-flat-pattern.png**. Line-drawn header cylinder with branch saddle 6.0–7.2; seam splits and unrolls 7.2–8.2 into the plate outline; match-dissolve to the real viewer at 8.2, camera 1.0×→1.6× on the branch opening. | Unrolled to the plate.<br>942.478 mm, opening included. | 板に展開。<br>942.478 mm、開口部つき。 | Unroll is a line drawing only, geometry matched to the real 942.478 mm header plate. Dimension line draws with arrowheads; the value **wipes in whole** in JetBrains Mono — no count-up, no intermediate numbers. mm rulers, seam marks and curve names stay legible at rest. | Sheet-metal unroll, dimension tick, lock click at 8.2 on the beat |
| 9.5–10.5 | **Graphic slam.** Cyan caliper jaws close on the word from both edges. | CHECKED. | 検算。 | Vertical stretch-in, jaws meet at 9.82 and stop dead; 0.14 s exit. JA set in Noto Sans JP 700, no letter-spacing collapse below 0.02em. | Hard hit + caliper snap; bed ducks 0.3 s |
| 10.5–14.0 | **07-job-details.png** / **21-ja-job-details.png**. 1.4×→2.4× on the checks table; cyan frame travels down three rows (primary / reference / difference columns), then a short pull at 13.0 to quantities and assumptions. | Primary value. Independent reference.<br>The difference, shown.<br>*(13.0)* Every result on record. | 主計算値と、独立した参照値。<br>その差まで表示。<br>*(13.0)* すべて記録に残る。 | Frame hops on the half-second grid, one row per hop; column headers stay in frame at every rest. Caption swap on a 0.12 s cross-fade, no slide. | Three dry ticks (row hops), low pad, audit line lands on a soft click |
| 14.0–17.5 | **05-job-gates.png** / **15-ja-job-gates.png**. Open 1.6× on REVIEW REQUIRED / 要レビュー, tilt down the gate list, hold the filter chips 16.2–17.4 with all four counts uncropped: 5 PASS · 6 INCONCLUSIVE · 1 NOT IMPLEMENTED · 1 NOT RUN. | 13 release gates.<br>Nothing passes silently.<br>*(16.0)* Cannot compute? It refuses — it does not guess. | リリースゲートは13。<br>黙って通さない。<br>*(16.0)* 計算できなければ、推測せず拒否。 | Chips keep the product's own colours; cyan underline only on the gate reason text. No green success glow anywhere. Camera rests on panel boundaries so no chip label is cut. | Four gate clunks (one per chip), bed drops to pulse under the refusal line |
| 17.5–21.0 | **02-convert.png** / **14-ja-convert.png** (17.5–19.0): push on the PDF drop area and the required scale field 1:N. Beat cut to **03-pdf-result.png** / **22-ja-pdf-result.png** (19.0–21.0): 1.2×→1.8× on the amber pill; "Download DXF — not released" stays in frame. | PDF lines to DXF, at a stated scale 1:N.<br>*(19.0)* Converted — not verified, not released. | PDFの線をDXFへ。縮尺1:Nを指定（必須）。<br>*(19.0)* 変換のみ。未検証・未リリース。 | Accent switches to amber for this shot only. Pill mask-reveals left→right and is never cropped, blurred or covered by a caption. No sparkle, no "reading the drawing" motif of any kind. | Plotter feed, dull amber thud at 19.0; deliberately no bright chime |
| 21.0–23.5 | **17-dark-flat-pattern.png** (EN) / **16-ja-flat-pattern.png** (JA), with **19-phone-job.png** rising as a 390 px portrait panel at 22.2 in 2.5D parallax. JA cut appends 「・画面は英語版」 to the status caption while the phone capture is on screen. | English and Japanese. Offline,<br>on the shop's own PC. | 日本語と英語。オフラインで、<br>工場のPC上で動く。 | Phone panel rises 60 px with a soft contact shadow, settles 0.5 s damped; background plate parallaxes 12 px. Cyan frame on the language/theme control, not on a result. | Room tone, one soft hit at 22.2, bed opens up |
| 23.5–25.0 | **Graphic.** Five line icons draw in a row on a cyan baseline, mono labels beneath: cylinder shell · cone frustum · eccentric reducer · branch stub · header shell with branch. | Five shape families. | 5つの形状ファミリー。<br>円筒／円錐台／偏心異径管／枝管／母管と枝管 | Icons draw 0.18 s each, staggered 0.12 s; labels fade with them. JA labels on one line, separated by ／, no orphan characters. Riser carries into the end card. | Five ascending ticks, short riser 24.4–25.0 |
| 25.0–30.0 | **End card.** Cyan mark = the developed saddle curve, drawn from the real pattern geometry. Wordmark, positioning line, CTA, contact placeholder, disclaimer. | NEXUS FABRIC<br>Flat patterns you can check.<br>Request a demonstration<br>[contact]<br>Development build · not validated · not for manufacturing | NEXUS FABRIC<br>検算できる板展開。<br>デモのご依頼はこちら<br>[contact]<br>開発版・未検証・製造には使用できません | Mark draws 25.05–25.75; wordmark settles 25.3; line 25.85; CTA bounce 26.5 (0.86→1.00); disclaimer fades in 25.2–25.8 and holds. The only picture held ≥1 s in the film. | Final chord at 25.0, tail decays to 29.8, last 0.2 s clean |

## Language and asset map

- EN cut: 02, 03, 04, 05, 06, 07, 17, 19. JA cut: 14, 22, 20, 15, 16, 21 (+ 19 with the English-screen addendum).
- Captions, slams, status line and end card are the only text authored outside the captures. All numbers come from the captures: 9 fields, 13 gates, 5/6/1/1 chips, 942.478 mm, 1:N, 390 px, five families.
- JA typography: Noto Sans JP, two balanced lines maximum per caption, kinsoku applied, no single-character last line, no break inside 942.478 mm or 1:N.

## Palette

| Role | Hex |
|---|---|
| Background steel | `#0B0E11` |
| Panel steel | `#141A20` |
| Rule / dimension line | `#2A333C` |
| Cyan measurement accent | `#35D6E8` |
| Cyan deep (fills, glows) | `#0E7C8A` |
| Primary ink | `#F2F6F8` |
| Secondary ink | `#9AA7B2` |
| Caution amber (PDF conversion, disclaimer dot) | `#E8A33D` |

Screen content keeps its own colours. Amber is used only where the product itself is amber; no green is introduced anywhere in the film.

## Type (all offline)

- **Noto Sans** 400 / 600 / 800 — EN captions 76 px, slams 200–330 px auto-fit to 3000 px width, end-card line 84 px.
- **Noto Sans JP** 500 / 700 / 900 — JA captions 68 px, slams 190–300 px, letter-spacing 0.02em; end-card line 76 px.
- **JetBrains Mono** 400 / 700 — dimensions, counts, gate chips labels, icon labels, `1:N`, `942.478 mm`, `[contact]`. Tabular figures on.
- Status caption 34 px Noto Sans 600 / Noto Sans JP 500, 0.09em (EN) / 0.04em (JA), on a 88% steel plate.

## Sound design

- Bed: 120 BPM, half-second pulse grid; every cut and slam lands on it (0, 1, 2, 3, 6, 8.2, 9.5, 10.5, 14, 17.5, 19, 21, 23.5, 25).
- Layers: low steel drone, muted kick on slams, dry caliper ticks on measurement beats, plotter feed under the PDF shot, one sheet-metal unroll at 7.2.
- Hit points: hard hits at 0/1/2 and 9.5; gate clunks 14.4/14.9/15.4/15.9; amber thud 19.0 (duller, no chime); riser 24.4–25.0; final chord 25.0.
- Mix: −14 LUFS integrated, true peak −1.2 dBTP, no clipping, no silence longer than 0.2 s before 29.8. Bed ducks 3 dB under every caption entrance.
- No voice-over in either cut; the film must read with sound off.

## Truth-rule checklist

- [ ] Every software frame carries `Actual software · development build` / 「実際の画面・開発版」, legible at 1080p downscale.
- [ ] REVIEW REQUIRED / 要レビュー and the amber CONVERTED FROM PDF — NOT VERIFIED — NOT RELEASED pill are never cropped, covered or dimmed.
- [ ] "Download DXF — not released" visible whenever the PDF result is on screen.
- [ ] Gate chips shown exactly as the product shows them: 5 PASS, 6 INCONCLUSIVE, 1 NOT IMPLEMENTED, 1 NOT RUN, total 13. No chip is hidden or reordered.
- [ ] No word or image implying approved, certified, validated, production-ready, error-free, guaranteed, secure, smart, intelligent, any claim of automated intelligence, or automated drawing understanding. No sparkle, scan-and-understand or "reading the drawing" motif.
- [ ] PDF path is described as conversion of vector lines at a stated scale only, always paired with not verified / not released.
- [ ] Only implemented capabilities shown: five shape families, flat pattern, independent check with primary/reference/difference, 13 gates, refusal instead of guessing, conventions in the DXF, audit record, EN/JA, offline on the shop's PC.
- [ ] Every number on screen exists in the captures; the 942.478 mm dimension wipes in whole, never counts up.
- [ ] End card carries the positioning line, "Request a demonstration" / 「デモのご依頼はこちら」, `[contact]` placeholder and "Development build · not validated · not for manufacturing" / 「開発版・未検証・製造には使用できません」.
- [ ] No invented company name, address, phone, URL or customer logo. No certification marks.
- [ ] JA cut reviewed by the localisation specialist; no orphan characters, no machine-translated gate terminology.

## Render and QC

- 3840×2160, 30 fps, 900 frames, 30.000 s, both cuts; decode-error scan, black-frame scan, loudness log.
- Frame grabs at 2 fps for review; full-resolution grabs of every resting frame to confirm no clipped panel text and no cropped warning.
- Held picture ≥1 s: end card only. 9:16 version not made.
