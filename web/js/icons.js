const NS = 'http://www.w3.org/2000/svg';

const esc = (s) => String(s).replace(/[&<>"']/g, (c) =>
  ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

function attrs(name, { size = 20, class: cls, label } = {}) {
  const a = { width: size, height: size, viewBox: '0 0 24 24' };
  if (cls) a.class = cls;
  if (label) { a['aria-label'] = label; a.role = 'img'; }
  else a['aria-hidden'] = 'true';
  a['data-icon'] = name;
  return a;
}

export function icon(name, opts = {}) {
  const svg = document.createElementNS(NS, 'svg');
  for (const [k, v] of Object.entries(attrs(name, opts))) svg.setAttribute(k, String(v));
  const use = document.createElementNS(NS, 'use');
  use.setAttribute('href', `#i-${name}`);
  svg.appendChild(use);
  return svg;
}

export function iconHtml(name, opts = {}) {
  const a = Object.entries(attrs(name, opts))
    .map(([k, v]) => `${k}="${esc(v)}"`).join(' ');
  return `<svg xmlns="${NS}" ${a}><use href="#i-${esc(name)}"></use></svg>`;
}
