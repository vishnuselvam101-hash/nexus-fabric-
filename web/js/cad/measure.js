// Measure and pick state: turns canvas clicks into requests for the server's measurements. The numbers come from
// the server (nf-cad); this file only keeps the two-click state and builds the request.

export function createMeasure() {
  let first = null;
  return {
    reset() { first = null; },
    pending() { return first !== null; },
    first() { return first; },
    // returns { wait: true } after the first point, or { query } with the two points after the second
    click(p) {
      if (!first) { first = p; return { wait: true, first: p }; }
      const a = first;
      first = null;
      return { query: { kind: 'distance', from: a, to: p }, both: [a, p] };
    },
  };
}

export function q(params) {
  const u = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null) continue;
    u.set(k, Array.isArray(v) ? v.map((n) => (typeof n === 'number' ? n : n)).join(',') : String(v));
  }
  return u.toString();
}
