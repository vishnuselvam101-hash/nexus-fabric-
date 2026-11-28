import { esc } from './api.js';
import { icon } from './icons.js';
import { t } from './i18n.js';

const isNode = (v) => v instanceof Node;

function append(parent, children) {
  for (const child of children.flat(Infinity)) {
    if (child === null || child === undefined || child === false || child === true) continue;
    parent.append(isNode(child) ? child : document.createTextNode(String(child)));
  }
  return parent;
}

export function el(tag, attrs, ...children) {
  const node = document.createElement(tag);
  for (const [key, val] of Object.entries(attrs || {})) {
    if (val === null || val === undefined || val === false) continue;
    if (key.startsWith('on') && typeof val === 'function') {
      node.addEventListener(key.slice(2).toLowerCase(), val);
    } else if (key === 'class' || key === 'className') {
      node.className = Array.isArray(val) ? val.filter(Boolean).join(' ') : String(val);
    } else if (key === 'style' && typeof val === 'object') {
      Object.assign(node.style, val);
    } else if (key === 'dataset' && typeof val === 'object') {
      Object.assign(node.dataset, val);
    } else if (key === 'html') {
      node.innerHTML = val;
    } else if (key in node && key !== 'list' && typeof val !== 'object') {
      node[key] = val === true ? true : val;
    } else {
      node.setAttribute(key, val === true ? '' : String(val));
    }
  }
  return append(node, children);
}

export function frag(...children) {
  return append(document.createDocumentFragment(), children);
}

export function $(sel, root = document) {
  return root.querySelector(sel);
}

export function $$(sel, root = document) {
  return Array.from(root.querySelectorAll(sel));
}

export function toast(message, type = 'ok') {
  const kind = ['ok', 'warn', 'err'].includes(type) ? type : 'ok';
  const node = el('div', { class: `toast toast-${kind}`, role: 'status' });
  node.innerHTML = esc(String(message ?? ''));
  document.body.append(node);
  requestAnimationFrame(() => node.classList.add('is-in'));
  const dismiss = () => {
    node.classList.remove('is-in');
    setTimeout(() => node.remove(), 250);
  };
  const timer = setTimeout(dismiss, 4000);
  node.addEventListener('click', () => {
    clearTimeout(timer);
    dismiss();
  });
  return node;
}

const BADGES = {
  REVIEW_REQUIRED: { icon: 'alert', label: () => t('badge.state.review') },
  REFUSED: { icon: 'cross', label: () => t('badge.state.refused') },
  SUPERSEDED: { icon: 'clock', label: () => t('badge.state.superseded') },
};

export function badge(state) {
  const key = String(state || '').toUpperCase();
  const cfg = BADGES[key] || { icon: null, label: () => key || t('badge.state.unknown') };
  const node = el('span', { class: `badge badge-${key.toLowerCase()}` });
  if (cfg.icon) {
    const mark = icon(cfg.icon);
    if (typeof mark === 'string') {
      const wrap = el('span', { class: 'badge-icon' });
      wrap.innerHTML = mark;
      node.append(wrap);
    } else if (isNode(mark)) {
      node.append(mark);
    }
  }
  node.append(el('span', { class: 'badge-label' }, cfg.label()));
  return node;
}

export function chip(text, type = 'neutral') {
  const kind = ['ok', 'warn', 'bad', 'neutral'].includes(type) ? type : 'neutral';
  return el('span', { class: `chip chip-${kind}` }, String(text ?? ''));
}

export function kv(pairs = []) {
  const rows = [];
  for (const pair of pairs) {
    if (!pair) continue;
    rows.push(el('dt', {}, String(pair.label ?? '')));
    rows.push(
      el('dd', { class: pair.mono ? 'mono' : null }, pair.value ?? '—')
    );
  }
  return el('div', { class: 'kv' }, ...rows);
}

export function field(opts = {}) {
  const { name, label, type = 'text', value, hint, error, required } = opts;
  const id = `f-${name || Math.random().toString(36).slice(2, 8)}`;
  const input = el('input', {
    id,
    name,
    type,
    value: value ?? '',
    required: !!required,
    'aria-invalid': error ? 'true' : null,
    'aria-describedby': error ? `${id}-err` : hint ? `${id}-hint` : null,
  });
  return el(
    'div',
    { class: ['field', error ? 'has-error' : null] },
    label ? el('label', { for: id }, label, required ? el('span', { class: 'req' }, ' *') : null) : null,
    input,
    hint ? el('span', { class: 'field-hint', id: `${id}-hint` }, hint) : null,
    error ? el('span', { class: 'field-error', id: `${id}-err` }, error) : null
  );
}

export function busy(btn, loading) {
  if (!btn) return btn;
  btn.classList.toggle('is-busy', !!loading);
  btn.disabled = !!loading;
  return btn;
}

export function fmtSize(bytes) {
  const n = Number(bytes) || 0;
  if (n < 1024) return `${n} B`;
  if (n < 1048576) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1048576).toFixed(1)} MB`;
}

export function fmtTime(iso) {
  if (!iso) return '—';
  const d = new Date(typeof iso === 'number' && iso < 1e12 ? iso * 1000 : iso);
  if (Number.isNaN(d.getTime())) return String(iso);
  return d.toLocaleString();
}

export function fmtElapsed(ms) {
  const n = Math.max(0, Number(ms) || 0);
  if (n < 1000) return `${Math.round(n)} ms`;
  if (n < 60000) return `${Math.round(n / 1000)}s`;
  const m = Math.floor(n / 60000);
  const s = Math.round((n % 60000) / 1000);
  return `${m}m ${s}s`;
}

export function modal(opts = {}) {
  const { title = '', body = '', actions = [] } = opts;
  const onKey = (e) => {
    if (e.key === 'Escape') close();
  };
  const close = () => {
    document.removeEventListener('keydown', onKey);
    backdrop.classList.remove('is-in');
    backdrop.remove();
  };
  const header = el(
    'header',
    { class: 'modal-head' },
    el('h2', {}, title),
    el('button', { class: 'modal-close', type: 'button', 'aria-label': 'Close', onclick: close }, '×')
  );
  const section = el('section', { class: 'modal-body' });
  if (typeof body === 'string') section.innerHTML = esc(body);
  else if (isNode(body)) section.append(body);
  const footer = el(
    'footer',
    { class: 'modal-foot' },
    ...actions.map((action) =>
      el(
        'button',
        {
          class: `btn-${action.type || 'secondary'}`,
          type: 'button',
          onclick: (e) => {
            if (typeof action.onclick === 'function') action.onclick(e, { close });
            else close();
          },
        },
        action.label ?? 'OK'
      )
    )
  );
  const dialog = el(
    'div',
    { class: 'modal', role: 'dialog', 'aria-modal': 'true' },
    header,
    section,
    actions.length ? footer : null
  );
  const backdrop = el(
    'div',
    {
      class: 'modal-backdrop',
      onclick: (e) => {
        if (e.target === backdrop) close();
      },
    },
    dialog
  );
  document.addEventListener('keydown', onKey);
  document.body.append(backdrop);
  requestAnimationFrame(() => backdrop.classList.add('is-in'));
  return { el: backdrop, close };
}
