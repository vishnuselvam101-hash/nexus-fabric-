# On-screen text that is not in the narration — flagship film 2

Source of truth: `script/narration.json`, key `onscreen` (both languages). `script/narration.js` is generated from it by
`tools/build_script.py`. Words that appear inside product screens are the application's own and are not listed here.
Numbers, equation symbols and DXF lines are computed from the product's output and are not listed.

Sizes (current look): statement lines 96–132 px, centred, one per scene, on the wallpaper; secondary lines 32 px under
the window or in the film's own scenes; small print 22 px, mid-grey. "Statement" below means a line of the first kind.

"In narration" means the sentence appears word for word in the spoken line for that shot. Most captions paraphrase the
narration, so they are new text. The Japanese wording has **not been reviewed by a native-speaking engineer**.

| Key | English | 日本語 | Shot (treatment) | Size and place | In narration |
|---|---|---|---|---|---|
| hook | One curve. | 一本の曲線。 | 1 | statement, top centre | no (narration: "there is one curve") |
| hook2 | Get it wrong, and nothing fits. | 誤れば、合いません。 | 1 | statement, top centre | yes (L1, second sentence) |
| cap_form | The dimensions of the joint | 取合いの寸法を入力 | 3 | secondary, under the window | no |
| cap_result | Both flat patterns, calculated | 二枚の板展開を計算 | 4 | secondary, under the window | no (narration: "calculates both flat patterns") |
| cap_eq | One equation. Every point on the cut. | ひとつの式で、切断線のすべての点。 | 6 | secondary, bottom left | no |
| check_title | Interface check | 部材間の取合い確認 | 7 | secondary, under the window | no |
| cap_fold | Folded back: one curve | 立体に戻すと、一本の曲線 | 7 | statement, top centre | no |
| cap_records | Every step recorded. Replay matches. | すべて記録。再実行で一致。 | 8 | secondary, under the window | no |
| cap_gates | What it cannot confirm, it shows | 確認できないことは、隠さない | 9 | secondary, under the window | yes, as a clause of L6 |
| cap_dxf | Written. Read back. Compared. | 出力・再読込・照合 | 10 | statement, top centre | no |
| label_film | Visualisation made for this film · not a product view | 本映像用のビジュアル・製品画面ではありません | the film's own drawings | small print | no |
| label_actual | Actual software · development build | 実際のソフトウェア・開発版 | product scenes | small print, after the secondary line | no |
| label_output | Actual product output · development build | 製品の実際の出力・開発版 | the product's numbers and the DXF | small print | no |
| check_diff | difference | 差 | 7 (film's own panel, not used now) | — | no |
| check_tol | tolerance | 許容差 | 7 (film's own panel, not used now) | — | no |
| unit_mm | mm | mm | 6, 7 | small print | no |
| part_header | header | 母管 | 7 | small print, under each plate | no |
| part_branch | branch | 枝管 | 7 | small print, under each plate | no |
| positioning | AI-native engineering platform powered by AMOEBA | AMOEBAを核とする、AIネイティブの設計基盤 | 11 | secondary, centred | no (the owner's positioning line) |
| planned_ai | AI drawing interpretation: planned | AIによる図面解釈：計画中 | 11 | secondary, centred | yes in substance ("Reading drawings is planned"), words differ |
| today | Running today | 稼働中 | 11 | small print, centred | yes in substance ("running today") |
| today_items | Calculation · Checks · Record | 計算 · 検証 · 記録 | 11 | small print, centred | yes in substance ("the calculation, the checks, the record") |
| brand | Engineering, Reimagined. | 設計を、再定義。 | 12 | statement, centred | yes in substance (L9 "Engineering, reimagined.") |
| cta | Request a demonstration | デモのご依頼 | 12 | button text, centred | no |
| disclaimer | Product concept. Capabilities in development. Not validated for manufacturing. | 製品コンセプトです。機能は開発中であり、製造用途での検証は未実施です。 | 12 | small print, centred at the foot | no (the owner's wording, exact) |
| bytes | bytes | バイト | 10 (DXF file summary line) | small print | no |
| credit | (none) | 音声：VOICEVOX Nemo | 12 (Japanese only) | small print, bottom right | no (required credit, see audio/VOICES.md) |
| wordmark | NEXUS FABRIC | NEXUS FABRIC | 11–12 | lockup, not translated | the product name |

Unused keys in `narration.json` (kept, not drawn): `wrong`, `eq_point`, `dxf_reread`, `ui_note`.

Film-drawn symbols and labels that are not words: `s(φ)`, `R`, `r`, `β`, `φ`, `s`, `×`, the degree marks on the angle axis.
The DXF stream (shot 10) is the file's own text. Its annotation "NOT RELEASED" (and the file name) is drawn in amber because
the product's file carries it; "UNITS: MILLIMETRES" is the file's own text. Amber is used for nothing else in the film.
