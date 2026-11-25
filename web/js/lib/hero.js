/* NEXUS FABRIC — home hero: one small WebGL scene (steel plane, ruled grid, one accent sweep).
   Own shader, no library. Colours come from the theme tokens. Without WebGL the stage keeps its CSS grid;
   with prefers-reduced-motion one frame is drawn and nothing animates. The loop stops once the canvas leaves the page. */
const VS = 'attribute vec2 p; void main(){ gl_Position = vec4(p, 0.0, 1.0); }';
const FS = `precision mediump float;
uniform vec2 uRes; uniform float uT; uniform vec3 uBg; uniform vec3 uLine; uniform vec3 uAcc;
void main(){
  vec2 uv = (gl_FragCoord.xy - 0.5 * uRes) / uRes.y;
  vec3 col = uBg;
  float h = 0.12 - uv.y;               // distance below the horizon (horizon at uv.y = 0.12, behind the drop surface)
  if (h > 0.0) {
    float z = 1.0 / h;                  // depth of the plane under this pixel
    vec2 q = vec2(uv.x * z, z + uT * 0.6) * 1.6;
    vec2 g = abs(fract(q) - 0.5);
    float ln = 1.0 - smoothstep(0.0, 0.035, min(g.x, g.y));
    float fade = smoothstep(0.0, 0.5, h) * (1.0 - smoothstep(1.2, 2.6, z));
    col = mix(col, uLine, ln * 0.7 * fade);
    float sweep = exp(-pow((uv.x - (fract(uT * 0.07) * 2.4 - 1.2)) * 3.0, 2.0));
    col = mix(col, uAcc, ln * sweep * fade * 0.85);
  }
  col = mix(col, uAcc, 0.16 * exp(-abs(h) * 14.0) * (1.0 - smoothstep(0.2, 1.1, abs(uv.x))));   // a soft molten horizon
  col = mix(uBg, col, smoothstep(1.35, 0.35, length(uv * vec2(0.7, 1.0))));   // vignette fades to the background, not to black
  gl_FragColor = vec4(col, 1.0);
}`;

const FALLBACK = { bg: [0.055, 0.063, 0.075], line: [0.23, 0.26, 0.30], acc: [1.0, 0.42, 0.10] };

function rgb(css, fb) {
  const m = /#([0-9a-f]{6})/i.exec(String(css).trim());
  if (!m) return fb;
  const n = parseInt(m[1], 16);
  return [(n >> 16) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

function themeColours(el) {
  const cs = getComputedStyle(el);
  return {
    bg: rgb(cs.getPropertyValue('--bg'), FALLBACK.bg),
    line: rgb(cs.getPropertyValue('--border-strong'), FALLBACK.line),
    acc: rgb(cs.getPropertyValue('--accent'), FALLBACK.acc),
  };
}

function compile(gl, type, src) {
  const s = gl.createShader(type);
  gl.shaderSource(s, src); gl.compileShader(s);
  return gl.getShaderParameter(s, gl.COMPILE_STATUS) ? s : null;
}

export function startHero(canvas) {
  const reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const gl = canvas.getContext('webgl', { antialias: false, alpha: false, powerPreference: 'low-power' });
  const vs = gl && compile(gl, gl.VERTEX_SHADER, VS);
  const fs = gl && compile(gl, gl.FRAGMENT_SHADER, FS);
  if (!gl || !vs || !fs) { canvas.parentElement?.classList.add('no-gl'); return; }
  const prog = gl.createProgram();
  gl.attachShader(prog, vs); gl.attachShader(prog, fs); gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) { canvas.parentElement?.classList.add('no-gl'); return; }
  gl.useProgram(prog);
  // one oversized triangle covers the viewport
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(prog, 'p');
  gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  const U = {
    res: gl.getUniformLocation(prog, 'uRes'), t: gl.getUniformLocation(prog, 'uT'),
    bg: gl.getUniformLocation(prog, 'uBg'), line: gl.getUniformLocation(prog, 'uLine'), acc: gl.getUniformLocation(prog, 'uAcc'),
  };
  const dpr = Math.min(window.devicePixelRatio || 1, 1.5);   // cap the pixel cost for office PCs
  let col = themeColours(canvas), frames = 0, raf = 0;
  const size = () => {
    const r = canvas.getBoundingClientRect();
    canvas.width = Math.max(1, Math.round(r.width * dpr));
    canvas.height = Math.max(1, Math.round(r.height * dpr));
    gl.viewport(0, 0, canvas.width, canvas.height);
  };
  const draw = (t) => {
    if (frames++ % 30 === 0) col = themeColours(canvas);
    gl.uniform2f(U.res, canvas.width, canvas.height); gl.uniform1f(U.t, t);
    gl.uniform3fv(U.bg, col.bg); gl.uniform3fv(U.line, col.line); gl.uniform3fv(U.acc, col.acc);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };
  const t0 = performance.now();
  // weak GPU: if most frames of a 120-frame window take over 50 ms, the scene is dropped for the static grid
  let last = 0, n = 0, slow = 0;
  const frame = (now) => {
    raf = 0;
    if (!canvas.isConnected) return;               // the view was left: stop the loop
    if (last) { n++; if (now - last > 50) slow++; }
    last = now;
    if (n >= 120) {
      if (slow > 40) { canvas.hidden = true; canvas.parentElement?.classList.add('no-gl'); return; }
      n = 0; slow = 0;
    }
    draw((now - t0) / 1000);
    if (!reduce && !document.hidden) raf = requestAnimationFrame(frame);
  };
  size();
  const onVis = () => {
    if (!canvas.isConnected) { document.removeEventListener('visibilitychange', onVis); return; }
    if (!document.hidden && !raf && !reduce) raf = requestAnimationFrame(frame);
  };
  document.addEventListener('visibilitychange', onVis);
  if (typeof ResizeObserver !== 'undefined') {
    new ResizeObserver(() => { size(); if (reduce) draw(0); else if (!raf) raf = requestAnimationFrame(frame); }).observe(canvas);
  }
  if (reduce) draw(0); else raf = requestAnimationFrame(frame);
}
