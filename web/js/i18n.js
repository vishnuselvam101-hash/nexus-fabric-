// NEXUS FABRIC simplified UI — internationalisation. ES module, no dependencies, no external resources.
//
// Two kinds of text: (1) client-owned labels/messages with EN+JA in the catalog below,
// (2) engine/server prose translated via the template tables (SENT/WHOLE/REASON/SERVER).
// Japanese uses fabrication trade terms: 板展開, 取合い, けがき面, 母管, 枝管.

export const LANGS = ['en', 'ja'];
export const LANG_NAME = { en: 'English', ja: '日本語' };

let cur = 'en';
export const missing = new Set();

export const lang = () => cur;
export function setLang(l) {
  cur = LANGS.includes(l) ? l : 'en';
  document.documentElement.lang = cur;
  return cur;
}
export function initialLang(saved) {
  let q = null;
  try { q = new URLSearchParams(location.search).get('lang'); } catch { /* no URL API */ }
  if (LANGS.includes(q)) return q;
  if (LANGS.includes(saved)) return saved;
  const want = (navigator.languages && navigator.languages[0]) || navigator.language || '';
  return /^ja\b/i.test(want) ? 'ja' : 'en';
}

// ---- internal helpers ----
const escHtml = (v) => String(v ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const fill = (s, p) => (p ? s.replace(/\{(\w+)\}/g, (x, k) => (k in p ? p[k] : x)) : s);
function pick(key, p) {
  let m = MSG[cur][key];
  if (m === undefined) { missing.add(`${cur}:${key}`); m = MSG.en[key]; }
  if (m === undefined) return key;
  if (Array.isArray(m)) m = m[p && Number(p.n) === 1 ? 0 : 1];
  return m;
}

// ---- public text API ----
export const t = (key, p) => fill(pick(key, p), p);
export const h = (key, p) => fill(escHtml(pick(key, p)).replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`(.+?)`/g, '<span class="mono">$1</span>'), p);
export const has = (key) => MSG[cur][key] !== undefined || MSG.en[key] !== undefined;

/** Apply translations to all [data-i18n] elements under `root`. */
export function applyPage(root = document) {
  for (const el of root.querySelectorAll('[data-i18n]')) {
    el.textContent = t(el.dataset.i18n);
  }
}

// ================================================================ English
const EN = {
  // build status
  'build.short': 'Development build — not validated',
  'build.full': 'Development build — not validated — outputs must not be used for manufacturing',
  'build.status': 'DEVELOPMENT BUILD · NOT VALIDATED · NOT FOR MANUFACTURING',
  'build.login': '**Development build.** Not validated. No output may be used for manufacturing.',

  // states and verdicts
  'state.APPROVED': 'Approved', 'state.REJECTED': 'Rejected', 'state.SUPERSEDED': 'Superseded',
  'state.NO_RESULT': 'No result', 'state.REVIEW_REQUIRED': 'REVIEW REQUIRED', 'state.REFUSED': 'REFUSED',
  'result.PASS': 'PASS', 'result.FAIL': 'FAIL', 'result.INCONCLUSIVE': 'INCONCLUSIVE',
  'result.NOT_IMPLEMENTED': 'NOT IMPLEMENTED', 'result.NOT_RUN': 'NOT RUN',
  'obl.open': 'open', 'obl.discharged': 'discharged', 'obl.discharged_by_decision': 'discharged by decision', 'obl.waived': 'waived',
  'obl.open.n': ['{n} open scrutiny obligation', '{n} open scrutiny obligations'],
  'adv.open': 'open', 'adv.acknowledged': 'acknowledged',
  'dec.confirmed': 'confirmed', 'dec.waived': 'waived',
  'role.admin': 'admin', 'role.engineer': 'engineer', 'role.viewer': 'viewer',

  // common words
  'yes': 'yes', 'no': 'no', 'retry': 'Retry', 'cancel': 'Cancel', 'close': 'Close',
  'creating': 'Creating…', 'storing': 'Storing…', 'loading': 'Loading',
  'common.back': 'Back', 'common.create': 'Create', 'common.save': 'Save',
  'common.delete': 'Delete', 'common.signout': 'Sign out', 'common.download': 'Download',
  'job.n': 'Job #{id}', 'project.n': 'Project {id}', 'unit.deg': 'deg',
  'eng.tag': 'engine output', 'srv.tag': 'server message',
  'toast.dismiss': 'Dismiss notification',

  // skip-to-content
  'skip': 'Skip to content',

  // navigation (simplified)
  'nav.history': 'History', 'nav.more': 'More',

  // workspace tabs
  'ws.calculations': 'Calculations', 'ws.projects': 'Projects', 'ws.amoeba': 'AMOEBA',
  'ws.obligations': 'Obligations', 'ws.audit': 'Audit', 'ws.users': 'Users',
  'ws.new.calc': 'New calculation', 'ws.new.project': 'New project', 'ws.add.user': 'Add user',
  'ws.none.calc': 'No calculations yet.', 'ws.none.projects': 'No projects.',
  'ws.none.obl': 'No open obligations.', 'ws.none.audit': 'No audit entries.', 'ws.none.users': 'No users.',
  'ws.admin': 'Admin access required.',

  // convert (hero drop zone)
  'convert.title': 'Drop a PDF or click to browse',
  'convert.page': 'Page number', 'convert.recent': 'Recent conversions', 'convert.options': 'Options',
  'convert.drop': 'Drop a PDF to convert', 'convert.sub': 'Engineering drawing → verified flat-pattern DXF',
  'convert.choose': 'Choose PDF', 'convert.release': 'Release to convert',

  // processing stages
  'proc.title': 'Processing',
  'proc.upload': 'Upload', 'proc.extract': 'Extract', 'proc.pattern': 'Pattern', 'proc.verify': 'Verify',
  'proc.cancel': 'Cancel',

  // result page
  'result.approved': 'APPROVED', 'result.another': 'Convert another',
  'result.dxf': 'Download DXF', 'result.report': 'Download Report',
  'result.retry': 'Adjust options & retry',

  // login
  'login.title': 'Sign in', 'login.user': 'Username', 'login.pass': 'Password', 'login.submit': 'Sign in',
  'login.setup': 'Setup', 'login.token': 'Setup token', 'login.createAccount': 'Create account',
  'login.bad': 'Incorrect username or password.',
  'login.ended': 'Your session has ended. Sign in again.',

  // api and network
  'api.signin': 'Please sign in.',
  'api.unreachable': 'Cannot reach the server. Check that it is running, then retry.',
  'api.failed': 'Request failed ({status})',

  // errors
  'error.load': 'Failed to load', 'error.network': 'Network error',
  'error.generic': 'Something went wrong', 'error.auth': 'Authentication required',
  'error.convert': 'Conversion failed', 'error.refused': 'Conversion was refused',
  'error.timeout': 'Request timed out', 'error.pdfonly': 'Only PDF files are accepted',

  // toasts
  'toast.saved': 'Saved', 'toast.error': 'Error', 'toast.created': 'Created', 'toast.ok': 'Done',

  // badges
  'badge.approved': 'APPROVED', 'badge.review': 'REVIEW REQUIRED', 'badge.rejected': 'REJECTED',
  'badge.refused': 'REFUSED', 'badge.converted': 'CONVERTED — NOT VERIFIED — NOT RELEASED',
  'badge.error': 'ERROR', 'badge.processing': 'PROCESSING', 'badge.limitExceeded': 'LIMIT EXCEEDED',

  // table headers (workspace tabs)
  'th.job': 'Job', 'th.family': 'Geometry family', 'th.project': 'Project',
  'th.state': 'Release state', 'th.created': 'Created', 'th.name': 'Name', 'th.docs': 'Documents',
  'th.jobs': 'Jobs', 'th.username': 'Username', 'th.role': 'Role', 'th.status': 'Status',
  'th.when': 'When', 'th.user': 'User', 'th.action': 'Action', 'th.detail': 'Detail',
};

// ================================================================ 日本語
const JA = {
  'build.short': '開発版・未検証',
  'build.full': '開発版・未検証 — 出力を製造に使用してはなりません',
  'build.status': '開発版 · 未検証 · 製造使用禁止',
  'build.login': '**開発版。**未検証。出力はいずれも製造に使用してはなりません。',

  'state.APPROVED': '承認済み', 'state.REJECTED': '却下', 'state.SUPERSEDED': '旧版',
  'state.NO_RESULT': '結果なし', 'state.REVIEW_REQUIRED': '要レビュー', 'state.REFUSED': '入力拒否',
  'result.PASS': '合格', 'result.FAIL': '不合格', 'result.INCONCLUSIVE': '未確定',
  'result.NOT_IMPLEMENTED': '未実装', 'result.NOT_RUN': '未実行',
  'obl.open': '未解決', 'obl.discharged': '解消済み', 'obl.discharged_by_decision': '判断により解消', 'obl.waived': '免除',
  'obl.open.n': '未解決の要精査項目 {n}件',
  'adv.open': '未確認', 'adv.acknowledged': '確認済み',
  'dec.confirmed': '確認済み', 'dec.waived': '免除',
  'role.admin': '管理者', 'role.engineer': '技術者', 'role.viewer': '閲覧者',

  'yes': 'はい', 'no': 'いいえ', 'retry': '再試行', 'cancel': 'キャンセル', 'close': '閉じる',
  'creating': '作成中…', 'storing': '保存中…', 'loading': '読み込み中',
  'common.back': '戻る', 'common.create': '作成', 'common.save': '保存',
  'common.delete': '削除', 'common.signout': 'ログアウト', 'common.download': 'ダウンロード',
  'job.n': 'ジョブ #{id}', 'project.n': 'プロジェクト {id}', 'unit.deg': '度',
  'eng.tag': 'エンジン出力・英語', 'srv.tag': 'サーバー出力・英語',
  'toast.dismiss': '通知を閉じる',

  'skip': '本文へ移動',

  'nav.history': '履歴', 'nav.more': 'その他',

  'ws.calculations': '計算', 'ws.projects': 'プロジェクト', 'ws.amoeba': 'AMOEBA',
  'ws.obligations': '要精査項目', 'ws.audit': '監査', 'ws.users': 'ユーザー',
  'ws.new.calc': '新規計算', 'ws.new.project': '新規プロジェクト', 'ws.add.user': 'ユーザーを追加',
  'ws.none.calc': '計算はまだありません。', 'ws.none.projects': 'プロジェクトはありません。',
  'ws.none.obl': '未解決の要精査項目はありません。', 'ws.none.audit': '監査エントリーはありません。', 'ws.none.users': 'ユーザーはいません。',
  'ws.admin': '管理者権限が必要です。',

  'convert.title': 'PDFをドロップ、またはクリックして選択',
  'convert.page': 'ページ番号', 'convert.recent': '最近の変換', 'convert.options': 'オプション',
  'convert.drop': 'PDFをドロップして変換', 'convert.sub': '設計図面 → 検証済み板展開DXF',
  'convert.choose': 'PDFを選択', 'convert.release': 'ドロップして変換',

  'proc.title': '処理中',
  'proc.upload': 'アップロード', 'proc.extract': '抽出', 'proc.pattern': '展開図', 'proc.verify': '検証',
  'proc.cancel': 'キャンセル',

  'result.approved': '承認済み', 'result.another': '別のファイルを変換',
  'result.dxf': 'DXFをダウンロード', 'result.report': 'レポートをダウンロード',
  'result.retry': 'オプションを変更して再試行',

  'login.title': 'ログイン', 'login.user': 'ユーザー名', 'login.pass': 'パスワード', 'login.submit': 'ログイン',
  'login.setup': 'セットアップ', 'login.token': 'セットアップトークン', 'login.createAccount': 'アカウント作成',
  'login.bad': 'ユーザー名またはパスワードが正しくありません。',
  'login.ended': 'セッションが終了しました。もう一度ログインしてください。',

  'api.signin': 'ログインしてください。',
  'api.unreachable': 'サーバーに接続できません。サーバーが動作していることを確認してから、再試行してください。',
  'api.failed': 'リクエストが失敗しました（{status}）',

  'error.load': '読み込みに失敗しました', 'error.network': 'ネットワークエラー',
  'error.generic': 'エラーが発生しました', 'error.auth': '認証が必要です',
  'error.convert': '変換に失敗しました', 'error.refused': '変換が拒否されました',
  'error.timeout': 'リクエストがタイムアウトしました', 'error.pdfonly': 'PDFファイルのみ受け付けます',

  'toast.saved': '保存しました', 'toast.error': 'エラー', 'toast.created': '作成しました', 'toast.ok': '完了',

  'badge.approved': '承認済み', 'badge.review': '要レビュー', 'badge.rejected': '却下',
  'badge.refused': '拒否', 'badge.converted': 'PDFから変換・未検証・未リリース',
  'badge.error': 'エラー', 'badge.processing': '処理中', 'badge.limitExceeded': '上限超過',

  'th.job': 'ジョブ', 'th.family': '形状種別', 'th.project': 'プロジェクト',
  'th.state': 'リリース状態', 'th.created': '作成', 'th.name': '名称', 'th.docs': '文書',
  'th.jobs': 'ジョブ', 'th.username': 'ユーザー名', 'th.role': '権限', 'th.status': '状態',
  'th.when': '日時', 'th.user': 'ユーザー', 'th.action': '操作', 'th.detail': '詳細',
};

const MSG = { en: EN, ja: JA };
export const CATALOGUE = MSG;

// ================================================================ Engine / server identifiers
// [English label (null = show the identifier), Japanese label]. The identifier is always shown next to JA.
const ID = {
  family: {
    cylinder_shell: ['Cylindrical shell', '円筒胴'],
    cone_frustum: ['Concentric reducer', '同心レジューサ'],
    eccentric_reducer: ['Eccentric reducer', '偏心レジューサ'],
    header_with_branch: ['Header with branch (assembly)', '母管＋枝管（組立）'],
    branch_stub: ['Branch stub', '枝管スタブ'],
    header_shell_with_opening: [null, '開口付き母管胴'],
  },
  familyLong: {
    cylinder_shell: ['Cylindrical shell', '円筒胴'],
    cone_frustum: ['Concentric reducer (right cone frustum)', '同心レジューサ（直円錐台）'],
    eccentric_reducer: ['Eccentric reducer (oblique cone frustum)', '偏心レジューサ（斜円錐台）'],
    header_with_branch: ['Assembly: header shell with branch (two parts, one joint)', '組立：母管胴と枝管（2部材・取合い1箇所）'],
    branch_stub: ['Branch stub on cylindrical header', '円筒母管に取り付く枝管スタブ'],
  },
  param: {
    r_dev: ['Development radius', '展開半径'], length: ['Length', '長さ'],
    r1: ['Large-end radius', '大径側半径'], r2: ['Small-end radius', '小径側半径'], h: ['Axial height', '軸方向高さ'], e: ['Offset of small end', '小径側の偏心量'],
    header_radius: ['Header contact radius', '母管の取合い半径'], header_dev_radius: ['Header development radius', '母管の展開半径'], header_length: ['Header shell length', '母管胴の長さ'],
    branch_station: ['Branch position along header', '母管軸方向の枝管位置'], branch_radius: ['Branch contact radius', '枝管の取合い半径'], branch_dev_radius: ['Branch development radius', '枝管の展開半径'],
    beta_deg: ['Angle between axes', '両軸のなす角'], delta: ['Axis offset', '軸オフセット'],
    s_end: ['Branch square end, distance along branch axis', '枝管の直切り端（枝管軸に沿った距離）'],
    'branch_stub.s_end': ['Square end, distance along branch axis', '直切り端（枝管軸に沿った距離）'],
    surface_reference: [null, '半径の基準面'],
    header_surface_reference: ['Header contact radius: wall surface', '母管の取合い半径の基準面'],
    branch_surface_reference: ['Branch contact radius: wall surface', '枝管の取合い半径の基準面'],
    joint: ['Joint type', '取合いの形式'],
  },
  quantity: {
    seam_gap: ['Seam gap (flat pattern, small end)', '継目の隙間（展開図・小径側）', 'mm'], seam_gap_resolution: ['Seam gap resolution', '継目の隙間の分解能', 'mm'],
    area: ['Area', '面積', 'mm²'], gross_area: ['Gross area, before the opening', '総面積（開口前）', 'mm²'], net_area: ['Net area, after the opening', '正味面積（開口を差し引いた後）', 'mm²'], opening_area: ['Opening area', '開口の面積', 'mm²'], developed_width: ['Developed width', '展開幅', 'mm'], length: ['Length', '長さ', 'mm'], opening_perimeter: ['Opening perimeter', '開口の周長', 'mm'],
    opening_x_min: ['Opening, smallest x', '開口のx最小値', 'mm'], opening_x_max: ['Opening, largest x', '開口のx最大値', 'mm'],
    developed_angle: ['Developed angle', '展開角', 'rad'], sector_angle: ['Sector angle', '扇形の中心角', 'rad'],
    rho_at_0: ['Slant radius at 0°', '頂点からの母線長（0°）', 'mm'], rho_at_180: ['Slant radius at 180°', '頂点からの母線長（180°）', 'mm'],
    s_at_0: ['Cut distance at 0°', '切断距離（0°）', 'mm'], s_at_90: ['Cut distance at 90°', '切断距離（90°）', 'mm'], s_max: ['Longest cut distance', '切断距離の最大値', 'mm'], s_min: ['Shortest cut distance', '切断距離の最小値', 'mm'],
    apex_radius_large: ['Apex radius, large end', '頂点からの半径（大径側）', 'mm'], apex_radius_small: ['Apex radius, small end', '頂点からの半径（小径側）', 'mm'],
    chord_large_arc: ['Chord of large arc', '大径側円弧の弦長', 'mm'], slant_length: ['Slant length', '母線長', 'mm'], sin_half_angle: ['Sine of half apex angle', '半頂角の正弦', '–'],
  },
  part: { shell: [null, '胴板'], reducer: [null, 'レジューサ'], branch: [null, '枝管'], header: [null, '母管'], interface: [null, '取合い'] },
  curve: {
    seam_a: ['seam edge A', '継目の辺A'], seam_b: ['seam edge B', '継目の辺B'],
    large_end: ['large-end edge', '大径側の端'], small_end: ['small-end edge', '小径側の端'],
    saddle_cut: ['saddle cut against the header', '母管との相貫切断線'], square_end: ['square end', '直切り端'],
    bottom: ['end edge at axial position 0', '管端の辺（軸方向0側）'], top: ['end edge at the far end', '管端の辺（軸方向の終端側）'],
    branch_opening: ['opening for the branch', '枝管用の開口'],
  },
  entity: { line: [null, '直線'], arc: [null, '円弧'], polyline: [null, 'ポリライン'] },
  layer: { CUT_OUTER: ['profile', '外形'], CUT_INNER: ['opening', '開口'] },
  devclass: { 'E-C': ['exact development, closed form', '厳密な展開・閉形式'], 'E-N': ['exact development, evaluated numerically', '厳密な展開・数値評価'] },
  side: { outside: ['outside', '外面'], inside: ['inside', '内面'] },
  sref: { unknown: [null, '不明'], outside: [null, '外面'], inside: [null, '内面'], neutral: [null, '中立面'] },
  joint: { set_on: ['Set-on', 'セットオン'] },
  gate: {
    G00: [null, '現行版であること'], G01: [null, '来歴の完全性'], G02: [null, '未解決の曖昧さがないこと'], G05: [null, '展開クラスの割当て'], G06: [null, '前提条件の裏付け'],
    G08: [null, '数値誤差の限界'], G10: [null, '検証'], G11: [null, '輪郭の位相'], G14: [null, 'アルゴリズムの評価状況'], G15: [null, '出力の完全性'], G16: [null, '技術者承認'],
    G18: [null, '要精査項目の解消'], G19: [null, '自己整合性'], G20: [null, '既知の誤りとの照合'], G21: [null, 'エンジンが現行版であること'], G22: [null, '保存済み結果の完全性'],
  },
  check: {
    bottom_edge_equals_circumference: [null, '端の辺の長さ＝円周長'], seam_edges_equal_length: [null, '両方の継目の辺が同じ長さ'], seam_length_equals_input: [null, '継目の辺の長さ＝入力した長さ'],
    area: [null, '面積'], large_arc_equals_circumference: [null, '大径側の円弧長＝円周長'], small_arc_equals_circumference: [null, '小径側の円弧長＝円周長'],
    seam_equals_slant_length: [null, '継目の辺の長さ＝母線長'], fold_back_radius_residual: [null, '立体に戻したときの半径の残差'], large_end_present: [null, '大径側の端があること'],
    apex_free_development_agrees: [null, '頂点を使わない展開との一致'], large_edge_equals_circumference: [null, '大径側の辺の長さ＝円周長'], small_edge_equals_circumference: [null, '小径側の辺の長さ＝円周長'],
    seam_equals_3d_ruling: [null, '継目の辺の長さ＝3Dの母線長'], cut_points_on_header_surface: [null, '切断線の点が母管面上にあること'], cut_points_on_branch_surface: [null, '切断線の点が枝管面上にあること'],
    saddle_handedness_matches_declared_side: [null, '相貫切断線の向きが、宣言したけがき面と一致'], outline_counter_clockwise: [null, '外形が反時計回り'],
    square_end_equals_circumference: [null, '直切り端の長さ＝円周長'], outline_extent_along_header_axis: [null, '母管軸方向の外形寸法'], outline_extent_around_header: [null, '母管周方向の外形寸法'],
    opening_points_on_branch_surface: [null, '開口の点が枝管面上にあること'], shell_edge_equals_circumference: [null, '胴板の辺の長さ＝円周長'],
    opening_closed_simple_inside_shell: [null, '開口が閉じた単純な輪郭で、胴板の内側にあること'], parts_declare_the_same_marking_side: [null, '両部材のけがき面の宣言が同じ'],
    branch_cut_lies_on_header_opening: [null, '枝管の切断線が母管の開口線上にあること'], header_opening_lies_on_branch_cut: [null, '母管の開口線が枝管の切断線上にあること'],
    opening_on_intersection_of_branch_geometry: [null, '開口が、枝管の形状の交線上にあること'], saddle_on_intersection_of_header_geometry: [null, '相貫切断線が、母管の形状の交線上にあること'],
    opening_centre_on_the_declared_delta_side: [null, '開口の中心が、宣言したデルタ側にあること'],
    opening_matches_3d_intersection: [null, '開口が、3Dの交線と一致すること'],
    opening_developed_length_equals_3d_arc_length: [null, '開口の展開長＝3Dの円弧長'],
    opening_area_equals_surface_area: [null, '開口の面積＝表面積'],
    header_circle_3d_arc_length: [null, '母管の円の3D円弧長'],
    saddle_matches_3d_intersection: [null, '相貫切断線が、3Dの交線と一致すること'],
    saddle_on_the_declared_delta_side: [null, '相貫切断線が、宣言したデルタ側にあること'],
    saddle_developed_length_equals_3d_arc_length: [null, '相貫切断線の展開長＝3Dの円弧長'],
    pattern_area_equals_surface_area: [null, '展開図の面積＝表面積'],
    square_end_3d_arc_length: [null, '直切り端の3D円弧長'],
  },
  rfamily: { source: [null, '原資料'], claim: [null, '主張'], relation: [null, '関係'], activity: [null, 'アクティビティ'], finding: [null, '所見'], assessment: [null, '評価'], attestation: [null, '証明'] },
  rkind: {
    dxf_quality_report: [null, 'DXF品質ゲートの報告'],
    'build.develop': [null, '展開計算'], 'build.dxf': [null, 'DXF出力'], 'build.intersect': [null, '相貫線の計算'], 'verify.family': [null, '形状種別ごとの検証'], 'verify.interface': [null, '取合いの検証'],
    'verify.profile': [null, '輪郭の検査'], 'verify.reimport': [null, '再読込と照合'], gate_evaluation: [null, 'ゲート評価'], interface_evidence: [null, '取合いの検証結果'], profile_report: [null, '輪郭検査の報告'],
    reimport_report: [null, '再読込・照合の報告'], replay_audit: [null, '再実行監査'], verification_evidence: [null, '検証結果'], correction: [null, '訂正'], obligation_resolution: [null, '要精査項目の解消'],
    advisory_acknowledgement: [null, '注意喚起の確認'], convention: [null, '規約'], development_request: [null, '展開計算の依頼'], dxf_output: [null, 'DXF出力'], interface: [null, '取合い'],
    intersection_curve: [null, '相貫線'], parameter: [null, 'パラメータ'], part: [null, '部材'], pattern: [null, '展開図'], engineering_outcome: [null, 'エンジンの判定'], lesson: [null, '教訓'],
    obligation: [null, '要精査項目'], advisory: [null, '注意喚起'], snapshot: [null, 'スナップショット'], source_artifact: [null, '原資料ファイル'],
  },
  rel: {
    input: [null, '入力'], developed_from: [null, '展開の元'], verifies: [null, '検証の対象'], rendered_from: [null, '出力の元'], evaluates: [null, '評価の対象'], about: [null, '対象'], declared_in: [null, '宣言の元'],
    convention_of: [null, '同じ規約'], defined_by: [null, '定義の元'], computed_from: [null, '計算の元'], mates_at: [null, '取合い先'], interface_feature: [null, '取合いの形状'], positioned_by: [null, '位置決めの元'],
    pattern_of: [null, '展開の対象'], uses: [null, '使用'],
  },
  lesson: { corrected_value: [null, '訂正された値'], engine_failure: [null, '検証で却下された結果'], integrity_failure: [null, '完全性チェックに失敗した結果'] },
  audit: {
    login: [null, 'ログイン'], logout: [null, 'ログアウト'], login_failed: [null, 'ログイン失敗'], setup: [null, '初回セットアップ'], access_denied: [null, 'アクセス拒否'],
    password_changed: [null, 'パスワード変更'], password_change_failed: [null, 'パスワード変更の失敗'], user_created: [null, 'ユーザー作成'], user_activated: [null, 'ユーザーの再有効化'], user_deactivated: [null, 'ユーザーの無効化'],
    member_added: [null, 'メンバー追加'], member_removed: [null, 'メンバー削除'], project_created: [null, 'プロジェクト作成'], upload_accepted: [null, 'アップロード受付'], upload_rejected: [null, 'アップロード拒否'],
    calculation: [null, '計算'], calculation_refused: [null, '計算の入力拒否'], correction: [null, '訂正'], correction_refused: [null, '訂正の拒否'], lesson_recorded: [null, '教訓の記録'],
    advisory_acknowledged: [null, '注意喚起の確認'], obligation_resolved: [null, '要精査項目の解消'], replay_audit: [null, '再実行監査'], replay_unavailable: [null, '再実行監査を実行できず'],
    dxf_export: [null, 'DXFのダウンロード'], stored_file_damaged: [null, '保存ファイルの破損'],
  },
  security: {
    allowed_hosts: [null, '応答するホスト名'], anti_forgery: [null, 'リクエスト偽造への対策'], audit_log: [null, '監査ログ'], blob_encryption: [null, '保存ファイルの暗号化'],
    client_address_source: [null, 'クライアントアドレスの取得元'], cookie_secure_flag: [null, '`Cookie`の`Secure`属性'], database_encrypted_at_rest: [null, 'データベースの保存時暗号化'],
    first_run_setup: [null, '初回セットアップ'], key_rotation: [null, '鍵のローテーション'], login_throttling: [null, 'ログイン試行の制限'], multi_factor_authentication: [null, '多要素認証'],
    password_hashing: [null, 'パスワードのハッシュ化'], project_isolation: [null, 'プロジェクト間の分離'], sandboxed_parsers: [null, 'パーサーのサンドボックス化'],
    session_lifetime_hours: [null, 'セッションの有効時間（時間）'], storage_key_outside_data_directory: [null, 'ストレージ鍵をデータディレクトリの外に置く'], transport: [null, '通信路'],
  },
  dlabel: { 'header-branch joint': [null, '母管と枝管の取合い'], intersection_curve: [null, '相貫線'], surface_reference: [null, '半径の基準面'] },
};
export const idLabel = (kind, id) => { const e = ID[kind] && ID[kind][id]; return (e && e[cur === 'ja' ? 1 : 0]) || null; };
export const quantityUnit = (k) => (ID.quantity[k] ? ID.quantity[k][2] : null);
export const ID_TABLE = ID;

// ================================================================ Prose the engine and server write
// [en, ja, topic?]; {n} = number, {w} = identifier, {t} = any text. JA uses {1} {2}… by position.
const SENT = [
  ['Parameters were entered manually; they are not linked to confirmed dimensions on a source drawing (drawing intelligence is not implemented).', 'パラメータは手入力されたもので、元図面上の確認済みの寸法とは連携していません（図面の読み取りは未実装）。'],
  ['Marking side: every pattern of this job is drawn as seen from the OUTSIDE of the formed part (pattern normal toward the outside).', 'けがき面：このジョブの展開図はすべて、成形後の部材を外面側から見た図として描かれています（展開図の法線は外面側を向きます）。', 'mside'],
  ['Spec 02 §0.3 requires a declared marking side but does not choose one; this side follows from the frames the specification writes down (§4.1, §4.4) and is a PROVISIONAL convention declared by the software.', '仕様書02 §0.3は、けがき面を宣言することを求めていますが、どちらの面とするかは定めていません。この面は、仕様書に書かれた座標系（§4.1、§4.4）から導いたもので、ソフトウェアが宣言した暫定の規約です。', 'mside'],
  ['It has not been reviewed by a fabrication engineer.', '製作技術者によるレビューは受けていません。', 'mside'],
  ['Marking a pattern on the other face mirrors the part (misfit of tens of millimetres for an offset, oblique branch).', '反対の面にけがくと、部材は鏡像になります（偏心した斜めの枝管では、数十ミリメートルの食い違いになります）。', 'mside'],
  ['An engineer must confirm the marking side before use.', '使用する前に、技術者がけがき面を確認する必要があります。', 'mside'],
  ['The parts of this job are not all drawn from the declared marking side ({t}); marked on one face they would not fit.', 'このジョブの部材は、すべてが宣言されたけがき面から描かれているわけではありません（`{1}`）。同じ面にけがくと、部材どうしが合いません。', 'mside'],
  ["Surface reference declared by the user as '{w}'.", '半径の基準面は、ユーザーにより「`{1}`」と宣言されています。', 'sref'],
  ['Surface reference of the entered radii is declared UNKNOWN: inside / outside / neutral must be established by an engineer before use.', '入力した半径の基準面は「不明（`UNKNOWN`）」と宣言されています。内面・外面・中立面のいずれであるかを、使用する前に技術者が確定する必要があります。', 'sref'],
  ['The declaration is recorded, not checked against a drawing.', 'この宣言は記録されるだけで、図面との照合は行われていません。', 'sref'],
  ['Contact and development radii were checked against the declared surface of each part (Spec 02 §6.1): the direction is possible; no wall thickness is modelled, so its size is not checked.', '取合い半径と展開半径を、各部材について宣言された基準面に照らして確認しました（仕様書02 §6.1）。大小の向きは成り立ち得ます。板厚はモデル化していないため、その大きさは確認していません。', 'radii'],
  ['{w} = {n} equals the outside contact radius {w} = {n}: that is a wall of zero thickness (an idealised surface).', '`{1}` = `{2}` は、外面の取合い半径 `{3}` = `{4}` と等しくなっています。これは板厚ゼロの壁（理想化された面）を意味します。', 'radii'],
  ['{w} = {n} equals the inside contact radius {w} = {n}: that is a wall of zero thickness (an idealised surface).', '`{1}` = `{2}` は、内面の取合い半径 `{3}` = `{4}` と等しくなっています。これは板厚ゼロの壁（理想化された面）を意味します。', 'radii'],
  ['Thickness is not modelled; an engineer must confirm the radii before use.', '板厚はモデル化していません。使用する前に、技術者が半径を確認する必要があります。', 'radii'],
  ['The relation between {w} = {n} and {w} = {n} could not be checked because the surface reference is unknown.', '基準面が不明のため、{1} = {2} と {3} = {4} の関係は確認できませんでした。', 'radii'],
  ['Development class assigned by the surface-type constructor.', '展開クラスは、曲面の種類ごとの構築処理によって割り当てられました。'],
  ['Class naming scheme is a proposal (Spec 02 §14), not adopted.', 'クラスの命名体系は提案の段階（仕様書02 §14）で、採用されたものではありません。'],
  ['No calibrated process data exists.', '校正済みの工程データはありません。'],
  ['Radii are used as reference-surface radii exactly as entered; thickness, allowances, weld gap and kerf are not modelled.', '半径は、入力値をそのまま基準面の半径として使用します。板厚、各種の代（しろ）、溶接ギャップ、カーフは考慮していません。'],
  ['eps_num is an a-priori rounding bound, not interval-certified.', '`eps_num`は丸め誤差の先験的な上界であり、区間演算による保証はありません。'],
  ['For an eccentric reducer its quadrature term is an ESTIMATE, not a bound.', '偏心レデューサでは、その求積項は推定値であり、上界ではありません。'],
  ['eps_rep is a certified upper bound (interval arithmetic, TOLERANCE_POLICY section 8) for a branch saddle and a header opening, where the pattern states it certified.', '`eps_rep`は、分岐の斜め部分と開口部について、認証された上界です（区間演算。許容誤差の方針文書 第8節）。'],
  ['For an eccentric reducer eps_rep is NOT certified: checks that would pass with it are INCONCLUSIVE; a FAIL stays FAIL.', '偏心レデューサの`eps_rep`は認証されていません。これを用いると合格となる検査は判定不能になり、不合格は不合格のままです。'],
  ['eps_rep is a certified upper bound of the chord error of the drawn polyline, eps_num included (interval arithmetic on the closed-form derivatives, docs/backend/TOLERANCE_POLICY.md section 8.1).', '`eps_rep`は、描かれた折れ線の弦の誤差の認証済みの上界であり、`eps_num`を含みます（導関数の閉形式に対する区間演算。許容誤差の方針文書 第8.1節）。'],
  ['eps_rep is NOT certified for this pattern (no bound could be proved, or its vertex term carries the quadrature estimate of the eccentric reducer): checks that would pass with it are INCONCLUSIVE (docs/backend/TOLERANCE_POLICY.md section 8.2).', 'このパターンの`eps_rep`は認証されていません（上界を証明できなかったか、その頂点項に偏心レデューサの求積の推定値が含まれるため）。これを用いると合格となる検査は判定不能になります（許容誤差の方針文書 第8.2節）。'],
  ['Independent-formulation checks (class I-F), including interface constraints where the job has several parts.', '独立した定式化による検証項目（クラスI-F）です。ジョブに複数の部材がある場合は、取合いの拘束条件を含みます。'],
  ['No separately implemented verifier or external kernel cross-check exists yet.', '別に実装された検証器や、外部カーネルとの突き合わせは、まだありません。'],
  ['Outer profiles closed, simple, counter-clockwise; openings closed, simple, clockwise (binary64 arithmetic, not exact predicates).', '外形は閉じた単純な輪郭で反時計回り、開口は閉じた単純な輪郭で時計回りです（`binary64`演算による判定で、厳密な述語ではありません）。'],
  ['Kerf-compensation ownership is not declared.', 'カーフ補正をどちらが行うかは、宣言されていません。'],
  ['Algorithms are evidence level EV-1 (AI-generated, not independently reviewed) and not production-qualified.', 'アルゴリズムは証拠レベルEV-1（AIが生成、独立したレビューなし）で、製造用途としての認定は受けていません。'],
  ['Every DXF re-imported by the independent reader and compared with the intended geometry.', 'すべてのDXFを独立したリーダーで再読込し、意図した形状と照合しました。'],
  ['CAM acceptance not tested.', '`CAM`での受入は試験していません。'],
  ['Engineer approval workflow is not implemented.', '技術者承認のワークフローは未実装です。'],
  ['This result cannot be released for manufacturing.', 'この結果を製造用にリリースすることはできません。'],
  ['{n} of {n} mandatory scrutiny obligations are OPEN.', '必須の要精査項目{2}件のうち、{1}件が未解決です。'],
  ["They cannot be closed by budget or time, only by evidence or an engineer's decision.", 'これらは、予算や時間の都合では解消できません。解消できるのは、証拠または技術者の判断だけです。'],
  ['All {n} scrutiny obligations are resolved.', '要精査項目{1}件は、すべて解決済みです。'],
  ['The propagation rule table is provisional ({t}), so closure cannot be certified.', '伝播の規則表は暫定版（`{1}`）のため、解消を保証することはできません。'],
  ['No scrutiny obligations recorded for this result.', 'この結果に対して記録された要精査項目はありません。'],
  ['The propagation rule table is provisional ({t}).', '伝播の規則表は暫定版（`{1}`）です。'],
  ['No replay audit has been run for this result.', 'この結果に対して、再実行監査はまだ実行されていません。'],
  ['Audits A1-A4 are not implemented.', '監査A1-A4は未実装です。'],
  ['The stored replay report is unreadable.', '保存されている再実行の報告を読み取れません。'],
  ['The result is rejected until a replay audit succeeds.', '再実行監査が成功するまで、この結果は却下となります。'],
  ['An earlier replay attempt could not be run (infrastructure failure, not a finding about this result).', '以前の再実行は実行できませんでした（基盤側の障害であり、この結果についての所見ではありません）。'],
  ['Run the replay audit again.', '再実行監査をもう一度実行してください。'],
  ['Replay audit ({w}) reproduced the release closure.', '再実行監査（`{1}`）で、リリース閉包が再現されました。'],
  ['Audits A1-A4 are not implemented, and the replay used the same machine and build.', '監査A1-A4は未実装です。また、再実行には同じマシンと同じビルドを使用しました。'],
  ['Replay audit did NOT reproduce the stored result.', '再実行監査で、保存済みの結果は再現されませんでした。'],
  ['The result is rejected and must be investigated.', 'この結果は却下となり、調査が必要です。'],
  ['{n} advisory(ies) open: this job repeats something that went wrong before in this project.', '未確認の注意喚起が{1}件あります。このジョブは、このプロジェクトで以前に誤りとなった内容を繰り返しています。'],
  ['An engineer must acknowledge each with a reason.', '技術者が、それぞれを理由を付けて確認する必要があります。'],
  ['{n} advisory(ies) raised and acknowledged by an engineer.', '注意喚起{1}件が発生し、技術者が確認済みです。'],
  ['No match among the {n} lesson(s) recorded in this project.', 'このプロジェクトに記録された教訓{1}件の中に、一致するものはありません。'],
  ['Lessons cover only mistakes already seen here.', '教訓が対象とするのは、ここで既に起きた誤りだけです。'],
  ['Superseded by job #{n} after a correction.', '訂正により、ジョブ #{1} に置き換えられました。'],
  ['This result must not be used.', 'この結果を使用してはなりません。'],
  ['Radii are taken as reference-surface (neutral) radii exactly as entered; thickness, neutral-factor, weld gap, shrinkage and kerf are not modelled.', '半径は、入力値をそのまま基準面（中立面）の半径として扱います。板厚、中立軸係数、溶接ギャップ、収縮、カーフは考慮していません。'],
  ['Mathematical model from Specification 02 (provisional; not independently reviewed; evidence level EV-1).', '数学モデルは仕様書02によります（暫定版。独立したレビューなし。証拠レベルEV-1）。'],
  ['Marking side: drawn as seen from the OUTSIDE of the formed part.', 'けがき面：成形後の部材を外面側から見た図です。'],
  ['Marking side: drawn as seen from the INSIDE of the formed part.', 'けがき面：成形後の部材を内面側から見た図です。'],
  ['Provisional convention of this build (Spec 02 §0.3 requires a declared side and does not choose one); not reviewed by a fabrication engineer.', 'このビルドの暫定規約です（仕様書02 §0.3は面の宣言を求めていますが、どちらの面とするかは定めていません）。製作技術者によるレビューは受けていません。'],
  ['Wall inclination varies around an oblique cone; a uniform reference-surface offset is a simplification (Spec 02 research item R-05).', '斜円錐では、壁の傾きが周方向で変わります。基準面のオフセットを一様とするのは簡略化です（仕様書02 研究項目R-05）。'],
  ['Seam placed at theta = 0; no seam-placement rule applied.', '継目は `theta` = 0 の位置に置いています。継目の位置に関する規則は適用していません。'],
  ['Saddle cut = the curve where the branch surface of the entered contact radius meets the header surface of the entered contact radius (Spec 02 §4.4).', '相貫切断線＝入力した取合い半径による枝管の面と、入力した取合い半径による母管の面が交わる曲線です（仕様書02 §4.4）。'],
  ['The cut is square through the wall (constant height through the thickness, as a cut perpendicular to the flat plate gives). Weld preparation, bevel and root gap are not modelled.', '切断は板厚を貫く直角の切断です（平板に垂直な切断が与える、厚さ方向で一定の高さ）。開先加工、ベベル、ルートギャップは考慮していません。'],
  ['Weld preparation and root gap are not modelled.', '開先加工とルートギャップは考慮していません。'],
  ['Polyline density: {n} segments per curve, uniform in the angle; doubled from the base number until the measured chord error is within the chord error limit.', 'ポリラインの密度：曲線1本あたり{1}セグメント。角度について一様で、測定した弦誤差が弦誤差の上限に収まるまで、基本数から倍にしています。'],
  ['Chord error limit: {n} mm, an implementation limit pending engineering review (docs/backend/TOLERANCE_POLICY.md); not an engineering tolerance.', '弦誤差の上限：{1} mm。工学的な公差ではなく、工学的レビュー待ちの実装上の上限です（`docs/backend/TOLERANCE_POLICY.md`）。'],
  ['Joint not declared: the saddle and the opening are cut on one curve of the entered contact radii, a zero-thickness idealisation.', '取合いの形式は未宣言です。相貫切断線と開口は、入力した取合い半径による1本の曲線で切断しています。板厚をゼロとした理想化です。'],
  ['A set-on branch is cut at its bore: declare the joint as set-on (request value set_on) with header_surface_reference outside and branch_surface_reference inside.', 'セットオンの枝管は、その内径で切断します。セットオンとするには、取合いを`set_on`として宣言し、母管の基準面を`outside`、枝管の基準面を`inside`としてください。'],
  ['Every DXF is re-imported by a separate reader and compared with the intended geometry, and checked by the DXF quality gate (open contours, overlaps, self-intersection, orientation, extents, release marking); a file that fails is not offered.', 'すべてのDXFを別のリーダーで再読込し、意図した形状と照合します。また、DXF品質ゲート（開いた輪郭、重なり、自己交差、向き、外形寸法、リリース表示）で検査します。不合格のファイルは提供しません。'],
  ['Seam at branch azimuth phi = 0.', '継目は、枝管の方位角 `phi` = 0 の位置です。'],
  ['Header rolled from plate; longitudinal seam placed diametrically opposite the branch.', '母管は、板を巻いて製作する前提です。長手継手は、枝管の正反対の位置に置いています。'],
  ['No seam-to-opening clearance rule is applied.', '継目と開口の離れに関する規則は適用していません。'],
  ['Opening = the curve where the branch surface of the declared radius meets the header surface of the declared radius (Spec 02 §4.4), cut square through the wall.', '開口＝宣言した半径による枝管の面と、宣言した半径による母管の面が交わる曲線です（仕様書02 §4.4）。切断は板厚を貫く直角の切断です。'],
  ['Which wall surfaces these are and the joint type are declared by the user and recorded with the job; weld preparation, root gap and bevel are not modelled.', 'これらがどの壁面かと取合いの形式は、ユーザーが宣言し、ジョブとともに記録したものです。開先加工、ルートギャップ、ベベルは考慮していません。'],
  ['Which wall surfaces these are, and the joint type, are declarations of the user recorded with the job.', 'これらがどの壁面かと、取合いの形式は、ユーザーが宣言し、ジョブとともに記録したものです。'],
  ['Replay ran in a separate process that received only the request, on the same machine with the same build (the child reported the same engine identifier).', '再実行は、リクエストだけを受け取る別プロセスで、同じマシン・同じビルドで行いました（子プロセスは同じエンジン識別子を報告しました）。'],
  ['A separately built binary or separate machine is not used.', '別にビルドしたバイナリや、別のマシンは使用していません。'],
  ['Discrepancies are reported, never repaired.', '不一致は報告されるだけで、修復されることはありません。'],
  ['This audit shows reproducibility, not correctness.', 'この監査が示すのは再現性であり、正しさではありません。'],
  ['Detects alteration or removal of entries inside the chain, and removal of the newest entries: the latest hash, the entry count and the latest id are stored as an anchor authenticated with the storage key.', 'チェーン内のエントリーの改変と削除、および最新のエントリーの削除を検出します。最新のハッシュ、エントリー数、最新のIDを、ストレージ鍵で認証したアンカーとして保存しています。'],
  ['Someone who can write the database file AND read the storage key can rebuild the chain and the anchor without trace.', 'データベースファイルへの書き込みと、ストレージ鍵の読み取りの両方ができる者は、痕跡を残さずにチェーンとアンカーを作り直せます。'],
  ['Someone who can write the database file can also restore an older copy of the whole database together with its older anchor (rollback); that is not detected.', 'データベースファイルに書き込める者は、古いアンカーを含むデータベース全体の古いコピーを復元すること（ロールバック）もできます。これは検出されません。'],
  ['An anchor held outside this machine is not implemented.', 'このマシンの外にアンカーを保持する機能は、未実装です。'],
];

const WHOLE = [
  ['set-on (Spec 02 section 4.4: branch inside surface on header outside surface; the opening is the branch bore)', 'セットオン（仕様書02 4.4節：母管の外面に枝管の内面。開口は枝管の内径）'],
  ['not declared (the branch end cut and the header opening are cut on one curve of the entered radii)', '取合いの形式は未宣言（枝管の端部の切断と母管の開口を、入力した半径による1本の曲線で切断）'],
  ['circumference 2*pi*r from the input radius', '入力した半径から求めた円周長 2*pi*r'],
  ['length of the opposite seam edge', '反対側の継目の辺の長さ'],
  ['input length', '入力した長さ'],
  ['lateral area 2*pi*r*L', '側面積 2*pi*r*L'],
  ['3D end-circle circumference 2*pi*r1', '3Dの端円の円周長 2*pi*r1'],
  ['3D end-circle circumference 2*pi*r2', '3Dの端円の円周長 2*pi*r2'],
  ['slant length from Pythagoras on the axial section', '軸を含む断面での三平方の定理による母線長'],
  ['lateral area pi*(r1+r2)*s of the 3D frustum', '3Dの円錐台の側面積 pi*(r1+r2)*s'],
  ['implicit taper equation of the frustum evaluated at folded-back arcs', '立体に戻した円弧上で評価した、円錐台のテーパーの陰関数式'],
  ['pattern structure', '展開図の構成'],
  ['geodesic-curvature integration of the large end circle (no apex), RK4', '大径側の端円の測地的曲率の積分（頂点を使わない）、`RK4`'],
  ['3D circumference 2*pi*r1 (polyline chord shortening allowed for)', '3Dの円周長 2*pi*r1（ポリラインの弦による短縮を見込む）'],
  ['3D circumference 2*pi*r2 (polyline chord shortening allowed for)', '3Dの円周長 2*pi*r2（ポリラインの弦による短縮を見込む）'],
  ['distance between the 3D end-circle points at theta = 0', '`theta` = 0 における、3Dの両端円上の点の距離'],
  ['surface integral of |S_theta x S_lambda| over the 3D ruled surface', '3Dの線織面上での |`S_theta` x `S_lambda`| の面積分'],
  ['fold-back by the declared marking side, then distance from the header axis (implicit equation)', '宣言されたけがき面に従って立体に戻し、母管軸からの距離を評価（陰関数式）'],
  ['fold-back by the declared marking side, then |(P-O) x d| distance from the branch axis', '宣言されたけがき面に従って立体に戻し、枝管軸からの距離 |(P-O) x d| を評価'],
  ['s(270 deg) - s(90 deg) read from the pattern through the declared marking side against 2*r*cot(beta)', '宣言されたけがき面に従って展開図から読み取った `s(270 deg) - s(90 deg)` を、`2*r*cot(beta)` と比較'],
  ['sign of the signed area of the outline: outer profiles are counter-clockwise in the pattern frame (Spec 02 section 0.3)', '外形の符号付き面積の符号。外形は、展開図の座標系で反時計回りです（仕様書02 0.3節）'],
  ['circumference 2*pi*r_dev', '円周長 `2*pi*r_dev`'],
  ['largest x of the plate outline against the input header length', '板の外形のx最大値を、入力した母管の長さと比較'],
  ['largest y of the plate outline against 2*pi*r from the input radius', '板の外形のy最大値を、入力した半径から求めた 2*pi*r と比較'],
  ['fold-back onto the header by the declared marking side, then |(P-O) x d| distance from the branch axis', '宣言されたけがき面に従って母管上に戻し、枝管軸からの距離 |(P-O) x d| を評価'],
  ['profile test of the opening and bounds test against the shell rectangle', '開口の輪郭の検査と、胴板の長方形に対する範囲の検査'],
  ['declared marking side of the header pattern against that of the branch pattern (1 = outside, -1 = inside)', '母管の展開図で宣言されたけがき面を、枝管の展開図のものと比較（1 = 外面、-1 = 内面）'],
  ['3D distance from each folded-back branch cut point to the folded-back header opening curve', '立体に戻した枝管切断線の各点から、立体に戻した母管の開口曲線までの3D距離'],
  ['3D distance from each folded-back opening point to the folded-back branch cut curve', '立体に戻した開口の各点から、立体に戻した枝管の切断曲線までの3D距離'],
  ['Recompute the intersection curve from the changed parameters', '変更されたパラメータから、相貫線を再計算する'],
  ['Recompute the development and pass family verification and the profile check', '展開を再計算し、形状種別ごとの検証と輪郭の検査に合格する'],
  ['Regenerate the DXF and pass the independent re-import check', 'DXFを再出力し、独立した再読込・照合に合格する'],
  ['Pass the interface check: both parts must reproduce the same joint curve', '取合いの確認に合格する。両方の部材が、同じ取合い線を再現すること'],
  ['Re-check the fit of this part at the joint', 'この部材の、取合い部での合いを再確認する'],
  ['Engineer to re-confirm this value: it was entered under the same convention as the corrected one', '技術者がこの値を再確認する。訂正された値と同じ規約のもとで入力されたため'],
  ["requires an engineer's decision", '技術者の判断が必要'],
  ["recomputed; verification checks for '{w}' and the profile check pass", '再計算済み。「`{1}`」の検証項目と輪郭の検査に合格'],
  ['regenerated; independent re-import check passes', '再出力済み。独立した再読込・照合に合格'],
  ['recomputed; both parts reproduce the same joint curve', '再計算済み。両方の部材が同じ取合い線を再現'],
  ['interface check passes on the recomputed parts', '再計算した部材で、取合いの確認に合格'],
  ['no passing evidence in the recomputed job', '再計算したジョブに、合格を示す証拠がありません'],
  ['{w} = {n} repeats a value that was corrected in this project (job #{n}: {n} -> {n}; reason: {t}). Confirm that {n} is right this time.', '{1} = {2} は、このプロジェクトで訂正された値の繰り返しです（ジョブ #{3}：{4} -> {5}、理由：{6}）。今回は {7} で正しいことを確認してください。'],
  ['This exact request produced a result rejected by verification before in this project (job #{n}).', 'これと全く同じリクエストは、このプロジェクトで以前、検証で却下される結果になりました（ジョブ #{1}）。'],
  ['This exact request produced a stored result that failed its integrity check before in this project (job #{n}).', 'これと全く同じリクエストは、このプロジェクトで以前、保存結果が完全性チェックに失敗しました（ジョブ #{1}）。'],
  ['not implemented: the file is stored encrypted and hashed; no drawing content is read', '読み取りは未実装です。ファイルは暗号化し、ハッシュ化して保存されます。図面の内容は読み取りません'],
  ['content-addressed append-only records', '内容アドレス方式・追記専用の記録'],
  ['project snapshots with predecessor links', '直前のスナップショットへのリンクを持つ、プロジェクトスナップショット'],
  ['typed dependency edges (derivation, interface, common cause)', '型付きの依存関係エッジ（導出、取合い、共通原因）'],
  ['dependency propagation with scrutiny obligations (provisional rule table)', '要精査項目を伴う依存関係の伝播（暫定の規則表）'],
  ['interface constraint verification between parts', '部材間の取合い確認'],
  ['lessons from corrections and failures, matched against every later job in the project', '訂正と不合格から得た教訓（プロジェクト内の以降のすべてのジョブと照合）'],
  ['corrections as attestations with supersession', '証明としての訂正（旧版化を伴う）'],
  ['release closure hash', 'リリース閉包のハッシュ'],
  ['gate evaluation with obligation and replay state', '要精査項目と再実行の状態を反映したゲート評価'],
  ['replay audit in a separate process', '別プロセスでの再実行監査'],
  ['membrane admission checks', 'メンブレン（受入境界）での受入チェック'],
  ['round scheduler and budgets', 'ラウンドのスケジューラと予算'],
  ['exploration probes', '探索プローブ'],
  ['hypothesis-driven investigation', '仮説に基づく調査'],
  ['coverage ledger', 'カバレッジ台帳'],
  ['lanes and overlays', 'レーンとオーバーレイ'],
  ['quarantine', '隔離'],
  ['strategy switching', '戦略の切り替え'],
  ['self-consistency audits A1-A4', '自己整合性監査A1-A4'],
  ['replay on a separate machine or build', '別のマシンまたはビルドでの再実行'],
  ['engineer approval', '技術者承認'],
];

const REASON = [
  ['must be finite and > 0, got {n}', '有限で0より大きい値が必要です（入力値 {1}）'],
  ['must be finite', '有限の値が必要です'],
  ['requires r1 > r2 (large end first); equal radii is a cylinder', 'r1 > r2 が必要です（大径側を先に）。半径が等しい場合は円筒です'],
  ['near-cylindrical cone: apex-based formula is ill-conditioned and the apex-free method is not implemented in this build', '円筒に近い円錐です。頂点を基準とする式は悪条件になり、頂点を使わない方法は、このビルドでは未実装です'],
  ['offset must be finite and >= 0', '偏心量は、有限で0以上の値が必要です'],
  ['requires r1 > r2; equal radii with offset (oblique cylinder) is not implemented in this build', 'r1 > r2 が必要です。半径が等しく偏心がある場合（斜円筒）は、このビルドでは未実装です'],
  ['near-cylindrical: apex-free method not implemented in this build', '円筒に近い形状です。頂点を使わない方法は、このビルドでは未実装です'],
  ['angle between axes must be within about 10..170 deg in this build (grazing intersections are ill-conditioned; limit is an implementation choice, not an engineering rule)', 'このビルドでは、両軸のなす角はおよそ10〜170度の範囲が必要です（浅い角度の相貫は悪条件になります。この制限は実装上の選択であり、技術上の規則ではありません）'],
  ['requires |delta| + r < R: tangent and partial intersections (including the equal-diameter tee) are not implemented in this build', '`|delta| + r` < `R` が必要です。接する場合と部分的な相貫（同径のT継手を含む）は、このビルドでは未実装です'],
  ['square end must lie beyond the saddle: s_end > {n}', '直切り端は、相貫切断線より先にある必要があります：`s_end` > {1}'],
  ['branch geometry outside the supported range (see branch stub limits)', '枝管の形状が対応範囲の外です（枝管スタブの制限を参照）'],
  ['opening spans x = {n} .. {n} and does not lie inside the shell length {n}', '開口は x = {1} .. {2} にわたり、胴の長さ {3} の範囲に収まっていません'],
  ['magnitude {n} mm exceeds {n} mm, the largest size this build has been exercised with (implementation limit, not an engineering rule)', '大きさ {1} mm が、このビルドで動作を確かめた最大寸法 {2} mm を超えています（実装上の制限であり、技術上の規則ではありません）'],
  ['{n} mm is below {n} mm, the smallest dimension this build has been exercised with (implementation limit, not an engineering rule)', '{1} mm は、このビルドで動作を確かめた最小寸法 {2} mm を下回っています（実装上の制限であり、技術上の規則ではありません）'],
  ['developed angle {n} rad would overlap', '展開角 {1} rad では、展開図が重なります'],
];

const SERVER = [
  ['invalid username or password', 'ユーザー名またはパスワードが正しくありません'],
  ['not signed in', 'ログインしていません'],
  ['not found', '見つかりません'],
  ['your role does not permit this action', 'この権限では、この操作はできません'],
  ['username must be 3-64 characters: letters, digits, . _ -', 'ユーザー名は3〜64文字で、使えるのは英字、数字、. _ - です'],
  ['role must be admin, engineer or viewer', '権限は `admin`、`engineer`、`viewer` のいずれかにしてください'],
  ['username already exists', 'このユーザー名は既に使われています'],
  ['setup already completed', '初回セットアップは既に完了しています'],
  ['too many setup attempts with a wrong token; try again in 10 minutes', '誤ったトークンでのセットアップ試行が多すぎます。10分後にもう一度お試しください'],
  ['setup token required: the first administrator can only be created with the one-time setup token of this installation (shown to the installer at first start, or set with NF_SETUP_TOKEN)', 'セットアップトークンが必要です。最初の管理者は、このインストールの1回限りのセットアップトークン（初回起動時にインストールした人に表示されるか、`NF_SETUP_TOKEN`で設定したもの）がなければ作成できません'],
  ['too many failed sign-in attempts for this account; try again in 15 minutes', 'このアカウントでのログイン失敗が多すぎます。15分後にもう一度お試しください'],
  ['too many failed sign-in attempts from this address; try again in 10 minutes', 'このアドレスからのログイン失敗が多すぎます。10分後にもう一度お試しください'],
  ['too many wrong current passwords; try again in 15 minutes', '現在のパスワードの誤りが多すぎます。15分後にもう一度お試しください'],
  ['current password is incorrect', '現在のパスワードが正しくありません'],
  ['password must have at least 10 characters', 'パスワードは10文字以上にしてください'],
  ['new password must have at least 10 characters', '新しいパスワードは10文字以上にしてください'],
  ['password is too long (at most 1024 bytes)', 'パスワードが長すぎます（最大1024バイト）'],
  ['new password is too long (at most 1024 bytes)', '新しいパスワードが長すぎます（最大1024バイト）'],
  ['password must not be the user name', 'パスワードをユーザー名と同じにすることはできません'],
  ['new password must not be the user name', '新しいパスワードをユーザー名と同じにすることはできません'],
  ['this password is on the list of commonly used passwords; choose another', 'このパスワードは、よく使われるパスワードの一覧にあります。別のものにしてください'],
  ['new this password is on the list of commonly used passwords; choose another', 'このパスワードは、よく使われるパスワードの一覧にあります。別のものにしてください'],
  ['you cannot deactivate your own account', '自分自身のアカウントを無効化することはできません'],
  ['project name is required', 'プロジェクトの名称を入力してください'],
  ['project name is too long (max 200 characters)', 'プロジェクトの名称が長すぎます（最大200文字）'],
  ['file exceeds the upload limit of {n} MiB', 'ファイルが、アップロードの上限 {1} `MiB` を超えています'],
  ['too many uploads are in progress; retry shortly', '進行中のアップロードが多すぎます。少し待ってから再試行してください'],
  ['the upload was interrupted', 'アップロードが中断されました'],
  ['file rejected at the membrane: empty', 'ファイルは受け付けられませんでした：中身が空です'],
  ['file rejected at the membrane: too_large', 'ファイルは受け付けられませんでした：大きすぎます'],
  ['file rejected at the membrane: unsupported_format', 'ファイルは受け付けられませんでした：対応していない形式です'],
  ['a reason is required', '理由が必要です'],
  ['a reason is required for every correction', '訂正には必ず理由が必要です'],
  ['advisory is not open', 'この注意喚起は、未確認の状態ではありません'],
  ['obligation is not open', 'この要精査項目は、未解決の状態ではありません'],
  ['nothing to replay: the request was refused and nothing was computed', '再実行するものがありません。リクエストは受け付けられず、何も計算されていません'],
  ['this result is already superseded; correct the current one', 'この結果は既に旧版です。現行の結果を訂正してください'],
  ['this request was refused and has no result to correct; submit a new calculation', 'このリクエストは受け付けられておらず、訂正する結果がありません。新しい計算を実行してください'],
  ['corrections with dependency propagation are implemented for assembly jobs only', '依存関係の伝播を伴う訂正は、組立のジョブでのみ実装されています'],
  ['new value equals the current value', '新しい値が現在の値と同じです'],
  ['new_value must be a finite number', '新しい値は有限の数値にしてください'],
  ['unknown parameter', '不明なパラメータです'],
  ['parameter cannot be corrected', 'このパラメータは訂正できません'],
  ['this result was superseded by job #{n}; its files are withheld', 'この結果は、ジョブ #{1} により旧版となりました。ファイルは提供されません'],
  ['this result is REJECTED; its files are withheld', 'この結果は却下されています。ファイルは提供されません'],
  ['no DXF for this part', 'この部材のDXFはありません'],
  ['no DXF for this part: the request was refused and nothing was computed', 'この部材のDXFはありません。リクエストは受け付けられず、何も計算されていません'],
  ['the database is busy; nothing was changed. Retry shortly.', 'データベースが使用中です。何も変更されていません。少し待ってから再試行してください。'],
  ["the server's storage is full; nothing was changed and nothing was recorded. An administrator must free space.", 'サーバーのストレージに空きがありません。何も変更されず、何も記録されていません。管理者が空き容量を確保する必要があります。'],
  ['internal storage error; nothing was changed. The server log has the details.', 'ストレージの内部エラーです。何も変更されていません。詳細はサーバーのログにあります。'],
  ['internal error; nothing was changed. The server log has the details.', '内部エラーです。何も変更されていません。詳細はサーバーのログにあります。'],
  ['the request was not completed in time and was abandoned', 'リクエストは時間内に完了せず、打ち切られました'],
  ['missing anti-forgery header', '偽造対策のヘッダーがありません'],
  ['cross-origin request refused', '別オリジンからのリクエストは拒否されました'],
];

// ---- prose translation infrastructure ----
const NUM = '(-?(?:\\d[\\d.]*(?:e[-+]?\\d+)?|NaN|inf))';
const compile = (tpl, whole) => new RegExp('^' + tpl.split(/(\{[nwt]\})/).map((s) => (s === '{n}' ? NUM : s === '{w}' ? '([A-Za-z_][\\w.+-]*)' : s === '{t}' ? '(.+?)' : s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))).join('') + (whole ? '$' : '(?=\\s|$)'));
const table = (rows, whole) => rows.map(([en, ja, topic]) => ({ re: compile(en, whole), ja, topic: topic || null }));
let T = null;
const tables = () => T || (T = { sent: table(SENT, false), whole: table(WHOLE, true), reason: table(REASON, true), server: table(SERVER, true) });
const subst = (ja, m) => ja.replace(/\{(\d)\}/g, (_, i) => m[Number(i)]);
const wholeJa = (rows, text) => { for (const r of rows) { const m = r.re.exec(text); if (m) return subst(r.ja, m); } return null; };

function outcomeJa(text) {
  const m = /^(out of range \(([^)]+)\)|not developable|degenerate geometry): (.+)$/.exec(text); if (!m) return null;
  const why = wholeJa(tables().reason, m[3]); if (!why) return null;
  return `${m[2] ? `範囲外（${m[2]}）` : m[1] === 'not developable' ? '展開不可' : '退化した形状'}：${why}`;
}

export function engineSentences(text) {
  const out = []; let rest = String(text ?? '').trim();
  const { sent } = tables();
  while (rest) {
    let hit = null;
    for (const r of sent) { const m = r.re.exec(rest); if (m) { hit = [r, m]; break; } }
    if (!hit) {
      const cut = /^.*?[.!?](?=\s+[A-Z0-9'"(]|$)/.exec(rest); const en = cut ? cut[0] : rest;
      out.push({ en, ja: null, topic: null }); rest = rest.slice(en.length).trimStart(); continue;
    }
    const [r, m] = hit; out.push({ en: m[0], ja: subst(r.ja, m), topic: r.topic }); rest = rest.slice(m[0].length).trimStart();
  }
  return out;
}

export function engineJa(text) {
  const s = String(text ?? '').trim(); if (!s) return null;
  const { whole, server } = tables();
  let ja = wholeJa(whole, s) || outcomeJa(s) || wholeJa(server, s);
  if (ja) return ja;
  let m = /^No pattern produced: (.+)$/.exec(s); if (m) { const o = outcomeJa(m[1]); return o ? `展開図は生成されませんでした：${o}` : null; }
  m = /^request refused, nothing was computed: (.+)$/.exec(s); if (m) { const o = outcomeJa(m[1]); return o ? `リクエストは受け付けられず、何も計算されていません：${o}` : null; }
  m = /^correction refused, nothing was changed: ([A-Za-z_]\w*) = (\S+) cannot be computed \((.+)\)\. Job #(\d+) is still the current result\.$/.exec(s);
  if (m) { const o = outcomeJa(m[3]); return o ? `訂正は受け付けられず、何も変更されていません：${m[1]} = ${m[2]} は計算できません（${o}）。現行の結果は、引き続きジョブ #${m[4]} です。` : null; }
  const segs = engineSentences(s);
  return segs.length && segs.every((x) => x.ja) ? segs.map((x) => x.ja).join('') : null;
}

export const PROSE_TABLES = { SENT, WHOLE, REASON, SERVER };
