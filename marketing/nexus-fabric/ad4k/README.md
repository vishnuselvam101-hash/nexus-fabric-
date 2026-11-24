# NEXUS FABRIC 4K advertisement, built on real application screens (30 s, EN and JA)

Made from `STORYBOARD.md`: a short hook, the real screens of the development build's new interface (engine 0.4.10) at 3840×2160, line graphics computed from the real geometry, bold words between the screens on a steady beat, and an end card with one line and a call to action. Nothing on a screen is redrawn; the camera moves and a cyan frame marks the element a caption refers to. The line graphics are computed from the same geometry as the job: header radius 150 mm, branch 100 mm at 60°, developed plate 1200 × 942.478 mm with the opening from the exact intersection.

**Status: draft for the owner's review. Not reviewed by an independent critic or a native Japanese copywriter. Nobody has listened to the soundtrack on studio monitors.**

## Files
- `film.html` — the film's core; `window.seek(seconds)`, `?lang=en|ja`.
- `unroll.js` — the line-drawn header, saddle and unroll, computed from the real geometry.
- `shots-a.js`, `shots-b.js` — the shots, first and second half.
- `screens/*.png` — unedited 3840×2160 captures of the running development build. `rects.json` holds the element positions measured from the running application.
- `render.js` — `node render.js stills|burst|video en|ja`.
- `cues.json` — the sound cue sheet.
- `audio/make_music.py`, `audio/make_bed.py` — score and sound design synthesised in code, no samples; the bed is written at −14 LUFS integrated, −1.5 dBFS true peak.
- `final/NEXUS-FABRIC-ad-4k-en.mp4`, `final/NEXUS-FABRIC-ad-4k-ja.mp4` — H.264 High 5.1, 3840×2160, 30 fps, 30.000 s, AAC 320 kb/s 48 kHz.
- `stills/sheet-en.png`, `stills/sheet-ja.png` — review sheets.

## The cut
| Time | Picture | EN words | JA words |
|---|---|---|---|
| 0–3 | graphic, three word slams | ENTER THE DIMENSIONS. / GET THE FLAT PATTERN. / CHECK EVERY VALUE. | 寸法を入力。 / 板展開を得る。 / すべて検算。 |
| 3–6 | new calculation, in on the nine dimension fields | Nine dimensions in. / Header shell with branch. | 寸法は9項目。 / 母管と枝管の取合い。 |
| 6–9.5 | line-drawn unroll, match-dissolve to the flat pattern viewer | Unrolled to the plate. / 942.478 mm, opening included. | 板に展開。 / 942.478 mm、開口部つき。 |
| 9.5–10.5 | slam | CHECKED. | 検算。 |
| 10.5–14 | job details, down the checks table, then quantities and assumptions | Primary value. Independent reference. / The difference, shown. / Every result on record. | 主計算値と、独立した参照値。 / その差まで表示。 / すべて記録に残る。 |
| 14–17.5 | gates, REVIEW REQUIRED, down the list to the filter chips | 13 release gates. / Nothing passes silently. / Cannot compute? It refuses — it does not guess. | リリースゲートは13。 / 黙って通さない。 / 計算できなければ、推測せず拒否。 |
| 17.5–21 | convert, then the PDF result and its amber pill | PDF lines to DXF, at a stated scale 1:N. / Converted — not verified, not released. | PDFの線をDXFへ。縮尺1:Nを指定（必須）。 / 変換のみ。未検証・未リリース。 |
| 21–23.5 | dark flat pattern with the phone panel rising | English and Japanese. Offline, on the shop's own PC. | 日本語と英語。オフラインで、工場のPC上で動く。 |
| 23.5–25 | graphic, five line icons on a baseline | Five shape families. | 5つの形状ファミリー。 / 円筒／円錐台／偏心異径管／枝管／母管と枝管 |
| 25–30 | end card | NEXUS FABRIC · Flat patterns you can check. · Request a demonstration · [contact] · Development build · not validated · not for manufacturing | NEXUS FABRIC · 検算できる板展開。 · デモのご依頼はこちら · [contact] · 開発版・未検証・製造には使用できません |

## Decisions and limits
- Only implemented capabilities are shown.
- Every frame containing software carries "Actual software · development build" / 「実際の画面・開発版」.
- The PDF path is shown as conversion only: converted, not verified, not released.
- The end card carries the disclaimer "Development build · not validated · not for manufacturing" / 「開発版・未検証・製造には使用できません」.
- `[contact]` is a placeholder; the owner must fill it before publishing.
