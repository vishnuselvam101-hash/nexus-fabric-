# NEXUS FABRIC — 60–75 s Apple-style product film (brief)

Write the STORYBOARD for a 66–72 second software launch film in the style of an Apple product film: calm confidence, black (#000) and near-black stages, one idea per shot, huge short headlines (2–5 words), generous negative space, slow precise camera moves over the real interface (push-ins, parallax, 3D tilt of a screen floating in space with a soft reflection), one accent colour (cyan #35D6E8) used sparingly, a single musical arc (quiet intro → build → reveal → resolved end), whole-number pacing on a 100 BPM grid (one beat = 0.6 s). No voice-over; must read with sound off. Two cuts with identical timing: EN and JA.

## Product facts you may use (and ONLY these — nothing else may be claimed)
- NEXUS FABRIC turns sheet-metal dimensions into a flat pattern (DXF) that the user can check. Five shape families: cylinder shell, cone frustum, eccentric reducer, branch stub, header shell with branch.
- Every result is computed twice: primary value and an independent reference, with the difference shown. 13 release gates; nothing passes silently. If it cannot compute, it refuses — it does not guess. Every result is on record (audit).
- A part's DXF is withheld while any check on its cut geometry is INCONCLUSIVE (new behaviour).
- PDF to DXF: converts the vector lines of a PDF page at a scale the user states (1:N); the result is labelled "converted — not verified, not released". It does NOT read or understand drawings; never imply that.
- English and Japanese. Runs offline on the shop's own PC (desktop) or a shop server; also usable on a phone browser.
- Simplified interface: open the app → drop a PDF or enter dimensions → result; all other tools sit behind "More".
- Real numbers available: header plate 942.478 mm × 1200 mm (2π·150) with a 720-point branch opening; branch with a 721-point saddle cut (header r 150, branch r 100, angle 60°); 13 gates.
- Status: development build, not validated, not for manufacturing. End card must say so.
- Seller/contact: chuo-sogo.net · sekishion@chuo-sogo.net · vishnu.s@chuo-sogo.net. CTA: "Request a demonstration" / 「デモのご依頼はこちら」. Positioning line: "Flat patterns you can check." / 「検算できる板展開。」
- FORBIDDEN words/ideas: approved, certified, validated, production-ready, error-free, guaranteed, secure, smart, intelligent, any claim of automated intelligence, automatic drawing understanding, any sparkle/scan/"reading the drawing" motif, invented customers or numbers.

## Available real assets (screens are 3840×2160 captures of the real UI, dark and light themes; push-ins up to 2.2× stay sharp)
screens/dark|light/: 01-login (sign in), 02-convert (PDF drop zone "Convert a PDF to DXF", scale 1:N), 03-result (amber "CONVERTED FROM PDF — NOT VERIFIED — NOT RELEASED" pill + "Download DXF · not released"), 04-workspace (More: users/tools), 05-mobile (phone 780×1688, convert screen), 06-ja (phone, Japanese), 07-math (new calculation form, 9 dimension fields), 08-job (job with REVIEW REQUIRED banner + gate list), 09-pattern (flat pattern viewer: header plate with the opening), 10-admin, 11-job-ja (Japanese job), 12-i18n-keys (AMOEBA centre in Japanese). Note the UI content occupies roughly the central 1280×720 css px of each frame — plan crops/push-ins accordingly.
data/header_with_branch.json: real flat-pattern curves (header rectangle + opening polyline; branch saddle polyline) — can be drawn as animated vector lines in the film (true geometry, line-drawing style, e.g. a 3D cylinder with branch that unrolls into the flat plate).
Fonts: Inter (Regular/Medium/SemiBold/Bold) for EN, Noto Sans CJK JP for JA.

## Tool constraints (the film is built in EffectCraft by After Effects-style scripts)
Possible: solids, text with per-character animators, shape layers (paths with keyframed vertices, trim paths, strokes, fills, gradients), image layers with masks/feather, 3D layers + one camera with depth of field, Gaussian/Camera Lens blur, glow, drop shadow, opacity/position/scale/rotation keyframes with easing, precomps, motion blur. Each SCENE is built as its own composition (3840×2160, 60 fps) and rendered separately; the edit and audio are assembled afterwards. Music and sound effects are synthesised in code (no generators); describe the cue per scene.

## Output (Markdown, under 220 lines)
1. One-paragraph concept and the headline sequence (EN + JA).
2. A table of 9–12 SCENES: id, start–end (s, on the 0.6 s grid, total 66–72 s), picture/camera (exact asset, crop region in px of the 3840×2160 frame, scale/move values), EN text, JA text (natural Japanese, not literal), motion notes (easing, timings in s), sound cue.
3. Typography and colour spec (sizes in px at 4K, weights, tracking, the status caption "Actual software · development build" / 「実際の画面・開発版」 on every software shot).
4. Music arc (BPM 100, key, sections with times) and a short SFX list.
5. Truth-rule checklist.
