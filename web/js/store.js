let state = {
  view: 'loading',
  user: null,
  setupRequired: false,
  projectId: null,
  convert: { phase: 'idle', file: null, progress: 0, conversionId: null, result: null, error: null },
  history: [],
  advanced: { tab: 'calculations' },
};

const listeners = new Set();

function getState() { return state; }

function setState(patch) {
  state = { ...state, ...patch };
  for (const fn of listeners) fn(state);
}

function subscribe(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function patchConvert(patch) {
  setState({ convert: { ...state.convert, ...patch } });
}

export { getState, setState, subscribe, patchConvert };
