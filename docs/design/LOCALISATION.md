# Localisation: English and Japanese

Round 3. The client has two languages: English (the reference) and Japanese. Every string the client owns is in
`web/i18n.js`; the server and the engine send English, which the client translates only where a template matches exactly.

> The Japanese terminology has not been reviewed by a native-speaking fabrication engineer.

## How the catalogue works

- `web/i18n.js` holds two dictionaries, `EN` and `JA`, with the same keys (1013 on 22 Nov 2026; `i18n_check.mjs` prints the count). A key missing in Japanese falls back to
  English and is recorded by `missing()`; the test fails if anything is recorded.
- `t(key, params)` returns plain text, for `textContent` and attributes. `h(key, params)` returns HTML: the text is
  escaped, `**bold**` becomes `<b>` and a backquoted span becomes monospace. Values from users or the server are escaped
  by the caller: user text with `ud()`, identifiers with `mono()`.
- Placeholders are `{name}`. A plural is `[singular, plural]`. The placeholders of each key are the same in both languages.
- A message is a whole sentence with placeholders; no sentence is built from fragments, so Japanese word order is free.
- Engine and server text arrives in English. It is translated only by exact match against four template tables:
  `SENT` (84 sentences), `WHOLE` (66 texts), `REASON` (15) and `SERVER` (54), plus 224 identifier labels in 21 kinds.
  If any sentence matches no template, the whole text is shown in English and tagged "engine output" or "server
  message". Numbers and identifiers in a matched template pass through unchanged. In a Japanese template, an identifier
  written in backquotes is shown in monospace.
- The language is chosen in this order: `?lang=ja` or `?lang=en` for one page load; the saved choice (`localStorage`,
  key `nf-lang`); the browser's first language if it is Japanese; English. The switch in the top bar, and on the sign-in
  screen, redraws the current screen from data already loaded: no request, no reload, typed input kept.

## Adding a string

1. Add the key to `EN` and to `JA` in `web/i18n.js`, with the same placeholders.
2. Render it with `t()` for text or `h()` for HTML. Do not join fragments by hand.
3. In Japanese, write identifiers, file names and hashes in backquotes, so that they show in monospace.
4. Run `node docs/design/tools/i18n_check.mjs`. It must report no missing key, no placeholder mismatch, and no bare
   English identifier in Japanese.
5. Rebuild with `cargo build --release -p nf-server` (the files are embedded in the binary), then run
   `tests/ui_smoke.js`. Its Japanese walk fails on any English word of three or more letters on the screens it visits.

## What is deliberately not translated

| Kept as it is | Why |
|---|---|
| NEXUS FABRIC; AMOEBA (the module's name) | product and module names |
| DXF, PDF, PNG, JPEG, TIFF, STEP, SHA-256; the units mm, deg, rad, MiB | file formats, the hash algorithm and units are the same in every language |
| Identifiers: parameter keys (`header_radius`), curve names (`seam_a`), gate IDs (`G01` to `G20`), check and record names, layer names (`CUT_OUTER`), the engine id, closure hashes | the engine, the DXF files and the stored records use these exact strings; a translation would break the link to them. Shown in monospace |
| File names and DXF content: `NOT-RELEASED`, `NF_NOT_RELEASED` | the release marking stays English, in the file name and in the file |
| User-entered text: project, user and file names, reasons | shown as typed, in an isolated element (`ud()`) |
| Engine and server sentences that match no template | shown in English and tagged; never translated in part |

Where an identifier has a Japanese label, the identifier stays beside the label, so that a reader can match the screen to
the engine output.

## How the tests guard it

- `tests/ui_smoke.js`, section 14b:
  - the switch: no reload, `<html lang>` follows, the choice survives a reload, `?lang=ja` and `?lang=en` each open in
    their language, and the empty route `#/` opens the dashboard;
  - a walk of the visible text on 32 Japanese states: sign-in, dashboard, job list, the five new-calculation forms, a
    computed job with each of its six tabs and a corrected job's changes, a record opened, the correction dialog, the
    keyboard help, projects, a project, the AMOEBA tabs, manufacturing, verification, administration (users, security,
    audit) and account. The walk skips identifiers, hashes, user text and `[lang=en]`, and accepts only these words:
    NEXUS, FABRIC, AMOEBA, DXF, PDF, PNG, JPEG, TIFF, STEP, SHA, rad;
  - after the walk, `missing()` is empty; in Japanese there is no horizontal overflow on the dashboard, the new-calculation
    form or the job view at 1366, 1024 and 768 pixels; and there is no console or page error in either language.
- `docs/design/tools/i18n_check.mjs` checks key parity, placeholder parity and bare English identifiers, without a browser.

## Identifiers: every one has a Japanese label (work plan 3.3, 22 Nov 2026)

Every gate G00 to G22, every check name, every refusal code and every record kind has a Japanese label or message. This
is checked from the build itself, not from a hand-written list:

- `crates/nf-server/tests/labels.rs` (run by `cargo test`): every code of `nf_core::refusal::ALL` has exactly one English
  and one Japanese message; the gate table `ID.gate` lists G00 to G22 with an English name, a Japanese label and a help
  text in both languages; the gates the sources evaluate are exactly those not in `GATES_NOT_EVALUATED` (G03, G04, G07,
  G09, G12, G13 from Specification 02 and G17, proposed for AMOEBA); and every check name, record kind, record family,
  relation and gate of the jobs the engine computes for all five families (each declared surface, and the set-on joint)
  has a Japanese label.
- `tests/ui_smoke.js`, the Japanese walk: besides English words, every screen it visits is probed for an identifier
  shown without a label. A visible monospace element whose text is shaped like an identifier (lower-case words joined
  by `_` or `.`, or a gate id) must have a Japanese label in the identifier table `ID`, unless it is one of the
  identifiers below. The walk now also visits the concentric reducer's checks and the help page.

The audit found three identifiers shown without a label, now labelled: the cone checks `large_end_3d_arc_length` and
`small_end_3d_arc_length`, and the record kind `verify.dxf_quality`; and the counters and skip reasons of the PDF
converter's report (`ID.pdfcount`, `ID.pdfskip`), shown in Japanese as label and identifier.

### Identifiers shown without a Japanese label (the documented list)

| Identifier | Where | Why it has no label |
|---|---|---|
| `eps_num`, `eps_rep` | quantities and assumptions of a pattern | deliberate technical names of the two error bounds, shown identically in both languages (also accepted by the word walk) |
| the 16 warning codes of the PDF converter (`annotations_not_converted`, `clipping_not_applied`, `cropbox_outside_mediabox`, `ext_gstate_missing`, `font_missing`, `invalid_dash`, `optional_content_ignored`, `pattern_fill_outline_only`, `rotate_invalid`, `shading_not_converted`, `text_chars_replaced`, `unbalanced_restore`, `unknown_operator`, `userunit_invalid`, `xobject_ignored`, `xobject_missing`) | conversion report | each is shown with the converter's own message beside it |
| a gate's machine name (`provenance_complete` ...) | the expanded row of a gate | shown under the gate's Japanese label, after 名称; the probe skips that line (`li[data-gate] .g-meta`) |
| `separate_process_replay` | gate G19 after a replay audit | the kind of replay audit, quoted as an identifier inside the server's sentence, which is translated by template |
| the refusal codes (`field_missing` ...) | the help page, "What each refusal code means" | each is in the row of its Japanese message; the probe skips that column (`#help-refusals td:first-child`) |

## Length rules count characters (work plan 3.2)

Every minimum and maximum length of a text a person types counts characters: Unicode scalar values after trimming the
characters Rust's `str::trim` trims, exactly as the server counts them. A Japanese character, and a character outside
the BMP, counts as one (`web/app.js`: `charCount`, `trimRs`; JavaScript's `.length` counts UTF-16 units and
`String.trim` trims a different set). Reasons need 5 to 2000 characters, project names 1 to 200, descriptions at most
2000, passwords at least 10 (their upper limit, 1024 bytes, is stated in bytes by the server and stays so). The
message `refusal.reason_required` said "five bytes" and was corrected in both languages.

## New Japanese strings — native review needed

Added or changed on 22 Nov 2026 (work plan 3.2, 3.3, 3.5) and 23 Nov 2026 (work plan 1.5). None has been reviewed by a
native-speaking fabrication engineer.

Changed refusal messages (their English said something the engine no longer measures, or "bytes"):

| Key | Japanese |
|---|---|
| `refusal.reducer_near_cylindrical` | 半径がほぼ等しいため、円錐の頂点までの距離が{measured} mmとなり、このビルドで扱った最大寸法{limit} mmを超えます。これは実装上の上限であり、工学上の規則ではありません。 |
| `refusal.developed_angle_overlaps` | 小径側の継目の隙間が{measured} mmとなり、輪郭検査の閉合分解能{limit} mm以下です。2本の継目の辺を区別できません。これは実装上の限界であり、工学上の規則ではありません。 |
| `refusal.reason_required` | 訂正には5文字以上の理由が必要です。日本語の文字も含め、1文字を1文字として数えます。前後の空白は数えません。 |

New identifier labels (`ID` in `web/i18n.js`):

| Kind | Identifier | Japanese |
|---|---|---|
| gate | G03 | 拘束条件の確定 |
| gate | G04 | 図面記載値と実測値の一致 |
| gate | G07 | 近似による歪みの受入れ |
| gate | G09 | 誤差予算 |
| gate | G12 | 加工上の制約 |
| gate | G13 | 再現性 |
| gate | G17 | 網羅性（提案のみ） |
| check | `large_end_3d_arc_length` | 大径側の端の3D円弧長 |
| check | `small_end_3d_arc_length` | 小径側の端の3D円弧長 |
| check | `seam_a_on_positive_x` | 継目の辺Aが＋X側にあること |
| check | `seam_b_at_development_angle` | 継目の辺Bが展開角の位置にあること |
| check | `end_circle_3d_arc_length` | 端の円の3D円弧長 |
| check | `end_circles_3d_arc_length_equal` | 両端の円の3D円弧長が同じ長さ |
| check | `area_equals_sampled_surface_area` | 展開図の面積＝サンプリングした表面積 |
| check | `bottom_runs_along_x` | 底辺がX軸に沿っていること |
| check | `seam_a_runs_along_y` | 継目の辺AがY軸に沿っていること |
| record kind | `verify.dxf_quality` | DXF品質ゲートの検査 |
| PDF counter | `annotations` | 注釈 |
| PDF counter | `clip_operators` | クリップ演算子 |
| PDF counter | `dash_pieces` | 破線の線分 |
| PDF counter | `dashed_subpaths` | 破線のサブパス |
| PDF counter | `degenerate_subpaths` | 退化したサブパス |
| PDF counter | `fill_subpaths` | 塗りのサブパス |
| PDF counter | `forms` | フォーム外部オブジェクト |
| PDF counter | `images` | 画像 |
| PDF counter | `path_segments` | パスの線分 |
| PDF counter | `pattern_fills` | パターン塗り |
| PDF counter | `shadings` | シェーディング |
| PDF counter | `stroke_subpaths` | 線のサブパス |
| PDF counter | `text_chars_drawing_symbol` | 図面記号として読んだ文字 |
| PDF counter | `text_chars_normalised` | 正規化した文字 |
| PDF counter | `text_chars_replaced` | 置き換えた文字 |
| PDF counter | `text_items` | 文字列の項目 |
| PDF counter | `text_items_skipped` | 出力しなかった文字列の項目 |
| PDF counter | `text_written` | 出力した文字列 |
| PDF counter | `unknown_operators` | 不明な演算子 |
| PDF skip reason | `degenerate_size` | 大きさが0に近い文字 |
| PDF skip reason | `no_font` | フォントがない |
| PDF skip reason | `undecodable_encoding` | 解読できない文字符号化 |

New catalogue keys (help page, sample projects, the refusal note of the form, the length message):

| Key | Japanese |
|---|---|
| `nav.help` | ヘルプ |
| `hp.title` | ヘルプ |
| `hp.sub` | このページの内容はすべてプログラムに含まれており、ネットワークは使用しません。 |
| `hp.toc` | このページの内容 |
| `hp.build.h` | このビルドについて |
| `hp.build.p` | **開発版です。** 検証されていません。展開図のある結果は、最良でも**要レビュー**のままです。技術者承認（ゲート`G16`）は実装されていないため、何もリリースできません。すべてのDXFファイルには**未リリース**と表示されます。いかなる出力も製造に使用してはなりません。 |
| `hp.build.ev` | すべてのアルゴリズムは証拠レベルEV-1（AIが生成、独立したレビューなし）です。製作技術者による出力と前提条件のレビューは行われていません。 |
| `hp.fam.h` | 形状種別ごとに必要な入力 |
| `hp.fam.p` | 長さと半径の単位はmm、角度の単位は度です。例の値は、新規計算フォームに最初に表示される値です。推奨値ではありません。 |
| `hp.fam.cylinder_shell` | 直円筒に巻く板1枚：展開する半径と、長さ。 |
| `hp.fam.cone_frustum` | 同心レジューサ（直円錐台）に巻く板1枚：大径側半径`r1`、小径側半径`r2`（`r1`の方が大きいこと）、軸方向高さ。 |
| `hp.fam.eccentric_reducer` | 偏心レジューサ（斜円錐台）の板1枚：同心レジューサの入力に加え、小径側の偏心量`e`（0以上）。 |
| `hp.fam.header_with_branch` | 2部材と取合い1箇所：開口付きの母管胴と、枝管。取合い半径（管が交わる面）、展開半径（各板を展開する面）、母管の長さ、枝管の位置、両軸のなす角、軸オフセット、枝管の直切り端。半径の基準面を宣言します（不明と宣言することもできます）。セットオンの取合いを宣言することもできます。 |
| `hp.fam.branch_stub` | 円筒母管に合わせて切断する枝管の板1枚：母管の取合い半径、枝管の取合い半径と展開半径、両軸のなす角、軸オフセット、直切り端。半径の基準面を宣言します（選ばない場合は不明）。 |
| `hp.th.input` | 入力項目 |
| `hp.th.key` | キー |
| `hp.th.unit` | 単位 |
| `hp.th.example` | 例の値 |
| `hp.optional` | 省略可 |
| `hp.gate.h` | 各ゲートの意味 |
| `hp.gate.p` | ゲートは、リリースの条件の1つです。各計算は評価したゲートを、その結果と、エンジンまたはサーバーが示す理由とともに一覧表示します。下の名称は、計算画面に表示されるラベルです。 |
| `hp.th.gate` | ゲート |
| `hp.th.name` | 名称 |
| `hp.th.means` | 意味 |
| `hp.gate.G00` | 結果が現行のものであること。訂正によって差し替えられた計算では不合格になります。差し替え済みの結果は使用してはなりません。 |
| `hp.gate.G01` | すべてのパラメータが、確認済みの出典にたどれること。パラメータは手入力で図面と結び付いていないため、このゲートは未確定のままです。 |
| `hp.gate.G02` | けがき面や半径の基準面など、未解決の曖昧さがないこと。挙げられた事項は技術者が確定する必要があります。 |
| `hp.gate.G03` | 寸法によって、形状が矛盾なく一意に定まること。 |
| `hp.gate.G04` | 図面に記載された寸法と、測定した寸法の間に未解決の差がないこと。 |
| `hp.gate.G05` | 各部材に展開クラス（厳密・閉形式、または厳密・数値評価）が割り当てられていること。 |
| `hp.gate.G06` | すべての前提条件が、校正された加工データで裏付けられていること。このビルドにはそのデータがないため、このゲートは未確定のままです。 |
| `hp.gate.G07` | 近似による展開の歪みが許容範囲内にあり、記録された判断によって受け入れられていること。 |
| `hp.gate.G08` | 数値誤差（`eps_num`）と曲線表現誤差（`eps_rep`）が示され、割当ての範囲内であること。 |
| `hp.gate.G09` | 誤差の合計が、誤差予算の範囲内にあること。 |
| `hp.gate.G10` | 形状種別ごとに必要な独立した検査が、すべて許容値の範囲で展開図と一致し、不合格がないこと。 |
| `hp.gate.G11` | すべての輪郭が閉じた単純な形で正しい向きであり、すべての開口が板の内側にあること。 |
| `hp.gate.G12` | 材料・機械・継目の規則を満たすこと。 |
| `hp.gate.G13` | 結果を再生成すると、同じ結果が得られること。 |
| `hp.gate.G14` | アルゴリズムが使用に適格と認められていること。このビルドのアルゴリズムはすべて証拠レベルEV-1（AIが生成、独立したレビューなし）のため、このゲートは未確定のままです。 |
| `hp.gate.G15` | すべてのDXFファイルを別の読込み処理で読み戻し、意図した形状と照合し、DXF品質ゲートで検査すること。不合格のファイルは提供しません。 |
| `hp.gate.G16` | 資格のある技術者が結果を承認すること。未実装のため、どの結果もリリースできず、すべてのDXFファイルに未リリースと表示されます。 |
| `hp.gate.G17` | 対象範囲内の原図の要素が、すべて調べられていること。 |
| `hp.gate.G18` | 訂正によって生じた要精査項目が、根拠または技術者の記録された判断によって、すべて解消されていること。 |
| `hp.gate.G19` | 再実行監査で、保存済みの結果が再現されること。再実行監査を実行するまでは未実行です。 |
| `hp.gate.G20` | この計算が、このプロジェクトで教訓として記録された誤りを繰り返していないこと。一致した場合は、技術者が理由を付けてそれぞれ確認していること。 |
| `hp.gate.G21` | 結果が、現在のエンジンのビルドで計算されていること。ほかのビルドの結果は再計算が必要です。 |
| `hp.gate.G22` | 保存済みの結果が整合性の検査に合格すること。合格しない場合、その計算は何も使用してはなりません。 |
| `hp.gate.na` | 仕様書02で定義されていますが、このビルドでは評価しません。 |
| `hp.gate.proposed` | AMOEBAのアーキテクチャで提案されたもので、仕様書02には含まれず、このビルドでは評価しません。 |
| `hp.ref.h` | 各拒否コードの意味 |
| `hp.ref.p` | 拒否された依頼には、これらのコードの1つと、対象の項目名が返されます。拒否された計算はそれを保持し、「この値で新規計算」ではその項目に同じ文で印を付けます。拒否された依頼では何も計算しません。 |
| `hp.th.code` | コード |
| `hp.th.message` | 意味 |
| `hp.ph.fields` | 該当する項目 |
| `hp.ph.limit` | 限界値 |
| `hp.ph.measured` | 測定値 |
| `hp.nm.h` | モデル化していないもの |
| `hp.nm.p` | 展開図は入力した面の厳密な展開であり、それ以上のものではありません。次のものはモデル化していません。 |
| `hp.nm.1` | 板厚：半径はすべて入力したとおりに、宣言した面（外面・内面・中立面・不明）の半径として使用します。 |
| `hp.nm.2` | 曲げ代と、ロール曲げの代。 |
| `hp.nm.3` | 溶接の隙間（ルートギャップ）。 |
| `hp.nm.4` | 切断幅（カーフ）。 |
| `hp.nm.5` | 規格や工場の技術上の規則と加工値。拒否コードに示す限界は、このビルドの実装上の限界であり、技術上の規則ではありません。 |
| `hp.smp.h` | サンプルプロジェクト |
| `hp.smp.p` | 各サンプルはプロジェクトを作成し、新規計算フォームの例の値を使って、通常のエンジンで計算を行います。ほかの計算と同じく記録・監査され、サンプルの計算はすべて**要レビュー**で終わります。サンプルプロジェクトがすでにある場合は、作成せずに開きます。 |
| `hp.smp.viewer` | 閲覧者はプロジェクトを作成できません。技術者または管理者に、サンプルプロジェクトの作成を依頼してください。 |
| `hp.smp.jobs` | 計算：{list} |
| `smp.1.name` | サンプル1：円筒胴 |
| `smp.2.name` | サンプル2：同心レジューサと偏心レジューサ |
| `smp.3.name` | サンプル3：母管＋枝管と枝管スタブ |
| `smp.desc` | 新規計算フォームの例の値で作成したサンプルプロジェクトで、実在の部材ではありません。開発版：すべての結果は要レビューのままです。 |
| `smp.btn.aria` | {name}を開く、または作成する |
| `smp.busy` | 計算中… |
| `smp.done` | サンプルプロジェクト#{id}を作成し、{n}件の計算を行いました。 |
| `smp.go` | サンプルプロジェクトを開く |
| `math.refused.note` | 計算{id}は入力拒否されました：{why} 対象の項目に印を付けています。 |
| `refusal.unmapped` | このフォームには{fields}の項目がありません。 |
| `v.text.long` | 長すぎます（{max}文字以内。現在{n}文字）。 |

## Glossary, with confidence

| English | Japanese | Confidence |
|---|---|---|
| flat pattern | 展開図 | a trade term |
| development (of a formed surface) | 板展開 | a trade term |
| marking side | けがき面 | a trade term |
| header, branch | 母管, 枝管 | a trade term |
| seam | 継目 | our choice: check |
| correction | 訂正 | our choice: check |
| scrutiny obligation | 要精査項目 | our choice: check |
| gate | ゲート | our choice: check |
| review required | 要レビュー | our choice: check |
| not released | 未リリース | our choice: check |
| refused (input) | 入力拒否 | our choice: check |
| inconclusive | 未確定 | our choice: check |

## Files

- `web/i18n.js`: the catalogue, the template tables and the identifier labels.
- `web/app.js`: the views. It uses `t`, `h`, `eng`, `srv`, `named`, `mono`, `ud` and `jaHtml` (backquoted identifiers in
  Japanese text).
- `docs/design/tools/i18n_check.mjs` and `tests/ui_smoke.js` (section 14b): the checks above.
