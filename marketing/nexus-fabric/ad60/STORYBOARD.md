# NEXUS FABRIC — Launch Film Storyboard
**Runtime 70.8 s · 118 beats @ 100 BPM · 3840×2160 · 60 fps · 4248 frames · EN + JA, identical cut**

## 1. Concept

A black room. One part, drawn in true geometry, turns from a cylinder with a branch into a flat plate — and the film spends the rest of its time proving that the plate can be checked. Every shot is one idea: a headline of two to five words in the left third, the real interface floating as a tilted card in the right two-thirds with a soft reflection, and nothing else. The camera never hurries; it pushes in on the exact row of the interface that carries the claim (the reference value, the gate list, the amber pill, the disabled download). Cyan appears six times in seventy seconds and nowhere else. The film's argument is restraint: it shows what the software refuses to do as prominently as what it does, and it ends on a development-build card, not a promise. No voice-over; every claim is legible with sound off.

**Headline sequence (EN / JA)**
1. Round to flat. / 曲面を、平面に。
2. Five shape families. / 5つの形状。
3. Enter the dimensions. / 寸法を入力。
4. Computed twice. / 2通りで計算。
5. 13 gates. / 13の関門。
6. It won't guess. / 推測はしない。
7. 942.478 mm. Exactly. / 942.478 mm。そのまま。
8. PDF lines. Your scale. / PDFの線を、指定の縮尺で。
9. Converted. Not verified. / 変換のみ。未検証。
10. Less on screen. / 画面は、シンプルに。
11. Offline. In your shop. / オフライン。自社の環境で。
12. Flat patterns you can check. / 検算できる板展開。

## 2. Scene table

**Capture convention:** content box **C = x 640–3200, y 360–1800** (2560×1440 px) inside each 3840×2160 capture; outside C is flat page background, safe to include, no browser chrome. Crop scale *s* = 3840 / crop-width; **no crop exceeds 2.20×**. Cards are displayed at ≤1.0× of cropped pixels (never upscaled).
**Layouts:** **L1** stage (black + vector, centred headline). **L2** card 1728×972 at (2660, 1080), rotY −7°→−4°, rotX +2°, headline column x 280–1620. **L3** hero card 2048×1152 at (1920, 1290), rotY −4°→−2°, headline centred, baseline y 480. All cards: mirrored reflection at 18% opacity, Gaussian blur 48 px, gradient mask over 420 px, +24 px offset; camera lens blur 6 px on reflection only; one 3D camera per comp, DOF f/2.8 focused on the card plane.
**Easing:** all moves cubic Bézier (0.22, 0.61, 0.36, 1). Headlines: per-character animator, opacity 0→100, y +28→0, 0.30 s per char, 0.02 s stagger, in at scene+0.6 s, out over last 0.3 s. Cuts are hard on the grid; only fade-up 0.0–0.6 s and fade-out 69.6–70.8 s.

| id · frames | start–end | picture / camera (source px) | EN text | JA text | motion | sound |
|---|---|---|---|---|---|---|
| S01 · 360f | 0.0–6.0 | L1. `data/header_with_branch.json` drawn as 3D wireframe: header cylinder r150 + branch r100 @60°, white 2 px strokes on #000. Camera orbits 14°→0° yaw, z −2600→−2480. | **Round to flat.** · caption `header r150 · branch r100 · 60°` | **曲面を、平面に。** · caption `母管 r150 · 枝管 r100 · 60°` | Trim paths 0→100%: header 0.6–2.4, branch 2.4–3.6 (cyan #35D6E8, only accent). Orbit 0.0–6.0. Headline in 1.2. | Sub sine D1 swell −30 dB; felt piano D3 at 1.2, A2 at 3.6; room-air bed. |
| S02 · 324f | 6.0–11.4 | L1. Five wireframes from the same primitive set, 5-across at y 1000, each 480 px wide, x 620/1240/1860/2480/3100. Static camera, plane scale 100→103%. | **Five shape families.** · labels `Cylinder shell · Cone frustum · Eccentric reducer · Branch stub · Header shell with branch` | **5つの形状。** · labels `円筒 · 円錐台 · 偏心異径管 · 枝管 · 母管と枝管` | Trim-path draw staggered 0.30 s each from 6.6; labels fade 0.3 s after each draw. Headline in 6.6. | Five soft plucks (sine, 90 ms decay) on 6.6/6.9/7.2/7.5/7.8; pad enters 8.4. |
| S03 · 360f | 11.4–17.4 | L2. `screens/dark/07-math`. Crop A x640–3200 y360–1800 (1.50×) → B x935–2905 y626–1734 (1.95×, centre 1920,1180) on the 9 dimension fields. | **Enter the dimensions.** · caption `Actual software · development build` | **寸法を入力。** · caption `実際の画面・開発版` | Push 11.4–17.4. Card scale 100→104%, x 2660→2630. Cyan underline on one active field, 12.6–13.2. | 9 ui-ticks at 0.15 s from 12.6; pulse (8th-note marimba) enters 11.4. |
| S04 · 360f | 17.4–23.4 | L2. `screens/dark/13-checks` (JA: `14-checks-ja`): the real checks panel, rows 'outline extent around header' and 'shell edge equals circumference' showing Primary 942.478 / Reference 942.478 / Difference 0.000. Crop A x640–3200 y80–1520 (1.50×) → B x700–2445 y700–1682 (2.20×) on those two cards. | **Computed twice.** · sub `Primary value · independent reference · difference shown` | **2通りで計算。** · sub `主計算・独立した参照計算・差分を表示` | Push 17.4–23.4, rotY −7°→−4°. Three column labels fade in 18.6 / 19.2 / 19.8 (0.3 s each). No highlight boxes. | Kick on beats 1 & 3 from 17.4; low pad Dm; one soft tick per label. |
| S05 · 324f | 23.4–28.8 | L2. `screens/dark/08-job` wide gate list. Crop A = C (1.50×) → B x735–3105 y453–1787 (1.62×, centre 1920,1120). JA: `11-job-ja`. | **13 gates.** · sub `Nothing passes silently.` | **13の関門。** · sub `黙って通さない。` | 13 gate rows brighten 0→100% opacity in sequence, 0.15 s apart, 24.6–26.55; cyan counter "13" fades in 26.6. Card scale 100→103%. | 13 gate-thunks at 0.15 s from 24.6; filtered 16th hats enter 23.4. |
| S06 · 324f | 28.8–34.2 | L2. `screens/dark/08-job`, REVIEW REQUIRED banner + INCONCLUSIVE row. Crop A x823–3017 y263–1497 (1.75×) → B x984–2856 y373–1427 (2.05×, centre 1920,900). JA: `11-job-ja`. | **It won't guess.** · sub `If it cannot compute, it refuses.` | **推測はしない。** · sub `計算できなければ、拒否します。` | Push 28.8–33.0, then hold 1.2 s. Card motion stops dead at 33.6 (music drops out). No shake, no red flash. | Riser 32.4–34.2; refuse-damp at 29.4; **all music out 33.6–34.2**. |
| S07 · 432f | 34.2–41.4 | L3. Reveal: `data/header_with_branch.json` 3D cylinder+branch unrolls (keyframed vertices) into the flat plate, 34.2–36.6; cross-dissolve 0.6 s into `screens/dark/09-pattern`. Crop A = C (1.50×) → B x942–3018 y536–1704 (1.85×, centre 1980,1120). | **942.478 mm. Exactly.** · sub `The plate is the circumference: 2π × 150.` · caption `942.478 mm × 1200 mm · 720-point opening` | **942.478 mm。そのまま。** · sub `板の幅は円周そのもの：2π × 150。` · caption `942.478 mm × 1200 mm · 開口 720点` | Unroll 34.2–36.6 (ease-out). Opening polyline redraws in cyan 37.2–38.4, then holds. The 942.478 mm dimension line draws with arrowheads 38.4–39.0 and the value wipes in whole (no count-up). Push 36.6–41.4. | Reveal downbeat 34.2: pad Dm–Bb–F–C, half-time kick/snare, sub D1. hold-latch at 35.4. |
| S08 · 324f | 41.4–46.8 | L2. `screens/dark/02-convert`, drop zone + scale 1:N field. Crop A x720–3120 y365–1715 (1.60×) → B x936–2904 y506–1614 (1.95×, centre 1920,1060). | **PDF lines. Your scale.** · sub `It does not read the drawing.` | **PDFの線を、指定の縮尺で。** · sub `図面の内容は読み取りません。` | A PDF page card drops 60 px into the zone 42.0–42.6 (ease-out, no bounce). Scale field shows `1:N`. Push 41.4–46.8. No scan line, no sweep, no sparkle. | paper-drop at 42.0; groove strips to pulse + pad. |
| S09 · 324f | 46.8–52.2 | L2. `screens/dark/03-result`, amber pill + "Download DXF · not released". Crop A x882–2958 y316–1484 (1.85×) → B x1028–2773 y424–1406 (2.20×, centre 1900,915). | **Converted. Not verified.** · sub `Not released.` | **変換のみ。未検証。** · sub `未リリース。` | Push 46.8–50.4, hold 1.8 s. Pill captured verbatim, amber untouched; surrounding card desaturated 12% so amber is the only warm pixel. | stamp-soft at 47.4; pad sustains, hats out. |
| S10 · 288f | 52.2–57.0 | L2. `screens/light/04-workspace` (only light-theme shot). Crop A = C (1.50×) → B x911–3169 y485–1755 (1.70×, centre 2040,1120) drifting toward "More". | **Less on screen.** · sub `Everything else behind More.` | **画面は、シンプルに。** · sub `ほかのツールは「その他」の中に。` | Card drift x 2660→2600, push 52.2–57.0. "More" label fades up 53.4. Stage stays #000; headline stays #F2F4F5. | Pad thins; single piano note at 53.4; pulse continues. |
| S11 · 324f | 57.0–62.4 | L1 composite. `screens/dark/02-convert` crop C as desktop tile 1400×788 at (2280, 760) rotY −8°; `screens/dark/05-mobile` 620×1341 at (2560, 1500) rotY −12°; `screens/dark/06-ja` 560×1211 at (3120, 1560) rotY −16°. JA cut: `06-ja` leads at (2560,1500), `05-mobile` second. | **Offline. In your shop.** · sub `Desktop · shop server · phone browser` · label `English / 日本語` | **オフライン。自社の環境で。** · sub `PC・社内サーバー・スマホのブラウザ` · label `English / 日本語` | Three planes settle in 57.0–58.2 (stagger 0.3 s), then 3% parallax drift to 62.4. One cyan hairline links the three, drawn 58.8–59.4. No cloud, no network icon. | Instruments reduce to piano + pad; two soft plucks on the phone entries. |
| S12 · 504f | 62.4–70.8 | L1. #000. Cyan hairline 2×640 px at (1920, 700). Wordmark, positioning line, CTA, contacts, status — all centred, stacked. | `NEXUS FABRIC` · **Flat patterns you can check.** · `Request a demonstration` · `chuo-sogo.net · sekishion@chuo-sogo.net · vishnu.s@chuo-sogo.net` · `Development build · not validated · not for manufacturing.` | `NEXUS FABRIC` · **検算できる板展開。** · `デモのご依頼はこちら` · `chuo-sogo.net · sekishion@chuo-sogo.net · vishnu.s@chuo-sogo.net` · `開発版・未検証・製造には使用できません。` | Hairline trim 0→100% 62.4–63.0. Wordmark in 63.0, positioning 63.6, CTA 65.4, contacts 66.6, status 67.8 (each 0.3 s). Hold to 69.6, fade to black 69.6–70.8. | D major add9 pad at 62.4; piano D3 at 63.6; sub decays to silence 69.6; 1.2 s black tail. |

## 3. Typography & colour

| Role | EN | JA | Size @4K | Weight | Tracking | Colour |
|---|---|---|---|---|---|---|
| Headline | Inter | Noto Sans CJK JP | 152 px, leading 176, ≤2 lines, column 1340 px | SemiBold | −2% (EN) / 0 (JA) | #F2F4F5 |
| Sub-line | Inter | Noto Sans CJK JP | 56 px, leading 76 | Regular | 0 / +2% | #B7BEC1 |
| Technical caption | Inter | Noto Sans CJK JP | 44 px | Regular | +1% | #8A9296 |
| **Status caption (every software shot, S03–S11)** | `Actual software · development build` | `実際の画面・開発版` | 44 px | Regular | +1% | #8A9296 |
| End wordmark | Inter | Inter | 96 px | Bold | +8% | #F2F4F5 |
| End CTA | Inter | Noto Sans CJK JP | 64 px | Medium | +2% | #35D6E8 |
| End contacts | Inter | Inter | 44 px | Regular | +1% | #8A9296 |
| End status | Inter | Noto Sans CJK JP | 40 px | Regular | +1% | #6E7679 |

- Placement: L2 headline block left-aligned at x 280, optical centre y 1080; sub-line 40 px below; status caption baseline y 2058 at x 280. L3 headline centred, baseline y 480; status caption centred, baseline y 2058. Title-safe 280 px all sides.
- Palette: stage #000000, card surround #0B0D0E, primary text #F2F4F5, secondary #B7BEC1 / #8A9296, accent **#35D6E8** only.
- Accent budget: S01 branch curve, S03 field underline, S05 counter, S07 opening polyline, S11 link line, S12 hairline + CTA. Max two accent elements per shot; total accent-on-screen ≈ 10 s. UI amber in S09 is captured, never recreated or recoloured. No gradients on text, no glow above 8 px radius, no lens flares, no sparkles.
- JA cut: identical frames, identical crops and moves. JA captures substituted only where they exist (`11-job-ja` in S04–S06, `06-ja` in S11); all other screens keep the real EN capture with JA overlay text. **Never fabricate a Japanese UI.** `10-admin` and `12-i18n-keys` held in reserve, unused.

## 4. Music & SFX

**100 BPM · 1 beat = 0.6 s · D Aeolian, resolving to D major add9 · all instruments synthesised in code (2-op FM felt piano, sine sub, triangle/sine pluck, filtered saw pad, noise hats).**

| Section | Time | Content |
|---|---|---|
| Intro | 0.0–11.4 | Sub D1 swell, felt piano single notes, room-air. No percussion. −24 LUFS. |
| Build A | 11.4–23.4 | 8th-note pluck pulse enters 11.4; kick beats 1 & 3 from 17.4; pad Dm from 17.4. |
| Build B | 23.4–33.6 | Filtered 16th hats, pad opens (cutoff 400→4000 Hz), riser 32.4–34.2. |
| Silence | 33.6–34.2 | Everything out. One beat of nothing before the reveal. |
| Reveal | 34.2–41.4 | Full pad Dm–Bb–F–C, half-time kick/snare, sub D1. Peak −16 LUFS. |
| Sustain | 41.4–57.0 | Strip to pulse + pad at 41.4; hats out 46.8; one piano note per scene change. |
| Resolve | 57.0–62.4 | Pulse out; piano + pad only. |
| End | 62.4–70.8 | D major add9 (D–F♯–A–E) at 62.4, piano D3 at 63.6, sub decay to silence 69.6, black tail 69.6–70.8. |

**SFX (all synthesised, ≤ −18 dBFS, none brighter than 4 kHz):** `ui-tick` 12 ms 1.2 kHz sine + noise click · `gate-thunk` 60 Hz sine 80 ms + soft click · `refuse-damp` low-passed noise thud, dry · `hold-latch` descending minor third, 0.3 s · `paper-drop` 120 ms band-passed noise · `stamp-soft` 180 Hz body + short noise · `reveal-air` reverse noise swell 32.4–34.2 · `room-air` pink-noise bed −42 dB, ducked 6 dB under music. **No sparkle, scan, telemetry or "reading" sounds.**

## 5. Truth-rule checklist

1. Only the five listed shape families are named or drawn (S02). No sixth shape, no invented capability.
2. Dual computation shown as primary / independent reference / difference (S04) — never "error-free", never a tolerance claim.
3. 13 gates counted on screen, one row per gate (S05); "Nothing passes silently" is the stated behaviour, not a quality claim.
4. Refusal shown as a stopped state (S06): "If it cannot compute, it refuses." No guess, no fallback, no auto-fix implied.
5. S07 states only the true circumference (2π × 150 = 942.478 mm) shown in the real checks/pattern; the withheld-DXF behaviour is NOT claimed in this film (no capture shows it).
6. PDF path (S08) says "PDF lines. Your scale." plus "It does not read the drawing." No scan sweep, no highlight crawl, no recognition motif in picture or sound.
7. S09 pill reproduced verbatim from the capture: CONVERTED FROM PDF — NOT VERIFIED — NOT RELEASED; headline adds nothing beyond "Converted. Not verified. / Not released."
8. All numbers are the supplied real ones: 942.478 mm × 1200 mm (2π·150), 720-point opening, 721-point saddle available to S01/S02 captions, r150 / r100 / 60°, 13 gates, 9 dimension fields, 1:N, and the checks panel values 942.478 / 942.478 / 0.000. No others appear.
9. Geometry animation is driven by `data/header_with_branch.json`; no hand-drawn approximation of the curves.
10. Offline / desktop / shop server / phone browser and EN + JA shown as deployment facts (S11); no cloud, uptime, security or speed claim.
11. Status caption on every software shot S03–S11; end card carries "Development build · not validated · not for manufacturing." / 「開発版・未検証・製造には使用できません。」 — the word *validated* appears **only** in this required negation.
12. Forbidden-word scan across both cuts: approved, certified, validated (except §11), production-ready, error-free, guaranteed, secure, smart, intelligent, any claim of automated intelligence, automatic drawing understanding — zero occurrences. No customers, logos, testimonials or invented metrics.
13. Sound-off read: every claim is on-screen text; nothing is carried by audio alone. EN and JA share frame counts (4248 f total) and every cut point.
