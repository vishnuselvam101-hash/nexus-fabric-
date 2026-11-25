/* NEXUS FABRIC — AMOEBA organism: a living, Amoeba proteus-like cell drawn in one fragment pass over the
   scene that holds the part it acts on. Own SDF shader and own bloom pass; three.js (vendored, r187) only
   for the scene and the render targets. No CDN.

   API
     const a = createAmoeba(canvasOrContainer, { quality: 'auto' | 'low' | 'med' | 'high', scene, camera })
     a.setStage('idle' | 'surround' | 'engulf' | 'digest' | 'release' | 'hold')
     a.setTarget(object3D | { x, y, w, h } | DOMRect | null)   // object3D is projected with `camera`
     a.setQuality('low' | 'med' | 'high')
     a.stats()    -> { mode, quality, fps }
     a.dispose()

   Organism units: half the canvas height, origin at the canvas centre, y up.
   Without WebGL2 the same API drives a static CSS blob. With prefers-reduced-motion one frame is drawn and
   nothing animates. The loop stops while the canvas is off-screen or the page is hidden. */
import * as THREE from '../../vendor/three/three.module.js';

const R0 = 0.27;                                   // body radius, organism units
const RATE = 4.5;                                  // stage tween rate (1/s): settles in about 0.7-1.0 s
const BG = [0x1b / 255, 0x1d / 255, 0x21 / 255];   // graphite, raw sRGB (the scene clear colour)

const STAGES = {
  idle:     { app: 0, sur: 0,   eng: 0, dig: 0, rel: 0, hold: 0, drop: 0 },
  surround: { app: 1, sur: 1,   eng: 0, dig: 0, rel: 0, hold: 0, drop: 0 },
  engulf:   { app: 1, sur: 0.6, eng: 1, dig: 0, rel: 0, hold: 0, drop: 0 },
  digest:   { app: 1, sur: 0.2, eng: 1, dig: 1, rel: 0, hold: 0, drop: 0 },
  release:  { app: 1, sur: 0,   eng: 0, dig: 0, rel: 1, hold: 0, drop: 1 },
  hold:     { app: 1, sur: 0.15, eng: 1, dig: 0, rel: 0, hold: 1, drop: 0 },
};

const QUALITY = {
  low:  { pr: 1,    lobes: 4, bloom: 0, msaa: 0, bdiv: 4 },
  med:  { pr: 1.25, lobes: 6, bloom: 1, msaa: 4, bdiv: 4 },
  high: { pr: 2,    lobes: 6, bloom: 1, msaa: 4, bdiv: 2 },
};

const VS = `varying vec2 vUv;
void main() { vUv = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }`;

const FS_ORGANISM = `precision highp float;
varying vec2 vUv;
uniform sampler2D tScene;
uniform vec2 uRes;
uniform float uT, uR0, uNL, uTgtOn, uTgtR, uSur, uEng, uDig, uRel, uHold, uDrop;
uniform vec2 uC, uTgt, uCrawl;
const float PI = 3.14159265;
const int NMAX = 6;

float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vn(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(h21(i), h21(i + vec2(1.0, 0.0)), f.x), mix(h21(i + vec2(0.0, 1.0)), h21(i + vec2(1.0, 1.0)), f.x), f.y);
}
float smin(float a, float b, float k) {
  float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
  return mix(b, a, h) - k * h * (1.0 - h);
}
float angd(float a, float b) { return atan(sin(a - b), cos(a - b)); }
vec2 dirv(float a) { return vec2(cos(a), sin(a)); }
float capT(vec2 p, vec2 a, vec2 b, float ra, float rb) {
  vec2 pa = p - a, ba = b - a;
  float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
  return length(pa - ba * h) - mix(ra, rb, h);
}
float radiusAt(float th, float R) {
  vec2 cs = dirv(th);
  float r = R * (1.0 + 0.10 * sin(2.0 * th + uT * 0.45) + 0.07 * sin(3.0 * th - uT * 0.31 + 1.3) + 0.04 * sin(5.0 * th + uT * 0.83));
  return r * (1.0 + 0.16 * (vn(cs * 1.4 + vec2(uT * 0.10, 3.7)) - 0.5));
}
// membrane + pseudopods (lobopodia) + food/part vacuole; negative inside
float field(vec2 p) {
  vec2 q = p - uC;
  float th = atan(q.y, q.x);
  float R = uR0 * (1.0 + 0.04 * uHold * sin(uT * 1.1));
  vec2 tq = uTgt - uC;
  float ang = angd(th, atan(tq.y, tq.x));
  float bulge = (0.24 * uEng - 0.50 * uRel) * uR0 * exp(-ang * ang / 0.18) * uTgtOn;
  float d = length(q) - (radiusAt(th, R) + bulge);
  float psiC = atan(uC.y - uTgt.y, uC.x - uTgt.x);
  for (int k = 0; k < NMAX; k++) {
    float fk = float(k);
    float on = step(fk + 0.5, uNL);
    vec2 dv = dirv(fk * 2.0 * PI / uNL + 0.6 * sin(fk * 2.7) + 0.28 * sin(uT * 0.19 + fk * 2.3) + 0.16 * sin(uT * 0.47 + fk * 1.1));
    float ph = fract(uT * (0.085 + 0.022 * fk) + fk * 0.173);
    float env = smoothstep(0.0, 0.40, ph) * (1.0 - smoothstep(0.50, 0.98, ph));
    float front = max(dot(dv, uCrawl), 0.0);
    float L = uR0 * (0.05 + 0.80 * env) * (0.75 + 0.6 * front);
    float Rb = radiusAt(th, R) + bulge;
    vec2 base = uC + dv * Rb * 0.55;
    vec2 tipN = uC + dv * (Rb + L);
    // crown around the part (surround); the crown closes on the far side (engulf)
    float off = (fk - 0.5 * (uNL - 1.0)) * 0.62;
    float psi = psiC + off * (0.55 + 1.35 * uEng);
    vec2 tipW = uTgt + dirv(psi) * (uTgtR * 2.6 + 0.02 * uR0 * sin(uT * 1.3 + fk * 2.0));
    vec2 tip = mix(tipN, tipW, uSur * uTgtOn);
    vec2 v = tip - base;
    vec2 perp = vec2(-v.y, v.x) / max(length(v), 1e-4);
    vec2 mid = 0.5 * (base + tip) + perp * (0.10 * length(v) * sin(uT * 0.9 + fk * 1.7));
    float dk = min(capT(p, base, mid, 0.30 * uR0, 0.23 * uR0), capT(p, mid, tip, 0.23 * uR0, 0.17 * uR0));
    d = smin(d, mix(1.0, dk, on), 0.10 * uR0);
  }
  float dvac = length(p - uTgt) - uTgtR * 1.30;
  d = smin(d, mix(1.0, dvac, uEng * uTgtOn), 0.14 * uR0);
  return d + (vn(p * 9.0 + vec2(uT * 0.3, -uT * 0.2)) - 0.5) * 0.010 * uR0;
}
void main() {
  vec2 p = (gl_FragCoord.xy - 0.5 * uRes) / (0.5 * uRes.y);
  vec3 under = texture2D(tScene, vUv).rgb;
  vec3 E = vec3(1.0, 0.42, 0.10);                       // molten NF orange
  vec3 G = vec3(0.13, 0.135, 0.15);                     // graphite
  float d = field(p);
  float cover = clamp(0.5 - d * 0.5 * uRes.y, 0.0, 1.0);
  vec3 outc = under + E * exp(-max(d, 0.0) * 22.0 / uR0) * 0.10;
  if (cover > 0.0) {
    vec2 q = p - uC;
    float r = length(q);
    float th = atan(q.y, q.x);
    float depth = max(-d, 0.0);
    float dome = sqrt(clamp(depth / (0.16 * uR0), 0.0, 1.0));
    float ecto = 1.0 - smoothstep(0.02 * uR0, 0.07 * uR0, depth);
    float cloud = 0.6 * vn(p * 3.2 + vec2(uT * 0.05, -uT * 0.03)) + 0.4 * vn(p * 7.0 - vec2(0.0, uT * 0.08));
    // granules drift outward with the streaming (cartesian cells, sampled against an outward flow)
    vec2 rd = q / max(r, 0.02);
    vec2 gq = p / 0.030 - rd * uT * 0.55;
    vec2 gc = floor(gq), gf = fract(gq) - 0.5;
    float gh = h21(gc + 0.5);
    vec2 gp = (vec2(h21(gc + 3.1), h21(gc + 7.3)) - 0.5) * 0.4;
    float gran = step(0.45, gh) * smoothstep(0.14, 0.04, length(gf - gp));
    float vol = smoothstep(0.02, 0.24, depth / uR0);     // endoplasm volume: dimmer at the membrane, full inside
    vec3 col = G * (0.8 + 0.2 * ecto);
    col += E * (0.16 + 0.40 * cloud) * vol * (1.0 - ecto);
    col += vec3(1.0, 0.78, 0.5) * gran * (0.30 + 0.30 * cloud) * vol * (1.0 - ecto);
    col += vec3(0.24, 0.26, 0.30) * ecto;                // glassy ectoplasm sheen
    // glass highlight on the upper left of the body, and a soft shade towards the lower right
    vec2 hl = p - (uC + vec2(-0.42, 0.42) * uR0);
    col += vec3(1.0, 0.92, 0.84) * exp(-dot(hl, hl) / (0.012 * uR0 * uR0)) * 0.10 * (1.0 - ecto);
    col *= 0.86 + 0.22 * dome;
    // nucleus: darker disc with a brighter rim
    vec2 N = uC + dirv(uT * 0.05 + 0.6) * 0.12 * uR0;
    float rN = 0.20 * uR0, ln = length(p - N);
    float nuc = 1.0 - smoothstep(rN - 0.010, rN + 0.005, ln);
    col = mix(col, vec3(0.20, 0.07, 0.02) + 0.10 * cloud * E, nuc * 0.88);
    col += E * exp(-abs(ln - rN) / 0.010) * 0.22 * (1.0 - ecto);
    // contractile vacuole: fills over ~5.5 s, then pulses empty
    float cph = fract(uT / 5.5);
    float crad = 0.12 * uR0 * smoothstep(0.0, 0.88, cph) * (1.0 - smoothstep(0.90, 0.97, cph));
    vec2 V = uC + dirv(-2.2 + 0.05 * sin(uT * 0.1)) * 0.60 * uR0;
    float lv = length(p - V);
    col = mix(col, vec3(0.30, 0.33, 0.37), (1.0 - smoothstep(crad - 0.003, crad + 0.003, lv)) * 0.75);
    col += vec3(0.9, 0.88, 0.85) * exp(-abs(lv - crad) / 0.005) * step(0.001, crad) * 0.55;
    // two food vacuoles drifting with the streaming
    for (int i = 0; i < 2; i++) {
      float fi = float(i);
      vec2 F = uC + dirv(uT * 0.05 * (1.0 + fi * 0.4) + fi * 2.4 + 0.4 * sin(uT * 0.23 + fi)) * (0.36 + 0.06 * fi) * uR0;
      float lf = length(p - F), rf = 0.10 * uR0;
      col = mix(col, vec3(0.24, 0.12, 0.03), (1.0 - smoothstep(rf - 0.004, rf + 0.004, lf)) * 0.85);
      col += vec3(1.0, 0.7, 0.3) * exp(-abs(lf - rf) / 0.006) * 0.25;
    }
    // target vacuole: the part sits inside, digestion glows and a scan ripple runs through it
    vec2 tq = p - uTgt;
    float lt = length(tq), Rv = uTgtR * 1.30;
    float tv = (1.0 - smoothstep(Rv - 0.004, Rv + 0.004, lt)) * uEng * uTgtOn;
    float rr = fract(uT * 0.35);
    float ripple = exp(-pow((lt / Rv - rr) * 9.0, 2.0)) * uDig;
    vec3 glow = vec3(1.0, 0.62, 0.22);
    col = mix(col, glow * (0.30 + 0.55 * uDig) * (0.7 + 0.8 * ripple) + vec3(0.12, 0.05, 0.01), tv * (0.55 - 0.15 * uDig));
    col += glow * exp(-abs(lt - Rv) / 0.008) * uEng * uTgtOn * (0.6 + 0.9 * uDig);
    // hold: the body tints amber
    col = mix(col, col * vec3(1.0, 0.70, 0.35) + vec3(0.10, 0.04, 0.0), uHold * 0.8);
    // membrane: fresnel rim and a thin bright line
    float fres = pow(1.0 - dome, 2.6);
    col += mix(vec3(1.0, 0.9, 0.8), E, 0.35) * fres * 0.42;
    col += vec3(1.0, 0.86, 0.72) * exp(-depth / (0.012 * uR0)) * 0.28;
    float alpha = mix(0.62, 0.90, 1.0 - ecto) * mix(1.0, 0.45, tv);
    outc = mix(outc, col, alpha * cover);
  }
  // expelled droplet (release)
  vec2 tt = uTgt - uC;
  vec2 DP = uC + dirv(atan(tt.y, tt.x)) * (uR0 * 0.92 + uDrop * uR0 * 0.55);
  float dd = length(p - DP) - (0.045 + 0.02 * uDrop) * uR0;
  float dcov = clamp(0.5 - dd * 0.5 * uRes.y, 0.0, 1.0) * smoothstep(0.0, 0.2, uDrop);
  vec3 dcol = E * 0.9 + vec3(1.0, 0.85, 0.6) * exp(-max(-dd, 0.0) / (0.02 * uR0)) * 0.35;
  outc = mix(outc, dcol, dcov);
  gl_FragColor = vec4(outc, 1.0);
}`;

const FS_BRIGHT = `precision highp float;
varying vec2 vUv;
uniform sampler2D tSrc;
uniform vec2 uTexel;
void main() {
  vec3 c = 0.25 * (texture2D(tSrc, vUv + uTexel * vec2(-0.5, -0.5)).rgb + texture2D(tSrc, vUv + uTexel * vec2(0.5, -0.5)).rgb
    + texture2D(tSrc, vUv + uTexel * vec2(-0.5, 0.5)).rgb + texture2D(tSrc, vUv + uTexel * vec2(0.5, 0.5)).rgb);
  float l = max(max(c.r, c.g), c.b);
  gl_FragColor = vec4(c * smoothstep(0.30, 0.85, l), 1.0);
}`;

const FS_BLUR = `precision highp float;
varying vec2 vUv;
uniform sampler2D tSrc;
uniform vec2 uDir;
void main() {
  vec3 c = texture2D(tSrc, vUv).rgb * 0.227027;
  c += (texture2D(tSrc, vUv + uDir).rgb + texture2D(tSrc, vUv - uDir).rgb) * 0.1945946;
  c += (texture2D(tSrc, vUv + uDir * 2.0).rgb + texture2D(tSrc, vUv - uDir * 2.0).rgb) * 0.1216216;
  c += (texture2D(tSrc, vUv + uDir * 3.0).rgb + texture2D(tSrc, vUv - uDir * 3.0).rgb) * 0.054054;
  c += (texture2D(tSrc, vUv + uDir * 4.0).rgb + texture2D(tSrc, vUv - uDir * 4.0).rgb) * 0.016216;
  gl_FragColor = vec4(c, 1.0);
}`;

const FS_COMPOSITE = `precision highp float;
varying vec2 vUv;
uniform sampler2D tSrc, tBloom;
uniform float uBloom;
void main() {
  gl_FragColor = vec4(texture2D(tSrc, vUv).rgb + texture2D(tBloom, vUv).rgb * uBloom, 1.0);
}`;

function autoQuality(gl) {
  const dbg = gl.getExtension('WEBGL_debug_renderer_info');
  const name = String(dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : '');
  if (/swiftshader|llvmpipe|software/i.test(name)) return 'low';
  if (navigator.deviceMemory && navigator.deviceMemory <= 4) return 'low';
  if (/nvidia|geforce|rtx|radeon rx/i.test(name)) return 'high';
  return 'med';
}

function staticBlob(container, canvas) {
  const blob = document.createElement('div');
  blob.className = 'amoeba-static';
  blob.setAttribute('aria-hidden', 'true');
  blob.style.cssText = 'position:absolute;left:50%;top:50%;width:46%;height:46%;transform:translate(-50%,-50%);'
    + 'border-radius:52% 48% 55% 45% / 48% 56% 44% 52%;'
    + 'background:radial-gradient(circle at 45% 42%,rgba(255,106,26,.55),rgba(255,106,26,.18) 55%,rgba(255,255,255,.06) 70%,rgba(20,22,26,0) 72%);'
    + 'box-shadow:0 0 40px rgba(255,106,26,.25),inset 0 0 30px rgba(255,106,26,.25);';
  if (canvas && canvas.parentNode === container) container.replaceChild(blob, canvas);
  else container.appendChild(blob);
  let stage = 'idle';
  return {
    mode: 'static',
    setStage(s) { if (STAGES[s]) { stage = s; blob.dataset.stage = s; } },
    setTarget() {},
    setQuality() {},
    stats() { return { mode: 'static', quality: 'none', fps: 0, stage }; },
    dispose() { blob.remove(); },
  };
}

export function createAmoeba(host, opts = {}) {
  const isCanvas = host && host.tagName === 'CANVAS';
  const container = isCanvas ? host.parentElement : host;
  const canvas = isCanvas ? host : document.createElement('canvas');
  const created = !isCanvas;
  if (created) {
    canvas.className = 'amoeba-canvas';
    canvas.setAttribute('aria-hidden', 'true');
    canvas.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;display:block;pointer-events:none';
    container.appendChild(canvas);
    if (getComputedStyle(container).position === 'static') container.style.position = 'relative';
  }
  const reduce = !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);

  let gl = null;
  try { gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: false, stencil: false }); } catch (e) { gl = null; }
  if (!gl) return staticBlob(container, created ? canvas : null);

  let quality = opts.quality && opts.quality !== 'auto' ? opts.quality : autoQuality(gl);
  if (!QUALITY[quality]) quality = 'med';

  const scene = opts.scene || new THREE.Scene();
  const camera = opts.camera || new THREE.PerspectiveCamera(30, 1, 0.1, 100);
  const renderer = new THREE.WebGLRenderer({ canvas, context: gl, antialias: false, alpha: false, powerPreference: 'default' });
  renderer.autoClear = true;
  renderer.setClearColor(new THREE.Color().setRGB(BG[0], BG[1], BG[2], THREE.LinearSRGBColorSpace), 1);

  const U = () => ({
    tScene: { value: null }, uRes: { value: new THREE.Vector2(1, 1) }, uT: { value: 0 }, uR0: { value: R0 },
    uNL: { value: 6 }, uTgtOn: { value: 0 }, uTgtR: { value: 0.1 }, uSur: { value: 0 }, uEng: { value: 0 },
    uDig: { value: 0 }, uRel: { value: 0 }, uHold: { value: 0 }, uDrop: { value: 0 },
    uC: { value: new THREE.Vector2() }, uTgt: { value: new THREE.Vector2(0.5, 0) }, uCrawl: { value: new THREE.Vector2(1, 0) },
  });
  const quadScene = new THREE.Scene();
  const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  const quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2));
  quad.frustumCulled = false;
  quadScene.add(quad);
  const mk = (fs, uniforms) => new THREE.ShaderMaterial({ vertexShader: VS, fragmentShader: fs, uniforms, depthTest: false, depthWrite: false });
  const orgU = U();
  const orgMat = mk(FS_ORGANISM, orgU);
  const brightU = { tSrc: { value: null }, uTexel: { value: new THREE.Vector2() } };
  const brightMat = mk(FS_BRIGHT, brightU);
  const blurU = { tSrc: { value: null }, uDir: { value: new THREE.Vector2() } };
  const blurMat = mk(FS_BLUR, blurU);
  const compU = { tSrc: { value: null }, tBloom: { value: null }, uBloom: { value: 0.85 } };
  const compMat = mk(FS_COMPOSITE, compU);

  let sceneRT = null, outRT = null, bA = null, bB = null;
  let W = 1, H = 1, bw = 1, bh = 1;
  const freeRT = () => { [sceneRT, outRT, bA, bB].forEach((rt) => rt && rt.dispose()); sceneRT = outRT = bA = bB = null; };
  function makeRTs() {
    freeRT();
    const Q = QUALITY[quality];
    sceneRT = new THREE.WebGLRenderTarget(bw, bh, { samples: Q.msaa });
    sceneRT.texture.colorSpace = THREE.SRGBColorSpace;
    outRT = new THREE.WebGLRenderTarget(bw, bh, { depthBuffer: false });
    if (Q.bloom) {
      const sw = Math.max(1, Math.round(bw / Q.bdiv)), sh = Math.max(1, Math.round(bh / Q.bdiv));
      bA = new THREE.WebGLRenderTarget(sw, sh, { depthBuffer: false });
      bB = new THREE.WebGLRenderTarget(sw, sh, { depthBuffer: false });
    }
  }

  // stage tween state
  const cur = { app: 0, sur: 0, eng: 0, dig: 0, rel: 0, hold: 0, drop: 0, tgt: 0 };
  let goal = { ...STAGES.idle };
  let stageName = 'idle';
  let target = null;          // { obj } | { rect } | null
  let time = reduce ? 4.0 : 0;
  let last = performance.now();
  let raf = 0, running = false, visible = true, disposed = false;
  let frames = 0, fpsAt = performance.now(), fps = 0;

  function resize() {
    const Q = QUALITY[quality];
    const cssW = Math.max(1, container.clientWidth || canvas.clientWidth || 1);
    const cssH = Math.max(1, container.clientHeight || canvas.clientHeight || 1);
    W = cssW; H = cssH;
    const pr = Math.min(window.devicePixelRatio || 1, Q.pr);
    renderer.setPixelRatio(pr);
    renderer.setSize(cssW, cssH, false);
    bw = canvas.width; bh = canvas.height;
    makeRTs();
    if (camera.isPerspectiveCamera) { camera.aspect = cssW / cssH; camera.updateProjectionMatrix(); }
    orgU.uRes.value.set(bw, bh);
    draw();
  }

  // target in canvas CSS pixels -> { x, y, r }
  function targetPx() {
    if (!target) return null;
    if (target.obj) {
      const obj = target.obj;
      obj.updateMatrixWorld(true);
      const sphere = new THREE.Sphere();
      new THREE.Box3().setFromObject(obj).getBoundingSphere(sphere);
      const c = sphere.center.clone().project(camera);
      const x = (c.x + 1) / 2 * W, y = (1 - c.y) / 2 * H;
      const dist = camera.position.distanceTo(sphere.center);
      const fov = (camera.fov || 30) * Math.PI / 180;
      const pxPerUnit = (H / 2) / (Math.tan(fov / 2) * Math.max(dist, 1e-3));
      return { x, y, r: sphere.radius * pxPerUnit };
    }
    const rc = target.rect;
    const cr = canvas.getBoundingClientRect();
    if (rc.left !== undefined) return { x: rc.left - cr.left + rc.width / 2, y: rc.top - cr.top + rc.height / 2, r: Math.max(rc.width, rc.height) / 2 };
    return { x: rc.x + rc.w / 2, y: rc.y + rc.h / 2, r: Math.max(rc.w, rc.h) / 2 };
  }

  function wanderAt(t) {
    const aspect = W / H;
    return [
      -0.30 * aspect + 0.10 * Math.sin(0.05 * t) + 0.06 * Math.sin(0.13 * t + 1.0),
      0.07 * Math.cos(0.04 * t) + 0.05 * Math.sin(0.11 * t),
    ];
  }

  function draw() {
    if (disposed) return;
    const h2 = H / 2;
    const tp = targetPx();
    const tg = cur.tgt;
    let T = [wanderAt(time)[0] + 0.5, wanderAt(time)[1]], Rt = 0.1;
    if (tp) {
      T = [(tp.x - W / 2) / h2, (h2 - tp.y) / h2];
      Rt = Math.max(0.04, tp.r / h2);
    }
    const w0 = wanderAt(time), w1 = wanderAt(time + 0.3);
    let cx = w1[0] - w0[0], cy = w1[1] - w0[1];
    const cl = Math.hypot(cx, cy) || 1;
    cx /= cl; cy /= cl;
    const home = wanderAt(time);
    // approach point: the body edge (surround) or the body centre (engulf), on the home side of the part
    let dx = home[0] - T[0], dy = home[1] - T[1];
    const dl = Math.hypot(dx, dy) || 1;
    dx /= dl; dy /= dl;
    const dist = (R0 * 0.9 + Rt * 1.2) * (1 - cur.eng) + R0 * 0.55 * cur.eng;
    const app = cur.app * tg;
    const Cx = home[0] + (T[0] + dx * dist - home[0]) * app;
    const Cy = home[1] + (T[1] + dy * dist - home[1]) * app;
    orgU.uT.value = time;
    orgU.uC.value.set(Cx, Cy);
    orgU.uTgt.value.set(tp ? T[0] : Cx + 0.3, tp ? T[1] : Cy);
    orgU.uTgtR.value = Rt;
    orgU.uTgtOn.value = tg;
    orgU.uCrawl.value.set(cx, cy);
    orgU.uNL.value = QUALITY[quality].lobes;
    orgU.uSur.value = cur.sur;
    orgU.uEng.value = cur.eng;
    orgU.uDig.value = cur.dig;
    orgU.uRel.value = cur.rel;
    orgU.uHold.value = cur.hold;
    orgU.uDrop.value = cur.drop;

    // 1. the part (scene) into a render target
    renderer.setRenderTarget(sceneRT);
    renderer.render(scene, camera);
    // 2. the organism over the part
    orgU.tScene.value = sceneRT.texture;
    quad.material = orgMat;
    renderer.setRenderTarget(outRT);
    renderer.render(quadScene, ortho);
    let bloomK = 0;
    // 3. bloom: bright pass, separable blur at reduced resolution
    if (bA) {
      brightU.tSrc.value = outRT.texture;
      brightU.uTexel.value.set(1 / bw, 1 / bh);
      quad.material = brightMat;
      renderer.setRenderTarget(bA);
      renderer.render(quadScene, ortho);
      blurU.tSrc.value = bA.texture;
      blurU.uDir.value.set(1.5 / bA.width, 0);
      quad.material = blurMat;
      renderer.setRenderTarget(bB);
      renderer.render(quadScene, ortho);
      blurU.tSrc.value = bB.texture;
      blurU.uDir.value.set(0, 1.5 / bA.height);
      renderer.setRenderTarget(bA);
      renderer.render(quadScene, ortho);
      bloomK = 0.85;
    }
    // 4. composite to the canvas
    compU.tSrc.value = outRT.texture;
    compU.tBloom.value = bA ? bA.texture : outRT.texture;
    compU.uBloom.value = bloomK;
    quad.material = compMat;
    renderer.setRenderTarget(null);
    renderer.render(quadScene, ortho);
  }

  function tween(dt) {
    const k = reduce ? 1 : 1 - Math.exp(-dt * RATE);
    for (const key of Object.keys(goal)) cur[key] += (goal[key] - cur[key]) * k;
    cur.tgt += ((target ? 1 : 0) - cur.tgt) * k;
  }

  function frame(now) {
    if (!running) return;
    raf = requestAnimationFrame(frame);
    const dt = Math.min(0.05, Math.max(0, (now - last) / 1000));
    last = now;
    time += dt;
    tween(dt);
    draw();
    frames++;
    if (now - fpsAt >= 1000) { fps = frames * 1000 / (now - fpsAt); frames = 0; fpsAt = now; }
  }

  function start() {
    if (running || disposed || reduce || !visible || document.hidden) return;
    running = true;
    last = performance.now();
    raf = requestAnimationFrame(frame);
  }
  function stop() { running = false; cancelAnimationFrame(raf); }

  const onVis = () => (document.hidden ? stop() : start());
  document.addEventListener('visibilitychange', onVis);
  const io = typeof IntersectionObserver !== 'undefined'
    ? new IntersectionObserver(([e]) => { visible = !!(e && e.isIntersecting); if (visible) start(); else stop(); })
    : null;
  if (io) io.observe(canvas);
  const ro = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(() => { if (!disposed) resize(); }) : null;
  if (ro) ro.observe(container);

  resize();
  if (reduce) {
    // one gentle static frame; stage changes redraw it
    for (const key of Object.keys(goal)) cur[key] = goal[key];
    cur.tgt = target ? 1 : 0;
    draw();
  } else start();

  return {
    mode: 'webgl',
    setStage(name) {
      if (!STAGES[name]) return;
      stageName = name;
      goal = { ...STAGES[name] };
      if (reduce) { for (const key of Object.keys(goal)) cur[key] = goal[key]; draw(); }
    },
    setTarget(t) {
      if (!t) target = null;
      else if (t.isObject3D) target = { obj: t };
      else target = { rect: t };
      if (reduce) { cur.tgt = target ? 1 : 0; draw(); }
    },
    setQuality(q) {
      if (!QUALITY[q] || q === quality) return;
      quality = q;
      resize();
    },
    stats() { return { mode: 'webgl', quality, fps: Math.round(fps), stage: stageName }; },
    dispose() {
      if (disposed) return;
      stop();
      disposed = true;
      document.removeEventListener('visibilitychange', onVis);
      if (io) io.disconnect();
      if (ro) ro.disconnect();
      freeRT();
      quad.geometry.dispose();
      [orgMat, brightMat, blurMat, compMat].forEach((m) => m.dispose());
      renderer.dispose();
      if (created) { renderer.forceContextLoss(); canvas.remove(); }
    },
  };
}
