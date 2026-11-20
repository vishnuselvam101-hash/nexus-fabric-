# On-screen text that is not in the narration — flagship film 3

Source of truth: `script/onscreen.js` (this file is written from it by `tools/build_strings.js`). The narration lines are in
`script/SCRIPT.md` and the voice files; the words that appear inside product screens are the application's own and are not listed.
Numbers, names, verdicts, DXF lines, graph labels and gate results are read from the product's output at build time and are not listed.

The Japanese wording has **not been reviewed by a native-speaking engineer**. Product words (state, gate results, quantity
names, tab names) are the application's own strings from `web/i18n.js`.

| Key | English | 日本語 |
|---|---|---|
| label_vis | Visualisation made for this film · figures from the product | 本映像用のビジュアル・数値は製品の出力です |
| label_sim | Interface simplified for this film · data from the product | 本映像用に画面を簡略化しています・データは製品の出力です |
| label_actual | Actual software · development build | 実際のソフトウェア・開発版 |
| hook | One wrong curve. | 曲線を、ひとつ誤れば。 |
| question | Before you cut — / how do you know? | 切る前に、 / どう確かめる？ |
| product_a | Dimensions in. | 寸法から、 |
| product_b | Flat pattern out. | 板展開へ。 |
| limits | Zero-thickness reference surfaces. / Thickness, bend allowance, weld gap and kerf are not modelled. | 板厚ゼロの基準面です。 / 未考慮：板厚、曲げ代、溶接ギャップ、カーフ |
| part_shell | Cylindrical shell | 円筒胴 |
| part_cone | Concentric reducer | 同心レジューサ |
| part_ecc | Eccentric reducer | 偏心レジューサ |
| part_branch | Branch stub | 枝管スタブ |
| part_header | Header shell with opening | 開口付き母管胴 |
| unit_mm | mm | mm |
| checks_big | It checks its own work. | 自ら、検算。 |
| checks_sub | Internal consistency checks — / not an approval. | 内部の整合チェックです。 / 承認ではありません。 |
| checks_count | {n} of {b} checks run | {n} / {b} 件の検算 |
| dxf_big | Written. Read back. / Compared. | 出力・再読込・照合 |
| amoeba_big | AMOEBA | AMOEBA |
| amoeba_def | Every value and every step, / recorded and linked. | すべての値と手順を記録し、 / つなぎます。 |
| change_big | Change one number. | 数値を、ひとつ変える。 |
| corr_line | Correction recorded: {o} → {n} mm | 訂正を記録：{o} → {n} mm |
| obl_line | {r} raised · {d} discharged · {o} open | {r}件発生 · {d}件解消 · {o}件未解決 |
| rule_flag | Flag rule: provisional, not yet reviewed. | 要精査の印：暫定規則、未レビュー |
| replay_big | Replay: match. | 再実行：一致 |
| rec_count | {n} records checked, 0 failing. | {n}件の記録を照合、不一致0件。 |
| honesty_big | What it can't confirm, / it shows. | 確認できないことは、 / 隠さない。 |
| res_pass | PASS | 内部チェック通過 |
| gate_approval | Human approval (not implemented) | 人による承認（未実装） |
| dev_line | Development build. Not validated for manufacturing. | 開発版です。製造用途での検証は未実施です。 |
| lang_big | English · 日本語 | English · 日本語 |
| offline | Runs fully offline. | 完全オフラインで動作 |
| brand_name | NEXUS FABRIC | NEXUS FABRIC |
| brand_line | Engineering, Reimagined. | 設計を、再定義。 |
| cta | Request a demonstration | デモのご依頼 |
| positioning | Engineering platform powered by AMOEBA. | AMOEBAを核とする設計基盤 |
| disclaimer | Product concept. Capabilities in development. Not validated for manufacturing. | 製品コンセプトです。機能は開発中であり、製造用途での検証は未実施です。 |
| contact | sekishion@chuo-sogo.net / vishnu.s@chuo-sogo.net | sekishion@chuo-sogo.net / vishnu.s@chuo-sogo.net |
| credit | — | 音声：VOICEVOX Nemo |

Product words the film shows on the product's own screens or restates from them (the application's own strings, web/i18n.js):
PASS, INCONCLUSIVE, NOT RUN, NOT IMPLEMENTED (gate and check results, with 合格 / 未確定 / 未実行 / 未実装); REVIEW REQUIRED (要レビュー);
open (未解決) and discharged (解消済み) (obligation states); Checks (検証項目); Records; Scrutiny; Changes; Graph; the quantity names
Developed width (展開幅), Length (長さ), Area (面積), Opening perimeter (開口の周長), Slant length (斜辺長); the Japanese check names
of the product (data/check-labels.js, 16 of the 26 check names; the other ten keep the machine name, as the product does in Japanese).
The sentence "{n} stored records checked, {f} failing." is the product's own (replay audit), in both languages.

Film-drawn symbols that are not words: the degree marks on the angle axis, the mark (the product's glyph, drawn from its geometry in
`lib/gfx.js`), the graph's typed relations (shown as the product returns them), the equation-free plots. The DXF stream
(scene 6) is the file's own text; its NOT-RELEASED file name and annotation are drawn in amber because the product's file carries them.
Amber is used only for REVIEW REQUIRED / NOT RELEASED and for open scrutiny obligations (the product's own state).

## Owner-supplied contact (revision 5, end card, 50-54 s)
Copied exactly from the owner's instruction, shown in both languages directly under the call to action at 40 px:
- sekishion@chuo-sogo.net
- vishnu.s@chuo-sogo.net
