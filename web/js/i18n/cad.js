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
