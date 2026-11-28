// NF CAD strings, English and Japanese. Kept apart from i18n.js (a hotspot for merges).
export const STRINGS = {
  en: {
    title: 'NF CAD', open: 'Open', loading: 'Loading…', noDrawing: 'No drawing. Open a document or a job part.',
    pan: 'Pan', pick: 'Pick', measure: 'Measure', fit: 'Fit', checks: 'Quality checks', layers: 'Layers',
    lang: '日本語', units: 'Units', unknown: 'unknown (not declared)', entities: 'Entities', version: 'Version',
    distance: 'Distance', dxdy: 'dx, dy', angle: 'Direction', picked: 'Picked', none: 'Nothing here',
    clickA: 'Click the first point', clickB: 'Click the second point', length: 'Length', area: 'Area',
    pass: 'PASS', fail: 'FAIL', signIn: 'Sign in to NF to open drawings.', error: 'Error',
    readOnly: 'Read-only view: nothing here edits the drawing.', truncated: 'Showing {n} of {total} entities',
    notTo: 'NOT TO SCALE unless a print scale is chosen', warnings: 'Warnings',
    editH: 'Edit (documents only)', move: 'Move', rotate: 'Rotate', mirror: 'Mirror', scale: 'Scale',
    del: 'Delete', addLine: 'Add line', setLayer: 'Set layer', undo: 'Undo', redo: 'Redo',
    dx: 'dx', dy: 'dy', deg: 'Angle (°)', factor: 'Factor', layerName: 'Layer',
    needSelection: 'Pick an entity first.', needLine: 'Click the start point, then the end point. Snaps are on.',
    clickEnd: 'Click the end point', editNote: 'Each edit makes a new version; the original is kept. Edited files are NOT verified NF patterns.',
    notEditable: 'This view is a job part: only documents can be edited.', noPivot: 'This entity has no centre to work from.',
    confirmDelete: 'Delete this entity in a new version?', nothingRedo: 'Nothing to redo.',
    flashEdited: 'New version #{id} created. It is not a verified NF pattern.', agentRefused: 'An agent cannot edit drawings: only a person can.',
    note: 'Note', insideBlock: 'Inside a block: edit its INSERT instead.',
  },
  ja: {
    title: 'NF CAD', open: '開く', loading: '読み込み中…', noDrawing: '図面がありません。書類またはジョブの部品を開いてください。',
    pan: '移動', pick: '選択', measure: '計測', fit: '全体表示', checks: '品質チェック', layers: 'レイヤー',
    lang: 'English', units: '単位', unknown: '不明（未宣言）', entities: '要素数', version: 'バージョン',
    distance: '距離', dxdy: 'dx, dy', angle: '方向', picked: '選択', none: '該当なし',
    clickA: '1点目をクリック', clickB: '2点目をクリック', length: '長さ', area: '面積',
    pass: '合格', fail: '不合格', signIn: '図面を開くにはNFにサインインしてください。', error: 'エラー',
    readOnly: '閲覧専用です。ここから図面は変更されません。', truncated: '{total} 件中 {n} 件を表示',
    notTo: '印刷縮尺を選ばない限り縮尺なし', warnings: '警告',
    editH: '編集（書類のみ）', move: '移動', rotate: '回転', mirror: '鏡映', scale: '拡大縮小',
    del: '削除', addLine: '線を追加', setLayer: 'レイヤー変更', undo: '元に戻す', redo: 'やり直す',
    dx: 'dx', dy: 'dy', deg: '角度（度）', factor: '倍率', layerName: 'レイヤー',
    needSelection: '先に要素を選択してください。', needLine: '1点目をクリックし、次に2点目をクリックしてください。スナップ有効。',
    clickEnd: '2点目をクリック', editNote: '編集のたびに新しいバージョンが作られ、元の図面は残ります。編集したファイルは検証済みNFパターンではありません。',
    notEditable: 'この表示はジョブの部品です。編集できるのは書類のみです。', noPivot: 'この要素には基準点がありません。',
    confirmDelete: 'この要素を削除した新しいバージョンを作りますか？', nothingRedo: 'やり直す操作がありません。',
    flashEdited: '新しいバージョン #{id} を作成しました。検証済みNFパターンではありません。', agentRefused: 'エージェントは図面を編集できません。人のみ可能です。',
    note: '注記', insideBlock: 'ブロックの中の要素です。INSERTを編集してください。',
  },
};

let lang = 'en';
try {
  const q = new URLSearchParams(location.search).get('lang');
  const saved = localStorage.getItem('nfcad.lang');
  const pref = q || saved || ((navigator.language || '').startsWith('ja') ? 'ja' : 'en');
  lang = pref === 'ja' ? 'ja' : 'en';
} catch (_) { lang = 'en'; }

export function getLang() { return lang; }
export function setLang(l) {
  lang = l === 'ja' ? 'ja' : 'en';
  try { localStorage.setItem('nfcad.lang', lang); } catch (_) { /* private window: keep the session value only */ }
}
export function t(key, vars) {
  let s = (STRINGS[lang] && STRINGS[lang][key]) || STRINGS.en[key] || key;
  if (vars) for (const k of Object.keys(vars)) s = s.replace(`{${k}}`, String(vars[k]));
  return s;
}
