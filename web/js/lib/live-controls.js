// NEXUS FABRIC live controls: pause / resume, undo last, take over, and the Watch / Fast / Instant speed.
// Pause, resume and undo are a person's: the server refuses them from the AI operator. Nothing here approves or releases.
import { api, esc, ApiError } from '../api.js';
import { t } from '../i18n.js';

const SPEEDS = ['watch', 'fast', 'instant'];
const store = {
  get() { try { const v = localStorage.getItem('nf.opspeed'); return SPEEDS.includes(v) ? v : 'watch'; } catch { return 'watch'; } },
  set(v) { try { localStorage.setItem('nf.opspeed', v); } catch { /* storage may be blocked */ } },
};

/**
 * Mount the control bar into `host`. `cursor` is the operator cursor (for speed). Returns { update(poll), destroy() },
 * where update takes the server's event-list answer ({ paused, last_seq }).
 */
export function mountLiveControls(host, { oid, cursor, onChange }) {
  let paused = false, taken = false, lastSeq = 0, moved = false, speed = store.get();
  const bar = document.createElement('div');
  bar.className = 'lv-bar';
  bar.setAttribute('role', 'group');
  bar.setAttribute('aria-label', t('jf2.live.controls'));
  host.append(bar);
  const err = document.createElement('p');
  err.className = 'lv-err'; err.setAttribute('role', 'alert'); err.hidden = true;
  host.append(err);
  const note = document.createElement('p');
  note.className = 'lv-note'; note.setAttribute('aria-live', 'polite');
  host.append(note);
  cursor?.setSpeed(speed);

  const call = async (path, json) => {
    err.hidden = true; note.textContent = '';
    try { await api(`/api/orders/${oid}/operator/${path}`, { json: json || {} }); return true; }
    catch (e) { err.hidden = false; err.textContent = t('jf2.live.err', { msg: e instanceof ApiError ? e.message : String(e?.message || e) }); return false; }
  };
  const state = () => (taken ? 'taken' : paused ? 'paused' : moved ? 'work' : 'idle');
  const draw = () => {
    bar.dataset.state = state();
    bar.innerHTML = `<span class="lv-status" role="status"><i class="lv-dot" aria-hidden="true"></i>${esc(t(`jf2.live.s.${state()}`))}</span>
      <button type="button" class="lv-btn" data-lv="${paused ? 'resume' : 'pause'}" ${taken ? 'disabled' : ''}>${esc(t(paused ? 'jf2.live.resume' : 'jf2.live.pause'))}</button>
      <button type="button" class="lv-btn" data-lv="undo">${esc(t('jf2.live.undo'))}</button>
      <button type="button" class="lv-btn" data-lv="${taken ? 'hand' : 'take'}">${esc(t(taken ? 'jf2.live.hand' : 'jf2.live.take'))}</button>
      <span class="lv-seg" role="group" aria-label="${esc(t('jf2.live.speed'))}">${SPEEDS.map((s) => `<button type="button" data-sp="${s}" aria-pressed="${s === speed}">${esc(t(`jf2.live.sp.${s}`))}</button>`).join('')}</span>`;
  };
  bar.addEventListener('click', async (ev) => {
    const b = ev.target.closest('button');
    if (!b) return;
    if (b.dataset.sp) { speed = b.dataset.sp; store.set(speed); cursor?.setSpeed(speed); return draw(); }
    const act = b.dataset.lv;
    if (act === 'pause' && await call('pause')) { paused = true; cursor?.pause(); }
    else if (act === 'resume' && await call('resume')) { paused = false; taken = false; cursor?.resume(); }
    else if (act === 'take' && await call('pause')) { paused = true; taken = true; cursor?.pause(); }
    else if (act === 'hand' && await call('resume')) { paused = false; taken = false; cursor?.resume(); }
    else if (act === 'undo' && await call('undo')) note.textContent = t('jf2.live.undone');
    draw();
    if (onChange) onChange(act);
  });
  draw();
  return {
    update(d) {
      if (!d) return;
      const was = state();
      paused = !!d.paused;
      if (!paused) taken = false;
      if (d.last_seq > lastSeq) { if (lastSeq || d.last_seq) moved = true; lastSeq = d.last_seq; }
      if (state() !== was) draw();
    },
    destroy() { bar.remove(); err.remove(); note.remove(); },
  };
}
