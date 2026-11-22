// On-screen sentences of film 4. Source of NEW_STRINGS.md (tools/build_strings.mjs).
// Three type sizes only: headline 132 px, body 48 px, label 40 px. Product words (state, results, parameter and check
// names, the DXF name) come from data/film4-data.js (the application's catalogue and the product output), not from here.
// Origin of each line: F3 = flagship3/script/onscreen.js (reused unchanged), BRIEF = wording given in the film 4 brief,
// NEW = new in film 4 (Japanese needs native review).
'use strict';
window.NF_ONSCREEN4 = {
  // BRIEF (EN). JA assembled from F3 label_vis (first half) and F3 label_sim (second half).
  label_vis: { en: 'Visualisation made for this film · data from the product', ja: '本映像用のビジュアル・データは製品の出力です' },
  label_actual: { en: 'Actual software · development build', ja: '実際のソフトウェア・開発版' },                  // F3
  hook: { en: ['One wrong curve.'], ja: ['曲線を、ひとつ誤れば。'] },                                           // F3
  question: { en: ['Before you cut —', 'how do you know?'], ja: ['切る前に、', 'どう確かめる？'] },                // F3
  brand_name: { en: ['NEXUS FABRIC'], ja: ['NEXUS FABRIC'] },                                                    // F3
  product_a: { en: ['Dimensions in.'], ja: ['寸法から、'] },                                                     // F3
  product_b: { en: ['Flat pattern out.'], ja: ['板展開へ。'] },                                                  // F3
  // BRIEF (EN, one sentence each); JA F3
  limits: { en: ['Zero-thickness reference surfaces.', 'Thickness, bend allowance, weld gap and kerf are not modelled.'], ja: ['板厚ゼロの基準面です。', '未考慮：板厚、曲げ代、溶接ギャップ、カーフ'] },
  part_shell: { en: ['Cylindrical shell'], ja: ['円筒胴'] },                                                     // F3
  part_cone: { en: ['Concentric reducer'], ja: ['同心レジューサ'] },                                              // F3
  part_ecc: { en: ['Eccentric reducer'], ja: ['偏心レジューサ'] },                                                // F3
  part_branch: { en: ['Branch stub'], ja: ['枝管スタブ'] },                                                       // F3
  part_header: { en: ['Header shell', 'with opening'], ja: ['開口付き母管胴'] },                                  // F3 (EN set on two lines)
  points_line: { en: '{n} computed points on the opening', ja: '開口の計算点 {n}点' },                           // NEW
  checks_big: { en: ['It checks its own work.'], ja: ['自ら、検算。'] },                                          // F3
  one_curve: { en: ['Both edges.', 'One curve.'], ja: ['二つの縁が、', '一本の曲線に。'] },                       // NEW
  checks_count: { en: '{n} of {b} checks run', ja: '{n} / {b} 件の検算' },                                         // F3
  checks_sub: { en: 'Internal consistency checks — not an approval.', ja: '内部の整合チェックです。承認ではありません。' },   // BRIEF / F3
  dxf_words: { en: [['Written.'], ['Read back.'], ['Compared.']], ja: [['出力'], ['再読込'], ['照合']] },          // F3 dxf_big, one word at a time
  reimport_line: { en: '{r} of {w} entities read back · largest deviation {d} mm', ja: '{w}件中{r}件を再読込・最大偏差 {d} mm' },   // NEW (numbers: the product's re-import report)
  dxf_layer_note: { en: 'DXF entities as written', ja: 'DXF出力' },                                              // NEW (JA = catalogue rkind dxf_output)
  amoeba_big: { en: ['AMOEBA'], ja: ['AMOEBA'] },                                                                // F3
  amoeba_def: { en: ['Every value and every step,', 'recorded and linked.'], ja: ['すべての値と手順を記録し、', 'つなぎます。'] },   // F3
  change_big: { en: ['Change one number.'], ja: ['数値を、ひとつ変える。'] },                                      // F3
  obl_line: { en: '{r} raised · {d} discharged · {o} open', ja: '{r}件発生 · {d}件解消 · {o}件未解決' },            // F3
  // NEW: the obligations in plain words; counts from the data, wording from the obligations' notes (catalogue JA:
  // 解消済み, 技術者の判断が必要)
  obl_flagged: { en: 'flagged for review', ja: '件に要精査の印' },
  obl_cleared: { en: 'cleared by recomputed checks', ja: '件は再計算の検証で解消済み' },
  obl_open: { en: "require an engineer's decision", ja: '件は技術者の判断が必要' },
  replay_big: { en: ['Replay: match.'], ja: ['再実行：一致'] },                                                    // F3
  rec_count: { en: '{n} records checked, {f} failing.', ja: '{n}件の記録を照合、不一致{f}件。' },                   // F3 (failing count from the data)
  honesty_big: { en: ["What it can't confirm,", 'it shows.'], ja: ['確認できないことは、', '隠さない。'] },          // F3
  res_pass: { en: 'PASS', ja: '内部チェック通過' },                                                               // F3
  gate_approval: { en: 'Human approval (not implemented)', ja: '人による承認（未実装）' },                         // F3
  dev_line: { en: 'Development build. Not validated for manufacturing.', ja: '開発版です。製造用途での検証は未実施です。' },   // F3 / BRIEF
  lang_big: { en: ['English · 日本語'], ja: ['English · 日本語'] },                                              // F3
  offline: { en: ['Runs fully offline.'], ja: ['完全オフラインで動作'] },                                          // F3
  brand_line: { en: 'Engineering, Reimagined.', ja: '設計を、再定義。' },                                          // BRIEF
  cta: { en: 'Request a demonstration', ja: 'デモのご依頼' },                                                      // BRIEF
  positioning: { en: 'Engineering platform powered by AMOEBA.', ja: 'AMOEBAを核とする設計基盤' },                  // BRIEF
  disclaimer: { en: 'Product concept. Capabilities in development. Not validated for manufacturing.', ja: '製品コンセプトです。機能は開発中であり、製造用途での検証は未実施です。' },   // BRIEF / F3
  contact: { en: ['sekishion@chuo-sogo.net', 'vishnu.s@chuo-sogo.net'], ja: ['sekishion@chuo-sogo.net', 'vishnu.s@chuo-sogo.net'] },   // BRIEF (exact)
  credit: { en: '', ja: '音声：VOICEVOX Nemo' },                                                                  // BRIEF (licence condition, JA film)
};
