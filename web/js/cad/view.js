// Canvas view: transform, pan and zoom (mouse, touch and pen through pointer events), fit, drawing of the
// entities of the visible layers. World units are the drawing's; y grows upward on screen.

export function createView(canvas) {
  const ctx = canvas.getContext('2d');
  const v = { scale: 1, ox: 0, oy: 0, entities: [], visible: new Set(), extents: null, dpr: 1, colour: () => null, marks: null, highlight: null };
  const pointers = new Map();
  let drag = null;

  function resize() {
    const r = canvas.getBoundingClientRect();
    v.dpr = window.devicePixelRatio || 1;
    canvas.width = Math.max(1, Math.round(r.width * v.dpr));
    canvas.height = Math.max(1, Math.round(r.height * v.dpr));
    draw();
  }

  function fit() {
    const r = canvas.getBoundingClientRect();
    const e = v.extents;
    if (!e) { v.scale = 1; v.ox = r.width / 2; v.oy = r.height / 2; draw(); return; }
    const w = Math.max(e[2] - e[0], 1e-9), h = Math.max(e[3] - e[1], 1e-9);
    v.scale = Math.min((r.width - 40) / w, (r.height - 40) / h);
    v.ox = (r.width - (e[0] + e[2]) * v.scale) / 2;
    v.oy = (r.height + (e[1] + e[3]) * v.scale) / 2;
    draw();
  }

  // screen (CSS px) -> world
  function toWorld(sx, sy) {
    return [(sx - v.ox) / v.scale, (v.oy - sy) / v.scale];
  }
  function toScreen(x, y) {
    return [v.ox + x * v.scale, v.oy - y * v.scale];
  }
  function zoomAt(sx, sy, f) {
    const s = Math.min(Math.max(v.scale * f, 1e-6), 1e9);
    const [wx, wy] = toWorld(sx, sy);
    v.scale = s;
    v.ox = sx - wx * s;
    v.oy = sy + wy * s;
    draw();
  }
  function local(ev) {
    const r = canvas.getBoundingClientRect();
    return [ev.clientX - r.left, ev.clientY - r.top];
  }

  // handlers the page can set: click(worldPoint, screenPoint) for a tap that did not move
  const hooks = { click: null, change: null };

  canvas.addEventListener('wheel', (ev) => {
    ev.preventDefault();
    const [sx, sy] = local(ev);
    zoomAt(sx, sy, Math.exp(-ev.deltaY * 0.0015));
  }, { passive: false });

  canvas.addEventListener('pointerdown', (ev) => {
    canvas.setPointerCapture(ev.pointerId);
    pointers.set(ev.pointerId, local(ev));
    const [sx, sy] = local(ev);
    drag = { x: sx, y: sy, moved: false, start: [sx, sy], pinch: null };
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      drag.pinch = Math.hypot(a[0] - b[0], a[1] - b[1]);
    }
  });

  canvas.addEventListener('pointermove', (ev) => {
    if (!pointers.has(ev.pointerId)) return;
    pointers.set(ev.pointerId, local(ev));
    if (!drag) return;
    if (pointers.size >= 2) {
      const [a, b] = [...pointers.values()];
      const d = Math.hypot(a[0] - b[0], a[1] - b[1]);
      if (drag.pinch && d > 0) {
        zoomAt((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, d / drag.pinch);
        drag.pinch = d;
      }
      drag.moved = true;
      return;
    }
    const [sx, sy] = local(ev);
    const dx = sx - drag.x, dy = sy - drag.y;
    if (Math.hypot(sx - drag.start[0], sy - drag.start[1]) > 4) drag.moved = true;
    if (drag.moved) {
      v.ox += dx;
      v.oy += dy;
      draw();
    }
    drag.x = sx;
    drag.y = sy;
  });

  function end(ev) {
    const wasTap = drag && !drag.moved && pointers.size === 1;
    pointers.delete(ev.pointerId);
    if (wasTap && hooks.click) {
      const [sx, sy] = drag.start;
      hooks.click(toWorld(sx, sy), [sx, sy]);
    }
    if (pointers.size === 0) drag = null;
    else if (drag) drag.pinch = null;
  }
  canvas.addEventListener('pointerup', end);
  canvas.addEventListener('pointercancel', end);

  function draw() {
    const d = v.dpr;
    const w = canvas.width / d, h = canvas.height / d;
    ctx.setTransform(d, 0, 0, d, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const css = getComputedStyle(canvas);
    const fg = css.getPropertyValue('--fg').trim() || '#1d1d1b';
    for (const e of v.entities) {
      if (!v.visible.has(e.layer)) continue;
      ctx.strokeStyle = v.colour(e.layer) || fg;
      ctx.fillStyle = ctx.strokeStyle;
      ctx.lineWidth = 1;
      strokeGeom(e.geom);
    }
    if (v.highlight) {
      ctx.strokeStyle = css.getPropertyValue('--accent').trim() || '#1c5d99';
      ctx.lineWidth = 2.5;
      strokeGeom(v.highlight.geom);
    }
    if (v.marks) {
      ctx.fillStyle = css.getPropertyValue('--accent').trim() || '#1c5d99';
      for (const p of v.marks) {
        const [x, y] = toScreen(p[0], p[1]);
        ctx.beginPath(); ctx.arc(x, y, 4, 0, Math.PI * 2); ctx.fill();
      }
      if (v.marks.length === 2) {
        ctx.strokeStyle = ctx.fillStyle; ctx.lineWidth = 1.5; ctx.beginPath();
        const [a, b] = [toScreen(...v.marks[0]), toScreen(...v.marks[1])];
        ctx.moveTo(a[0], a[1]); ctx.lineTo(b[0], b[1]); ctx.stroke();
      }
    }
  }

  function path(points, closed) {
    ctx.beginPath();
    points.forEach((p, i) => {
      const [x, y] = toScreen(p[0], p[1]);
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    });
    if (closed) ctx.closePath();
    ctx.stroke();
  }

  // arcs and bulge segments are drawn as polylines of short chords (sampled in world units)
  function arcPoints(c, r, start, end, n = 64) {
    let sw = (end - start + 360) % 360;
    if (sw === 0) sw = 360;
    const out = [];
    for (let i = 0; i <= n; i++) {
      const a = (start + (sw * i) / n) * Math.PI / 180;
      out.push([c[0] + r * Math.cos(a), c[1] + r * Math.sin(a)]);
    }
    return out;
  }
  function bulgePoints(a, b, bulge, n = 32) {
    const out = [];
    const theta = 4 * Math.atan(bulge);
    const chord = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (Math.abs(theta) < 1e-12 || chord === 0) return [a, b];
    const r = chord / (2 * Math.abs(Math.sin(theta / 2)));
    const mid = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
    const left = [-(b[1] - a[1]) / chord, (b[0] - a[0]) / chord];
    const off = r * Math.cos(theta / 2) * Math.sign(theta);
    const c = [mid[0] + left[0] * off, mid[1] + left[1] * off];
    const a0 = Math.atan2(a[1] - c[1], a[0] - c[0]);
    for (let i = 0; i <= n; i++) {
      const ang = a0 + theta * (i / n);
      out.push([c[0] + r * Math.cos(ang), c[1] + r * Math.sin(ang)]);
    }
    return out;
  }

  function strokeGeom(g) {
    switch (g.type) {
      case 'point': { const [x, y] = toScreen(g.at[0], g.at[1]); ctx.fillRect(x - 1, y - 1, 2, 2); break; }
      case 'line': path([g.a, g.b], false); break;
      case 'circle': path(arcPoints(g.c, g.r, 0, 360, 96), true); break;
      case 'arc': path(arcPoints(g.c, g.r, g.start, g.end), false); break;
      case 'poly': {
        const pts = [];
        const n = g.v.length, segs = g.closed ? n : n - 1;
        for (let i = 0; i < segs; i++) {
          const a = [g.v[i][0], g.v[i][1]], b = [g.v[(i + 1) % n][0], g.v[(i + 1) % n][1]];
          const seg = g.v[i][2] ? bulgePoints(a, b, g.v[i][2]) : [a, b];
          pts.push(...(pts.length ? seg.slice(1) : seg));
        }
        if (!segs && n) pts.push([g.v[0][0], g.v[0][1]]);
        path(pts, g.closed);
        break;
      }
      case 'text': { const [x, y] = toScreen(g.at[0], g.at[1]); ctx.font = '11px system-ui'; ctx.fillText(g.text, x, y); break; }
      default: break;
    }
  }

  // a hit on screen: the world distance to a geometry is measured by the server; the page only highlights
  window.addEventListener('resize', resize);
  resize();

  return {
    v, hooks, draw, fit, resize, toWorld, toScreen,
    set(entities, extents) { v.entities = entities; v.extents = extents; fit(); },
  };
}
