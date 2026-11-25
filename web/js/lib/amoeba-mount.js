/* NEXUS FABRIC — AMOEBA mount points. A [data-amoeba] element hosts the living organism when the AMOEBA module
   (web/js/lib/amoeba.js) is present. Without the module, or without WebGL, the element keeps its text and nothing
   breaks. The data-stage attribute (idle | surround | engulf | digest | release | hold) comes from the real job
   state; callers set it with setAmoebaStage(). */
import { fxQuality } from './motion.js';

let modPromise = null;
function loadModule() {
  if (!modPromise) modPromise = import('./amoeba.js').catch(() => null);
  return modPromise;
}

export async function mountAmoebas(root) {
  const hosts = [...root.querySelectorAll('[data-amoeba]')];
  if (!hosts.length) return [];
  const mod = await loadModule();
  if (!root.isConnected || !mod || typeof mod.createAmoeba !== 'function') return [];
  const handles = [];
  for (const host of hosts) {
    try {
      const a = mod.createAmoeba(host, { quality: fxQuality() });
      a.setStage(host.dataset.stage || 'idle');
      handles.push({ host, a });
    } catch {
      // no WebGL or no room: the element keeps its text
    }
  }
  return handles;
}

export function setAmoebaStage(handles, stage) {
  for (const h of handles || []) {
    h.host.dataset.stage = stage;
    try { h.a.setStage(stage); } catch { /* module not ready */ }
  }
}

export function unmountAmoebas(handles) {
  for (const h of handles || []) {
    try { h.a.dispose(); } catch { /* already disposed */ }
  }
}
