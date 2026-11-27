/* NEXUS FABRIC — start screen scene. A dark 3D room with a fine perspective grid, a huge extruded steel NF mark with a
   molten-orange rim, and a steel plate that unrolls under it (the signature motion). One WebGL canvas that fills the
   host; the mark sits where the `anchor` element is, so the text column is never covered. The scene is an illustration:
   it shows no data. Everything the user needs (PDF drop, scale, recent jobs) is plain DOM above it.

   Quality tiers (decided here, on the user's own machine):
     full   WebGL2, one post pass (chromatic RGB split on the open and drop transitions, vignette)
     lite   same scene rendered straight to the screen, no post pass
     static no WebGL, prefers-reduced-motion, or a machine that cannot hold 30 fps: an inline SVG picture
   A frame-time monitor demotes full -> lite -> static when the average of 40 frames is slower than 34 ms.
   `?tier=full|lite|static` forces a tier (for measurement). No network, no extra libraries: three.js is vendored. */
import * as THREE from '../../vendor/three/three.module.js';
import { reducedMotion } from './motion.js';

const ACCENT = 0xff6a1a;
const COOL = 0x7fb4d9;
const SLOW_MS = 34;          // above this average frame time the tier is lowered
const CYCLE = 26;            // seconds per unroll loop

// the NF mark: two polygons on a 6.6 x 4 grid (N, then F)
export const MARK_W = 6.6;
export const MARK_H = 4;
const N_PTS = [[0, 0], [0.8, 0], [0.8, 2.5], [2.2, 0], [3, 0], [3, 4], [2.2, 4], [2.2, 1.5], [0.8, 4], [0, 4]];
const F_PTS = [[0, 0], [0.8, 0], [0.8, 1.6], [2.4, 1.6], [2.4, 2.3], [0.8, 2.3], [0.8, 3.3], [3, 3.3], [3, 4], [0, 4]];
const LETTERS = [{ pts: N_PTS, dx: 0 }, { pts: F_PTS, dx: 3.6 }];

const clamp01 = (x) => Math.min(1, Math.max(0, x));
const seg = (t, a, b) => clamp01((t - a) / (b - a));
const smooth = (x) => x * x * (3 - 2 * x);
const easeOut = (x) => 1 - Math.pow(1 - x, 3);

/* The static picture (also the WebGL-off fallback): the NF mark and a flat plate on a perspective grid. Colours come
   from the theme tokens, so it follows light/dark. Returns an <svg>. */
export function staticArt() {
  const NS = 'http://www.w3.org/2000/svg';
  const mk = (tag, attrs) => { const e = document.createElementNS(NS, tag); for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v); return e; };
  const svg = mk('svg', { viewBox: '0 0 800 500', class: 'home-art', 'aria-hidden': 'true', preserveAspectRatio: 'xMidYMid slice' });
  const g = mk('g', { class: 'art-grid' });
  for (let i = -10; i <= 10; i++) g.appendChild(mk('line', { x1: 400 + i * 20, y1: 300, x2: 400 + i * 85, y2: 500 }));
  for (let j = 0; j < 9; j++) { const y = 300 + 200 * Math.pow(j / 8, 1.8); g.appendChild(mk('line', { x1: 0, y1: y, x2: 800, y2: y })); }
  svg.appendChild(g);
  svg.appendChild(mk('path', { class: 'art-plate', d: 'M120 430 L680 430 L740 480 L60 480 Z' }));
  const s = 52, ox = 400 - (MARK_W * s) / 2, oy = 420;
  for (const L of LETTERS) {
    const d = L.pts.map(([x, y], i) => `${i ? 'L' : 'M'}${(ox + (x + L.dx) * s).toFixed(1)} ${(oy - y * s).toFixed(1)}`).join(' ') + ' Z';
    svg.appendChild(mk('path', { class: 'art-mark', d }));
  }
  return svg;
}

function studioEnvironment(renderer) {
  const pmrem = new THREE.PMREMGenerator(renderer);
  const env = new THREE.Scene();
  env.background = new THREE.Color(0x0a0b0d);
  const panel = (w, h, color, k, pos) => {
    const m = new THREE.Mesh(new THREE.PlaneGeometry(w, h), new THREE.MeshBasicMaterial({ color: new THREE.Color(color).multiplyScalar(k), side: THREE.DoubleSide }));
    m.position.set(...pos); m.lookAt(0, 0, 0); env.add(m);
  };
  panel(12, 3, 0xffffff, 1.6, [0, 9, 3]);
  panel(10, 6, 0xdfe8f2, 0.45, [0, 2, 10]);
  panel(3, 14, COOL, 1.8, [-10, 0, 2]);
  panel(2, 11, ACCENT, 1.5, [10, -1, 1]);
  const tex = pmrem.fromScene(env, 0.03).texture;
  pmrem.dispose();
  return tex;
}

function markGeometry() {
  const geos = [];
  for (const L of LETTERS) {
    const sh = new THREE.Shape();
    L.pts.forEach(([x, y], i) => (i ? sh.lineTo(x + L.dx, y) : sh.moveTo(x + L.dx, y)));
    sh.closePath();
    const g = new THREE.ExtrudeGeometry(sh, { depth: 0.7, bevelEnabled: true, bevelThickness: 0.07, bevelSize: 0.06, bevelSegments: 2, curveSegments: 1 });
    g.translate(-MARK_W / 2, 0, -0.35);
    geos.push(g);
  }
  return geos;
}

const PLATE_L = 9, PLATE_D = 3.4, PL_NX = 150, PL_NZ = 6, PL_R = 0.55;
/* plate unrolled by q in [0,1]: 0 = a tight roll, 1 = flat. The roll lies on its end and travels as it opens. */
function plateGeometry() {
  const n = (PL_NX + 1) * (PL_NZ + 1);
  const pos = new Float32Array(n * 3);
  const idx = [];
  for (let j = 0; j < PL_NZ; j++) for (let i = 0; i < PL_NX; i++) {
    const a = j * (PL_NX + 1) + i, b = a + 1, c = a + PL_NX + 1, d = c + 1;
    idx.push(a, c, b, b, c, d);
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
  geo.setIndex(idx);
  const edge = [];
  const lp = [];
  for (let i = 0; i <= PL_NX; i++) lp.push(i);
  for (let j = 1; j <= PL_NZ; j++) lp.push(j * (PL_NX + 1) + PL_NX);
  for (let i = PL_NX - 1; i >= 0; i--) lp.push(PL_NZ * (PL_NX + 1) + i);
  for (let j = PL_NZ - 1; j >= 1; j--) lp.push(j * (PL_NX + 1));
  for (let k = 0; k < lp.length; k++) edge.push(lp[k], lp[(k + 1) % lp.length]);
  const lineGeo = new THREE.BufferGeometry();
  lineGeo.setAttribute('position', geo.attributes.position);
  lineGeo.setIndex(edge);
  let lastQ = NaN;
  return { geo, lineGeo, set(q) {
    if (q === lastQ) return;
    lastQ = q;
    const flatLen = PLATE_L * q, rolledLen = PLATE_L - flatLen;
    const xc = PLATE_L / 2 - flatLen;                     // contact point of the roll with the floor
    for (let j = 0; j <= PL_NZ; j++) {
      const z = (j / PL_NZ) * PLATE_D - 0.4;
      for (let i = 0; i <= PL_NX; i++) {
        const s = (i / PL_NX) * PLATE_L, a = s - rolledLen;   // arc length from the contact point
        let x, y;
        if (a >= 0) { x = xc + a; y = 0; }
        else {
          const rho = PL_R * (1 - 0.3 * (-a / PLATE_L));
          const phi = a / PL_R;
          x = xc + rho * Math.sin(phi); y = rho * (1 - Math.cos(phi));
        }
        const o = (j * (PL_NX + 1) + i) * 3; pos[o] = x; pos[o + 1] = y; pos[o + 2] = z;
      }
    }
    geo.attributes.position.needsUpdate = true;
    geo.computeVertexNormals();
  } };
}

const POST_VS = 'varying vec2 vUv; void main(){ vUv = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }';
const POST_FS = `
precision highp float;
uniform sampler2D tex; uniform float amt; uniform float time; varying vec2 vUv;
float hash(float n){ return fract(sin(n) * 43758.5453); }
void main(){
  vec2 uv = vUv; vec2 d = uv - 0.5;
  float band = step(0.5, hash(floor(uv.y * 14.0) + floor(time * 18.0))) * amt;
  uv.x += (hash(floor(uv.y * 14.0) + 3.0) - 0.5) * 0.06 * band;
  float k = 0.0012 + amt * 0.018;
  vec2 o = d * k * (1.0 + 4.0 * dot(d, d)) + vec2(amt * 0.004, 0.0);
  vec3 c = vec3(texture2D(tex, uv + o).r, texture2D(tex, uv).g, texture2D(tex, uv - o).b);
  c *= 1.0 - 0.55 * dot(d, d);
  gl_FragColor = vec4(c, 1.0);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}`;

const palette = (light) => light
  ? { bg: 0xeef1f4, grid: 0x2b6c97, gridOp: 0.28, fog: 0xeef1f4, exposure: 1.0 }
  : { bg: 0x0a0c0f, grid: 0x7fb4d9, gridOp: 0.34, fog: 0x0a0c0f, exposure: 1.1 };
const isLight = () => getComputedStyle(document.documentElement).colorScheme === 'light';

/* 0 = no WebGL2, 1 = software renderer (no GPU: the picture stays static), 2 = a GPU */
function gpuClass() {
  try {
    const gl = document.createElement('canvas').getContext('webgl2');
    if (!gl) return 0;
    const x = gl.getExtension('WEBGL_debug_renderer_info');
    const name = String((x ? gl.getParameter(x.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER)) || '');
    const soft = /swiftshader|llvmpipe|softpipe|software|basic render/i.test(name);
    gl.getExtension('WEBGL_lose_context')?.loseContext();
    return soft ? 1 : 2;
  } catch { return 0; }
}

/* createHomeScene(host, anchor): returns { tier, pulse(), dispose(), onTier } or null when the tier is static.
   host: element the canvas fills (position: relative). anchor: element whose box the mark fills. */
export function createHomeScene(host, anchor) {
  const forced = new URLSearchParams(location.search).get('tier');
  if (reducedMotion() || forced === 'static') return null;
  const gc = gpuClass();
  if (gc === 0 || (gc === 1 && forced !== 'full' && forced !== 'lite')) return null;
  const weak = (navigator.deviceMemory && navigator.deviceMemory <= 4) || (navigator.hardwareConcurrency && navigator.hardwareConcurrency <= 4);
  let tier = forced === 'full' || forced === 'lite' ? forced : (weak ? 'lite' : 'full');
  const auto = !(forced === 'full' || forced === 'lite');

  const canvas = document.createElement('canvas');
  canvas.className = 'home-canvas';
  canvas.setAttribute('aria-hidden', 'true');
  let renderer;
  try { renderer = new THREE.WebGLRenderer({ canvas, antialias: tier === 'full', powerPreference: 'high-performance' }); } catch { return null; }
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  host.prepend(canvas);

  const scene = new THREE.Scene();
  scene.environment = studioEnvironment(renderer);
  scene.environmentIntensity = 1.0;
  const camera = new THREE.PerspectiveCamera(32, 1, 0.1, 120);
  const CAM = new THREE.Vector3(0, 2.4, 13), LOOK = new THREE.Vector3(0, 1.0, 0);
  scene.add(new THREE.HemisphereLight(0xdfe8f2, 0x0e1013, 0.25));
  const key = new THREE.DirectionalLight(0xffffff, 2.2); key.position.set(4, 7, 8); scene.add(key);
  const rim = new THREE.DirectionalLight(COOL, 1.8); rim.position.set(-6, 3, -4); scene.add(rim);
  const orange = new THREE.DirectionalLight(ACCENT, 3.2); orange.position.set(6, 2.5, -5); scene.add(orange);
  const warm = new THREE.PointLight(ACCENT, 7, 20, 1.8); warm.position.set(-6, -0.5, 4); scene.add(warm);

  // perspective grid: floor and back wall; the floor drifts toward the viewer
  const gridMat = new THREE.LineBasicMaterial({ transparent: true, depthWrite: false });
  const floor = new THREE.GridHelper(120, 120, 0xffffff, 0xffffff); floor.material = gridMat; floor.position.y = -0.04;
  const wall = new THREE.GridHelper(120, 120, 0xffffff, 0xffffff); wall.material = gridMat; wall.rotation.x = Math.PI / 2; wall.position.z = -22; wall.position.y = 20;
  scene.add(floor, wall);

  // the part: NF mark + plate share one group, scaled to the anchor
  const world = new THREE.Group(); scene.add(world);
  const steel = new THREE.MeshStandardMaterial({ color: 0xd2dae2, metalness: 1, roughness: 0.3, envMapIntensity: 2.4 });
  const plateMat = new THREE.MeshStandardMaterial({ color: 0xc3ccd6, metalness: 0.9, roughness: 0.32, side: THREE.DoubleSide, envMapIntensity: 1.2 });
  const edgeMat = new THREE.LineBasicMaterial({ color: new THREE.Color(ACCENT).multiplyScalar(1.6), transparent: true, opacity: 0.9 });
  const mark = new THREE.Group();
  const markGeos = markGeometry();
  for (const g of markGeos) {
    mark.add(new THREE.Mesh(g, steel));
    mark.add(new THREE.LineSegments(new THREE.EdgesGeometry(g, 35), edgeMat));
  }
  mark.position.set(0, 0.02, 0.2);
  const plate = plateGeometry();
  const plateMesh = new THREE.Mesh(plate.geo, plateMat);
  const plateEdge = new THREE.LineSegments(plate.lineGeo, edgeMat);
  const plateGroup = new THREE.Group(); plateGroup.add(plateMesh, plateEdge); plateGroup.position.set(0.0, 0.0, -0.3);
  world.add(mark, plateGroup);

  // post pass (full tier only)
  let rt = null, post = null, quad = null;
  const pu = { tex: { value: null }, amt: { value: 0 }, time: { value: 0 } };
  const pass = () => {
    if (rt) return;
    rt = new THREE.WebGLRenderTarget(4, 4, { type: THREE.HalfFloatType, samples: 4 });
    post = new THREE.ShaderMaterial({ vertexShader: POST_VS, fragmentShader: POST_FS, uniforms: pu, depthTest: false, depthWrite: false });
    quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), post); quad.frustumCulled = false;
  };
  const postScene = new THREE.Scene(), postCam = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

  const S = { w: 1, h: 1, scale: 1, disposed: false, visible: true, raf: 0, t0: performance.now(), pulseAt: -10, mx: 0, my: 0, cx: 0, cy: 0, frames: [], last: 0, fpsN: 0, fpsT: 0 };
  const api = { get tier() { return tier; }, onTier: null, fps: 0 };

  function applyTheme() {
    const p = palette(isLight());
    scene.background = new THREE.Color(p.bg);
    scene.fog = new THREE.Fog(p.fog, 14, 52);
    gridMat.color.set(p.grid); gridMat.opacity = p.gridOp; gridMat.fog = true;
    renderer.toneMappingExposure = p.exposure;
  }
  function setTier(next) {
    tier = next;
    host.dataset.tier = next;
    renderer.setPixelRatio(next === 'full' ? Math.min(window.devicePixelRatio || 1, 1.5) : 1);
    if (next === 'full') pass(); else if (rt) { rt.dispose(); post.dispose(); rt = post = null; }
    resize();
    if (api.onTier) api.onTier(next);
  }
  function resize() {
    const r = host.getBoundingClientRect();
    S.w = Math.max(1, Math.round(r.width)); S.h = Math.max(1, Math.round(r.height));
    renderer.setSize(S.w, S.h, false);
    if (rt) { const pr = renderer.getPixelRatio(); rt.setSize(Math.round(S.w * pr), Math.round(S.h * pr)); }
    camera.aspect = S.w / S.h;
    // world size per CSS pixel at the distance of the mark
    const k = (2 * CAM.distanceTo(LOOK) * Math.tan(THREE.MathUtils.degToRad(16))) / S.h;
    const a = anchor.getBoundingClientRect(), hr = r;
    const aw = Math.max(80, a.width), ah = Math.max(80, a.height);
    S.scale = Math.min((aw * k * 0.62) / MARK_W, (ah * k * 0.5) / MARK_H);
    world.scale.setScalar(S.scale);
    // shift the picture so that the world origin lands in the middle of the anchor
    S.cx = a.left - hr.left + a.width / 2 - S.w / 2; S.cy = a.top - hr.top + a.height / 2 - S.h / 2;
    camera.setViewOffset(S.w, S.h, -S.cx, -S.cy + ah * 0.08, S.w, S.h);
    camera.updateProjectionMatrix();
  }

  function frame(now) {
    S.raf = 0;
    if (S.disposed || !S.visible || document.hidden) return;
    const t = (now - S.t0) / 1000;
    // frame-time monitor
    if (S.last) {
      S.frames.push(now - S.last);
      const sum0 = S.frames.reduce((a, b) => a + b, 0);
      if (S.frames.length >= 40 || (S.frames.length >= 3 && sum0 > 1200)) {
        const avg = sum0 / S.frames.length;
        api.fps = Math.round(1000 / avg); host.dataset.fps = String(api.fps);
        S.frames.length = 0;
        if (auto && avg > SLOW_MS && !(t - S.stamp < 0.5)) {
          if (tier === 'full') { setTier('lite'); S.stamp = t; }
          else { api.demoted = true; if (api.onTier) api.onTier('static'); return; }
        }
      }
    }
    S.last = now;
    // timeline: rise + unroll at the start, long hold, roll up, hold
    const c = t % CYCLE, first = t < CYCLE;
    const open = easeOut(seg(t, 0.05, 1.5));
    let q = smooth(seg(c, 0.5, 4.8)) * (1 - smooth(seg(c, 17, 21.5)));
    if (first && c > 17) q = Math.max(q, 0);
    plate.set(q);
    mark.scale.set(1, 0.001 + 0.999 * open, 1);
    mark.position.y = 0.02;
    const sway = Math.sin(t * 0.32) * 0.28, mouse = S.mx * 0.22;
    world.rotation.y = sway + mouse;
    edgeMat.opacity = 0.55 + 0.35 * Math.sin(t * 0.8 + 1) * 0.5 + 0.2 * q;
    floor.position.z = (t * 0.35) % 1;
    // pulse (open / drop): 450 ms
    const pk = seg(now / 1000 - S.pulseAt, 0, 0.45);
    pu.amt.value = pk > 0 && pk < 1 ? Math.sin(pk * Math.PI) : 0;
    pu.time.value = t;
    camera.position.set(CAM.x + S.mx * 0.9, CAM.y + S.my * -0.35, CAM.z);
    camera.lookAt(LOOK);
    if (tier === 'full' && rt) {
      renderer.setRenderTarget(rt); renderer.render(scene, camera); renderer.setRenderTarget(null);
      pu.tex.value = rt.texture;
      if (!postScene.children.length) postScene.add(quad);
      renderer.render(postScene, postCam);
    } else renderer.render(scene, camera);
    S.raf = requestAnimationFrame(frame);
  }
  const kick = () => { if (!S.raf && !S.disposed) S.raf = requestAnimationFrame(frame); };

  const onMove = (e) => { S.mx = (e.clientX / window.innerWidth - 0.5) * 2; S.my = (e.clientY / window.innerHeight - 0.5) * 2; };
  window.addEventListener('pointermove', onMove, { passive: true });
  const ro = new ResizeObserver(() => { resize(); kick(); }); ro.observe(host); ro.observe(anchor);
  const io = new IntersectionObserver((es) => { S.visible = es.some((e) => e.isIntersecting); if (S.visible) { S.last = 0; kick(); } });
  io.observe(host);
  const onVis = () => { S.last = 0; kick(); };
  document.addEventListener('visibilitychange', onVis);
  const mo = new MutationObserver(applyTheme); mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
  const mq = matchMedia('(prefers-color-scheme: light)'); mq.addEventListener('change', applyTheme);

  api.pulse = () => { S.pulseAt = performance.now() / 1000; };
  api.dispose = () => {
    S.disposed = true; if (S.raf) cancelAnimationFrame(S.raf);
    ro.disconnect(); io.disconnect(); mo.disconnect(); mq.removeEventListener('change', applyTheme);
    window.removeEventListener('pointermove', onMove); document.removeEventListener('visibilitychange', onVis);
    scene.environment?.dispose?.();
    for (const g of markGeos) g.dispose();
    plate.geo.dispose(); plate.lineGeo.dispose(); steel.dispose(); plateMat.dispose(); edgeMat.dispose(); gridMat.dispose();
    floor.geometry.dispose(); wall.geometry.dispose();
    if (rt) { rt.dispose(); post.dispose(); quad.geometry.dispose(); }
    renderer.dispose(); renderer.forceContextLoss(); canvas.remove();
  };
  applyTheme();
  setTier(tier);
  S.stamp = -10;
  api.pulse();
  kick();
  return api;
}
