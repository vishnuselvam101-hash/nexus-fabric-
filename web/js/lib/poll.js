import { api } from '../api.js';

export function pollConversion(id, onUpdate, signal) {
  let delay = 750;
  let elapsed = 0;
  let paused = false;

  const onVis = () => {
    if (document.hidden) { paused = true; return; }
    paused = false;
    tick();
  };
  document.addEventListener('visibilitychange', onVis);

  async function tick() {
    if (signal && signal.aborted) { cleanup(); return; }
    if (paused) return;
    try {
      const r = await api(`/api/pdf-conversions/${id}`);
      onUpdate(r);
      if (r.status === 'converted' || r.status === 'refused' || r.status === 'limit_exceeded') {
        cleanup();
        return;
      }
    } catch (e) {
      if (e.status === 404) { onUpdate({ status: 'error', error: 'Conversion not found' }); cleanup(); return; }
    }
    elapsed += delay;
    if (elapsed > 5000) delay = 1500;
    if (elapsed > 30000) delay = 3000;
    if (elapsed > 300000) { onUpdate({ status: 'timeout' }); cleanup(); return; }
    setTimeout(tick, delay);
  }

  function cleanup() { document.removeEventListener('visibilitychange', onVis); }

  setTimeout(tick, delay);
  return () => { signal = { aborted: true }; cleanup(); };
}
