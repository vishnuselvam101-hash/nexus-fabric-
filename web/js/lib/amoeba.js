/* NEXUS FABRIC — AMOEBA organism: a living, Amoeba proteus-like cell drawn in one fragment pass over the
   scene that holds the part it acts on. Own SDF shader and own bloom pass; three.js (vendored, r187) only
   for the scene and the render targets. No CDN.

   API
     const a = createAmoeba(canvasOrContainer, { quality: 'auto' | 'low' | 'med' | 'high', scene, camera })
     a.setStage('idle' | 'surround' | 'engulf' | 'digest' | 'release' | 'hold')
     a.setTarget(object3D | { x, y, w, h } | DOMRect | null)   // object3D is projected with `camera`
     a.setQuality('low' | 'med' | 'high')
     a.stats()    -> { mode, quality, fps, stage, t }          // t = stage clock, seconds
     palette: { body, rim, accent } as '#rrggbb' or [r, g, b] in 0..1; default is a neutral grey-blue body with
     a muted copper accent, used only in digest and hold.
     a.dispose()

   Organism units: half the canvas height, origin at the canvas centre, y up.
   An Object3D target is moved out of the body on 'release' (world offset along the body-to-part direction,
   assumes the object sits directly under the scene root) and restored to its original position when the
   stage returns to idle. Without WebGL2 the same API drives a static CSS blob. With prefers-reduced-motion
   one frame is drawn and nothing animates. The loop stops while the canvas is off-screen or the page is hidden. */
import * as THREE from '../../vendor/three/three.module.js';

const R0 = 0.27;                                   // body radius, organism units
const RATE = 4.5;                                  // stage tween rate (1/s): settles in about 0.7-1.0 s
const OUT = 0.5;                                   // release: world units the part is pushed out of the body
const SPEED = 0.6;                                 // organism motion runs slower than the stage clock
const BG = [0x1b / 255, 0x1d / 255, 0x21 / 255];   // graphite, raw sRGB (the scene clear colour)
const PALETTE = { body: [0.36, 0.42, 0.48], rim: [0.70, 0.76, 0.82], accent: [0.78, 0.52, 0.30] };

function parseColor(c, fb) {
  if (Array.isArray(c) && c.length === 3) return c;
  if (typeof c === 'string' && /^#?[0-9a-f]{6}$/i.test(c)) {
    const h = c.replace('#', '');
    return [0, 2, 4].map((i) => parseInt(h.substr(i, 2), 16) / 255);
  }
  return fb;
}
function resolvePalette(p) {
  p = p || {};
  return {
    body: parseColor(p.body, PALETTE.body),
    rim: parseColor(p.rim, PALETTE.rim),
    accent: parseColor(p.accent, PALETTE.accent),
  };
}

const STAGES = {
  idle:     { app: 0, sur: 0,   eng: 0, dig: 0,   rel: 0, hold: 0,   drop: 0 },
  surround: { app: 1, sur: 1,   eng: 0, dig: 0,   rel: 0, hold: 0,   drop: 0 },
  engulf:   { app: 1, sur: 0,   eng: 1, dig: 0,   rel: 0, hold: 0,   drop: 0 },
  digest:   { app: 1, sur: 0,   eng: 1, dig: 1,   rel: 0, hold: 0,   drop: 0 },
  release:  { app: 1, sur: 0,   eng: 0, dig: 0,   rel: 1, hold: 0,   drop: 1 },
  hold:     { app: 1, sur: 0.15, eng: 1, dig: 0.0, rel: 0, hold: 1,   drop: 0 },
};

// lobes: active pseudopods (2-4); the leader is always lobe 0
const QUALITY = {
  low:  { pr: 1,    lobes: 3, bloom: 0, msaa: 0, bdiv: 4 },
  med:  { pr: 1.25, lobes: 4, bloom: 1, msaa: 4, bdiv: 4 },
  high: { pr: 2,    lobes: 4, bloom: 1, msaa: 4, bdiv: 2 },
};

const VS = `varying vec2 vUv;
void main() { vUv = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }`;

const FS_ORGANISM = `precision highp float;
varying vec2 vUv;
uniform sampler2D tScene;
uniform vec2 uRes;
uniform vec2 uC, uTgt, uCrawl;
uniform float uT, uR0, uNL, uTgtOn, uTgtR, uSur, uEng, uDig, uRel, uHold, uDrop;
uniform vec3 uBody, uRim, uAcc;
const float PI = 3.14159265;

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
// body radius along angle th; rear (uroid) is narrower and wrinkled
float bodyR(float th, float rear) {
  float R = uR0 * (1.0 + 0.04 * uHold * sin(uT * 1.1));
  float r = R * (1.0 + 0.08 * sin(2.0 * th + uT * 0.45) + 0.05 * sin(3.0 * th - uT * 0.31 + 1.3) + 0.03 * sin(5.0 * th + uT * 0.83));
  r *= 1.0 - 0.22 * rear;
  r += uR0 * 0.035 * rear * sin(9.0 * th + uT * 0.6 + 0.8 * sin(uT * 0.4));
  return r * (1.0 + 0.12 * (vn(dirv(th) * 1.4 + vec2(uT * 0.10, 3.7)) - 0.5));
}
float rearW(float th, float fa) { return smoothstep(0.1, -0.9, cos(th - fa)); }

// membrane + broad blunt lobopodia + part cup; negative inside. hy = hyaline cap weight at the leader tip
float field(vec2 p, out float hy) {
  vec2 q = p - uC;
  float th = atan(q.y, q.x);
  float fa = atan(uCrawl.y, uCrawl.x);
  vec2 tq = uTgt - uC;
  float phi0 = atan(tq.y, tq.x);
  float ang = angd(th, phi0);
  float bulge = (0.20 * uEng - 0.42 * uRel) * uR0 * exp(-ang * ang / 0.22) * uTgtOn;
  float d = length(q) - (bodyR(th, rearW(th, fa)) + bulge);
  float E = clamp(uSur + uEng, 0.0, 1.0) * uTgtOn;   // crown (surround) / cup (engulf) mode
  float Rw = uTgtR * 1.35;
  hy = 0.0;
  for (int k = 0; k < 4; k++) {
    float fk = float(k);
    float on = step(fk + 0.5, uNL);
    float off = fk < 0.5 ? 0.0 : (fk < 1.5 ? 1.05 : (fk < 2.5 ? -1.05 : 2.35));
    float a = fa + off + 0.22 * sin(uT * 0.23 + fk * 2.1);
    vec2 dv = dirv(a);
    float ph = fract(uT * (0.07 + 0.012 * fk) + fk * 0.27);
    float env = smoothstep(0.0, 0.45, ph) * (1.0 - smoothstep(0.55, 0.98, ph));
    float lead = fk < 0.5 ? 1.0 : 0.0;
    float Ls = lead > 0.5 ? (0.30 + 0.62 * env) : (0.14 + 0.30 * env);
    if (fk > 2.5) Ls *= 0.6;
    float L = uR0 * Ls;
    float Rm = bodyR(a, rearW(a, fa));
    vec2 base = uC + dv * Rm * 0.6;
    vec2 tipN = uC + dv * (Rm + L);
    // arms: lobes 1 and 2 reach round the part; the leader joins from the far side. s = 1 - engulf progress
    float s = 1.0 - uEng;
    float sg = fk < 0.5 ? 0.0 : (fk < 1.5 ? 1.0 : -1.0);
    float psi = phi0 + PI * s + sg * 0.85 * s;
    vec2 tipW = uTgt + dirv(psi) * (Rw + 0.015 * sin(uT * 1.3 + fk * 2.0));
    float m = E * (fk > 2.5 ? 0.0 : 1.0);
    vec2 tip = mix(tipN, tipW, m);
    vec2 v = tip - base;
    float len = max(length(v), 1e-4);
    vec2 perp = vec2(-v.y, v.x) / len;
    vec2 mid = 0.5 * (base + tip) + perp * (0.10 * len * sin(uT * 0.9 + fk * 1.7));
    // broad and blunt: tip radius about 0.23 of length (width about 0.46 L), wide base merging into the body
    float rt = max(0.23 * len, 0.05 * uR0);
    float rm = max(0.27 * len, 0.06 * uR0);
    float rb = 0.34 * len + 0.08 * uR0;
    float dk = min(capT(p, base, mid, rb, rm), capT(p, mid, tip, rm, rt));
    d = smin(d, mix(1.0, dk, on), 0.22 * uR0);
    hy = max(hy, lead * on * (1.0 - m) * (1.0 - smoothstep(0.5 * rt, 2.2 * rt, length(p - tip))));
  }
  return d + (vn(p * 9.0 + vec2(uT * 0.3, -uT * 0.2)) - 0.5) * 0.010 * uR0;
}
vec3 vac(vec3 col, vec2 p, vec2 V, float rr, float z) {
  float lf = length(p - V);
  float soft = 0.004 + 0.002 * z;
  float edge = 1.0 - smoothstep(rr - soft, rr + soft, lf);
  float k = 0.80 + 0.20 * z;                       // nearer vacuoles brighter
  col = mix(col, uBody * 0.30 * k, edge * 0.85);
  col += uRim * exp(-abs(lf - rr) / 0.006) * 0.10 * k;
  return col;
}
void main() {
  vec2 p = (gl_FragCoord.xy - 0.5 * uRes) / (0.5 * uRes.y);
  vec3 under = texture2D(tScene, vUv).rgb;
  vec3 E = uAcc;                                        // accent: only in digest and hold
  float acc = clamp(uDig + uHold, 0.0, 1.0);
  float hy;
  float d = field(p, hy);
  float cover = clamp(0.5 - d * 0.5 * uRes.y, 0.0, 1.0);
  vec3 outc = under + uRim * exp(-max(d, 0.0) * 22.0 / uR0) * 0.025;
  vec2 fw = uCrawl, sd = vec2(-uCrawl.y, uCrawl.x);
  if (cover > 0.0) {
    float depth = max(-d, 0.0);
    float dome = sqrt(clamp(depth / (0.16 * uR0), 0.0, 1.0));
    float ecto = 1.0 - smoothstep(0.02 * uR0, 0.07 * uR0, depth);
    // refraction: the body bends what lies under it (the part stays visible through the cytoplasm)
    float e = 0.006, dummy;
    vec2 grad = vec2(field(p + vec2(e, 0.0), dummy) - d, field(p + vec2(0.0, e), dummy) - d) / e;
    vec2 refrP = -grad * 0.07 * uR0 * (0.4 + 0.6 * dome);
    vec2 uvOff = refrP * vec2(0.5 * uRes.y) / uRes;
    vec3 seen = texture2D(tScene, vUv + uvOff).rgb;
    float cloud = 0.6 * vn(p * 3.2 + vec2(uT * 0.05, -uT * 0.03)) + 0.4 * vn(p * 7.0 - vec2(0.0, uT * 0.08));
    // granules: small, dense, brownish-grey, a few bright; they stream toward the leading pseudopod
    vec2 gq = p / 0.016 - fw * uT * 0.9;
    vec2 gc = floor(gq), gf = fract(gq) - 0.5;
    float gh = h21(gc + 0.5), gh2 = h21(gc + 3.1);
    vec2 gp = (vec2(h21(gc + 7.3), h21(gc + 11.7)) - 0.5) * 0.35;
    float gsz = 0.10 + 0.14 * h21(gc + 19.3);
    float gran = step(0.42, gh) * smoothstep(gsz + 0.04, gsz, length(gf - gp));
    vec3 gcol = mix(vec3(0.42, 0.40, 0.37), vec3(0.80, 0.78, 0.72), step(0.93, gh2));
    float vol = smoothstep(0.02, 0.24, depth / uR0);     // endoplasm: dimmer at the membrane, full inside
    vec3 col = uBody * (0.62 + 0.18 * cloud) * (0.85 + 0.20 * dome);
    col = mix(col, uAcc * (0.45 + 0.35 * cloud), acc * 0.30 * vol * (1.0 - ecto));
    col += gcol * gran * (0.30 + 0.35 * cloud) * vol * (1.0 - ecto);
    col += uRim * 0.30 * ecto;                           // glassy ectoplasm sheen
    col = mix(col, uRim, hy * 0.35);    // hyaline cap of the leading pseudopod
    // nucleus: off-centre, towards the trailing end
    vec2 N = uC - fw * 0.20 * uR0 + sd * 0.04 * uR0;
    float rN = 0.17 * uR0, ln = length(p - N);
    float nuc = 1.0 - smoothstep(rN - 0.008, rN + 0.004, ln);
    col = mix(col, uBody * (0.40 + 0.10 * cloud), nuc * 0.85);
    col += uRim * exp(-abs(ln - rN) / 0.008) * 0.08 * (1.0 - ecto);
    // contractile vacuole: fills over ~5.5 s, then pulses empty
    float cph = fract(uT / 5.5);
    float crad = 0.085 * uR0 * smoothstep(0.0, 0.88, cph) * (1.0 - smoothstep(0.90, 0.97, cph));
    vec2 V = uC + (fw * 0.10 + sd * 0.46) * uR0;
    float lv = length(p - V);
    col = mix(col, uRim * 0.55, (1.0 - smoothstep(crad - 0.003, crad + 0.003, lv)) * 0.6);
    col += uRim * exp(-abs(lv - crad) / 0.005) * step(0.001, crad) * 0.25;
    // two food vacuoles, uneven: different size, depth and place, not in a row
    col = vac(col, p, uC + (fw * 0.36 - sd * 0.14) * uR0, 0.085 * uR0, 0.6);
    col = vac(col, p, uC + (fw * -0.34 - sd * 0.44) * uR0, 0.13 * uR0, -0.5);
    // part: a faint amber glow in the digestion area, a scan ripple in digest
    vec2 tq = p - uTgt;
    float lt = length(tq), Rv = uTgtR * 1.30;
    float inside = (1.0 - smoothstep(Rv - 0.01, Rv + 0.02, lt)) * uEng * uTgtOn;
    float rr = fract(uT * 0.35);
    float ripple = exp(-pow((lt / Rv - rr) * 9.0, 2.0)) * uDig;
    col = mix(col, uAcc * (0.5 + 0.8 * ripple), inside * 0.20 * uDig);
    col += uAcc * exp(-abs(lt - Rv) / 0.008) * uEng * uTgtOn * uDig * 0.35;
    // hold: the body tints amber
    col = mix(col, col * (0.6 + 0.6 * uAcc) + uAcc * 0.04, uHold * 0.6);
    // membrane: fresnel rim and a thin bright line
    float fres = pow(1.0 - dome, 2.6);
    col += uRim * fres * 0.30;
    col += uRim * exp(-depth / (0.012 * uR0)) * 0.20;
    // glass highlight on the upper left
    vec2 hl = p - (uC + vec2(-0.42, 0.42) * uR0);
    col += uRim * exp(-dot(hl, hl) / (0.012 * uR0 * uR0)) * 0.08 * (1.0 - ecto);
    // translucent body: the seen scene shows through, tinted; the cytoplasm is more opaque at the rim
    float overT = clamp(uEng * uTgtOn * (1.0 - smoothstep(uTgtR * 0.9, uTgtR * 1.4, lt)), 0.0, 1.0);
    vec3 through = seen * vec3(0.90, 0.92, 0.94) + uBody * 0.06 * (1.0 - ecto) + uAcc * 0.03 * acc * (1.0 - ecto);
    float aBody = 0.50 + 0.30 * ecto + 0.15 * hy;
    aBody = mix(aBody, 0.20, overT);
    outc = mix(outc, mix(through, col, aBody), cover);
  }
  // expelled droplet (release): the mouth opens toward the part
  vec2 tt = uTgt - uC;
  vec2 DP = uC + dirv(atan(tt.y, tt.x)) * (uR0 * 0.92 + uDrop * uR0 * 0.55);
  float dd = length(p - DP) - (0.045 + 0.02 * uDrop) * uR0;
  float dcov = clamp(0.5 - dd * 0.5 * uRes.y, 0.0, 1.0) * smoothstep(0.0, 0.2, uDrop);
  vec3 dcol = uAcc * 0.9 + uRim * exp(-max(-dd, 0.0) / (0.02 * uR0)) * 0.35;
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
  gl_FragColor = vec4(c * smoothstep(0.55, 0.95, l), 1.0);
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
    + 'background:radial-gradient(circle at 45% 42%,rgba(92,107,122,.45),rgba(70,82,94,.22) 55%,rgba(255,255,255,.05) 70%,rgba(20,22,26,0) 72%);'
    + 'box-shadow:0 0 40px rgba(120,140,160,.10),inset 0 0 30px rgba(120,140,160,.12);';
  if (canvas && canvas.parentNode === container) container.replaceChild(blob, canvas);
  else container.appendChild(blob);
  let stage = 'idle';
  return {
    mode: 'static',
    setStage(s) { if (STAGES[s]) { stage = s; blob.dataset.stage = s; } },
    setTarget() {},
    setQuality() {},
    stats() { return { mode: 'static', quality: 'none', fps: 0, stage, t: 0 }; },
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

  const PAL = resolvePalette(opts.palette);
  const scene = opts.scene || new THREE.Scene();
  const camera = opts.camera || new THREE.PerspectiveCamera(30, 1, 0.1, 100);
  const renderer = new THREE.WebGLRenderer({ canvas, context: gl, antialias: false, alpha: false, powerPreference: 'default' });
  renderer.autoClear = true;
  renderer.setClearColor(new THREE.Color().setRGB(BG[0], BG[1], BG[2], THREE.LinearSRGBColorSpace), 1);

  const U = () => ({
    tScene: { value: null }, uRes: { value: new THREE.Vector2(1, 1) }, uT: { value: 0 }, uR0: { value: R0 },
    uNL: { value: 4 }, uTgtOn: { value: 0 }, uTgtR: { value: 0.1 }, uSur: { value: 0 }, uEng: { value: 0 },
    uDig: { value: 0 }, uRel: { value: 0 }, uHold: { value: 0 }, uDrop: { value: 0 },
    uBody: { value: new THREE.Vector3(...PAL.body) }, uRim: { value: new THREE.Vector3(...PAL.rim) },
    uAcc: { value: new THREE.Vector3(...PAL.accent) },
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
  const compU = { tSrc: { value: null }, tBloom: { value: null }, uBloom: { value: 0.3 } };
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
  let target = null;          // { obj, base } | { rect } | null
  let releaseDir = null;      // world direction the part is pushed along on release (Object3D targets)
  let lastC = [0, 0];         // organism centre of the last drawn frame
  let time = reduce ? 4.0 : 0;   // organism clock (shader and wander)
  let clk = 0;                   // stage clock (tweens, stats)
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

  // world point under canvas pixel (sx, sy) on the plane z = depthZ
  function worldAtPx(sx, sy, depthZ) {
    const ndc = new THREE.Vector3(sx / W * 2 - 1, 1 - sy / H * 2, 0.5).unproject(camera);
    const dir = ndc.sub(camera.position).normalize();
    return camera.position.clone().addScaledVector(dir, (depthZ - camera.position.z) / dir.z);
  }

  // release: direction from the body centre to the part, in world space (Object3D targets only)
  function prepRelease() {
    releaseDir = null;
    if (!target || !target.obj) return;
    const o = target.obj;
    o.updateMatrixWorld(true);
    const pw = new THREE.Vector3();
    o.getWorldPosition(pw);
    const bodyW = worldAtPx(W / 2 + lastC[0] * H / 2, H / 2 - lastC[1] * H / 2, pw.z);
    releaseDir = pw.sub(bodyW);
    releaseDir.z = 0;
    if (releaseDir.lengthSq() < 1e-8) releaseDir = null; else releaseDir.normalize();
  }

  // the part follows the release stage out of the body and returns to its base position when idle
  function applyPart() {
    if (!target || !target.obj || !target.base) return;
    if (goal.rel === 0 && cur.rel < 1e-3) cur.rel = 0;
    const k = Math.max(0, cur.rel) * OUT;
    const o = target.obj;
    o.position.copy(target.base);
    if (releaseDir && k > 0) o.position.addScaledVector(releaseDir, k);
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
    applyPart();
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
    lastC = [Cx, Cy];
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
      bloomK = 0.3;
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
    time += dt * SPEED;
    clk += dt;
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
      if (name === 'release') prepRelease();
      if (reduce) { for (const key of Object.keys(goal)) cur[key] = goal[key]; draw(); }
    },
    setTarget(t) {
      if (target && target.obj && target.base) target.obj.position.copy(target.base);
      if (!t) target = null;
      else if (t.isObject3D) target = { obj: t, base: t.position.clone() };
      else target = { rect: t };
      releaseDir = null;
      if (reduce) { cur.tgt = target ? 1 : 0; draw(); }
    },
    setQuality(q) {
      if (!QUALITY[q] || q === quality) return;
      quality = q;
      resize();
    },
    stats() { return { mode: 'webgl', quality, fps: Math.round(fps), stage: stageName, t: +clk.toFixed(3) }; },
    dispose() {
      if (disposed) return;
      stop();
      if (target && target.obj && target.base) target.obj.position.copy(target.base);
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
