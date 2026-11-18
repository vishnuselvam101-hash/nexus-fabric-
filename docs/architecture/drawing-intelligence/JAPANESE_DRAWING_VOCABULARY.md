# Japanese Drawing Vocabulary (seed list)

Version 0.1 · 18 November 2026 · Supporting note to [Spec 03 §3.4](../../specifications/03_DRAWING_INTELLIGENCE_AND_INGESTION_SPECIFICATION.md)

> **Unreviewed.** Compiled from the author's general knowledge of Japanese technical vocabulary, not from the factory's drawings or from standards. It must be reviewed by a native-speaking fabrication engineer before use; shop-floor terms vary by company and region. Entries marked (?) are less certain. The list seeds the semantic-token mapping; it is not a recognition dictionary for auto-correction (DI-32).

## Views and drawing structure

| Term | Meaning | Semantic token |
|---|---|---|
| 正面図 | Front view | VIEW_FRONT |
| 平面図 | Plan / top view | VIEW_TOP |
| 側面図 / 右側面図 / 左側面図 | Side / right / left side view | VIEW_SIDE / RIGHT / LEFT |
| 下面図 | Bottom view | VIEW_BOTTOM |
| 背面図 | Rear view | VIEW_REAR |
| 断面図, 断面 A-A | Section view | VIEW_SECTION |
| 詳細図, A部詳細 | Detail view, detail of A | VIEW_DETAIL |
| 矢視, A矢視 | View in direction of arrow A | VIEW_ARROW |
| 展開図 | Development (flat pattern) | VIEW_DEVELOPMENT |
| 組立図 / 部品図 | Assembly drawing / part drawing | DOC_ASSEMBLY / DOC_PART |
| 参考図 | Reference drawing | DOC_REFERENCE |
| 第三角法 / 第一角法 | Third-angle / first-angle projection | PROJECTION_3 / PROJECTION_1 |
| 尺度 | Scale | TITLE_SCALE |
| 図番 / 図面番号 | Drawing number | TITLE_DRAWING_NO |
| 品名 / 名称 | Part name / title | TITLE_NAME |
| 数量 / 個数 | Quantity | TITLE_QTY |
| 材質 / 材料 | Material | TITLE_MATERIAL |
| 改訂 / 訂正 / 変更 | Revision / correction / change | REVISION |
| 単位 | Unit | UNIT |
| 設計 / 製図 / 検図 / 承認 | Designed / drawn / checked / approved | TITLE_ROLE |

## Dimensions and qualifiers

| Term | Meaning | Semantic token |
|---|---|---|
| 寸法 | Dimension | — |
| 内径 / 外径 | Inside / outside diameter | SURFACE_INSIDE / SURFACE_OUTSIDE |
| 内寸 / 外寸 | Inside / outside dimension | SURFACE_INSIDE / SURFACE_OUTSIDE |
| 芯々 (芯芯), 中心間 | Centre to centre | CENTRE_TO_CENTRE |
| 板厚 / 肉厚 | Plate thickness / wall thickness | THICKNESS |
| 参考 / 参考寸法 | Reference (dimension) | FLAG_REFERENCE |
| 約 | Approximately | FLAG_APPROX |
| 公差 / 普通公差 | Tolerance / general tolerance | TOLERANCE / GENERAL_TOLERANCE |
| 指示なき… | "Where not specified…" (default rule in notes) | DEFAULT_RULE |
| 面取り | Chamfer | CHAMFER |
| 等分 / ピッチ | Equally divided / pitch | PATTERN |
| キリ / 通し | Drilled hole / through | HOLE_DRILL / THROUGH |
| 現合 / 現物合わせ | To be fitted at assembly / matched to the actual part | FLAG_FIT_ON_SITE — a dimension so marked is deliberately not fixed by the drawing |
| 勝手違い / 勝手反対 | Opposite hand (mirror-image counterpart) | MIRRORED_COUNTERPART — high-risk: implies a mirrored part |
| 対称 | Symmetric | SYMMETRIC |
| 各 n | n of each | QTY_EACH |
| 同上 / 以下同様 | Same as above / likewise below | REPEAT_REFERENCE |

## Fabrication and welding

| Term | Meaning | Semantic token |
|---|---|---|
| 溶接 | Welding | WELD |
| 全周溶接 | All-around weld | WELD_ALL_AROUND |
| 現場溶接 | Field (site) weld | WELD_FIELD |
| すみ肉 | Fillet | WELD_FILLET |
| 突合せ | Butt | WELD_BUTT |
| 開先 | Groove / bevel preparation | WELD_GROOVE |
| 両側 / 片側 | Both sides / one side | SIDE_BOTH / SIDE_ONE |
| 曲げ / 内R | Bend / inside radius | BEND / RADIUS_INSIDE |
| 継手 / 継目 | Joint / seam | JOINT / SEAM |
| 鋼管 / 鋼板 | Steel pipe / steel plate | PIPE / PLATE |
| レジューサ | Reducer | REDUCER |
| 同心 / 偏心 | Concentric / eccentric | CONCENTRIC / ECCENTRIC |
| 枝管 / 母管 (主管) | Branch pipe / header (main) pipe | BRANCH / HEADER |
| 角丸 (?) | Square-to-round transition (shop term) | SQUARE_TO_ROUND |
| ホッパー / シュート / ダクト / フランジ | Hopper / chute / duct / flange | — |
| 表面処理 / 塗装 / 亜鉛めっき | Surface treatment / painting / galvanising | FINISH |
| 板取り | Plate layout / nesting | NESTING |

## Notation patterns (to be confirmed on real drawings)

`φ216.3×t8.2` (diameter × thickness) · `t9`, `t=9`, `PL-9` (thickness / plate) · `4-φ18` or `4×φ18` (count – feature) · `R50`, `SR50`, `C5`, `□100` · `(1200)` reference · `100A`, `4B` nominal pipe sizes (designations requiring a standard table) · `L=1500` · material grade strings (treated as opaque designations until a verified material table exists).
