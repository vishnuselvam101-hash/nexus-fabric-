/* NEXUS FABRIC — steel part stage (home). A generic steel shell with a branch pipe: it turns in 3D and unrolls
   into its flat blank as the page scrolls. One WebGL canvas per host element, so the part can never leave its
   column and can never cover text outside it. It is an illustration, not the user's drawing (the page says so).
   Off-screen stages stop rendering; reduced motion renders one frame per change. Without WebGL the function
   returns null and the host keeps its CSS fallback. */
import * as THREE from '../../vendor/three/three.module.js';
import { reducedMotion, webglAvailable } from './motion.js';

const R = 1.6;                  // shell radius
const ARC = Math.PI * 1.6;      // shell arc (leaves a seam gap at the back)
const H = 2.6;                  // shell height
const NU = 72;
const NV = 18;
const BR = 0.5;                 // branch radius
const BL = 1.7;                 // branch length
const BNU = 40;
const BNV = 8;
const BRANCH_Y = 0.15;
const W = R * ARC;              // flat width of the shell
const GAP = 0.9;
const SHIFT = (GAP + 2 * Math.PI * BR) / 2;   // centres the flat blank on the origin
export const FLAT = { w: W + GAP + 2 * Math.PI * BR, h: H };

const FOV = 30;
const CAM_Z = 10;
const ACCENT = 0xff6a1a;
const COOL = 0x7fb4d9;

const clamp01 = (x) => Math.min(1, Math.max(0, x));
const seg = (t, a, b) => clamp01((t - a) / (b - a));
const smooth = (x) => x * x * (3 - 2 * x);

// shared surface builder: returns the 3D positions, the flat positions, triangles and the perimeter (cut lines)
function surface(nu, nv, fn) {
  const n = (nu + 1) * (nv + 1);
  const p3 = new Float32Array(n * 3);
  const pf = new Float32Array(n * 3);
  let k = 0;
  for (let j = 0; j <= nv; j++) {
    for (let i = 0; i <= nu; i++) {
      const [a, b] = fn(i / nu, j / nv);
      p3.set(a, k * 3);
      pf.set(b, k * 3);
      k++;
    }
  }
  const idx = [];
  for (let j = 0; j < nv; j++) {
    for (let i = 0; i < nu; i++) {
      const a = j * (nu + 1) + i;
      const b = a + 1;
      const c = a + (nu + 1);
      const d = c + 1;
      idx.push(a, c, b, b, c, d);
    }
  }
  const loop = [];
  for (let i = 0; i <= nu; i++) loop.push(i);
  for (let j = 1; j <= nv; j++) loop.push(j * (nu + 1) + nu);
  for (let i = nu - 1; i >= 0; i--) loop.push(nv * (nu + 1) + i);
  for (let j = nv - 1; j >= 1; j--) loop.push(j * (nu + 1));
  const edge = [];
  for (let q = 0; q < loop.length; q++) edge.push(loop[q], loop[(q + 1) % loop.length]);
  return { p3, pf, idx, edge };
}

function makePiece(data, mats) {
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(data.p3), 3));
  geo.setIndex(data.idx);
  geo.computeVertexNormals();
  const lineGeo = new THREE.BufferGeometry();
  lineGeo.setAttribute('position', geo.attributes.position);   // shares the live positions
  lineGeo.setIndex(data.edge);
  const group = new THREE.Group();
  group.add(new THREE.Mesh(geo, mats.solid), new THREE.LineSegments(lineGeo, mats.edge));
  return { group, geo, data, lastU: -1 };
}

function applyFold(piece, u) {
  if (Math.abs(u - piece.lastU) < 1e-4) return;
  piece.lastU = u;
  const arr = piece.geo.attributes.position.array;
  const { p3, pf } = piece.data;
  for (let k = 0; k < arr.length; k++) arr[k] = p3[k] + (pf[k] - p3[k]) * u;
  piece.geo.attributes.position.needsUpdate = true;
  piece.geo.computeVertexNormals();
}

// Studio reflections from a few emissive panels (no external HDR file).
function studioEnvironment(renderer) {
  const pmrem = new THREE.PMREMGenerator(renderer);
  const env = new THREE.Scene();
  env.background = new THREE.Color(0x0a0b0d);
  const panel = (w, h, color, k, pos) => {
    const m = new THREE.Mesh(new THREE.PlaneGeometry(w, h),
      new THREE.MeshBasicMaterial({ color: new THREE.Color(color).multiplyScalar(k), side: THREE.DoubleSide }));
    m.position.set(...pos);
    m.lookAt(0, 0, 0);
    env.add(m);
  };
  panel(12, 3, 0xffffff, 1.5, [0, 9, 3]);
  panel(10, 6, 0xdfe8f2, 0.4, [0, 2, 10]);
  panel(3, 14, COOL, 1.8, [-10, 0, 2]);
  panel(2, 11, ACCENT, 1.3, [10, -1, 1]);
  const tex = pmrem.fromScene(env, 0.03).texture;
  pmrem.dispose();
  return tex;
}

/* Pose for scroll progress p in [0,1] and time t (seconds). Unrolls between p 0.35 and 0.75; the glow comes last. */
export function steelPose(p, t) {
  const unroll = smooth(seg(p, 0.35, 0.75));
  const free = 1 - unroll;
  return {
    unroll,
    glow: seg(p, 0.8, 0.95),
    rotY: free * (0.55 * Math.sin(t * 0.45) + 0.18 + 2.4 * p),
    rotX: free * 0.22 * Math.sin(t * 0.31),
  };
}

/* createSteelStage(host): returns { set(p), dispose(), onresize }, or null without WebGL. p is the scroll progress. */
export function createSteelStage(host) {
  // without WebGL three.js writes a console error before it throws: decide first, so the page stays clean
  if (!webglAvailable()) return null;
  const canvas = document.createElement('canvas');
  canvas.setAttribute('aria-hidden', 'true');
  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: 'high-performance' });
  } catch {
    return null;
  }
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
  renderer.setClearColor(0x000000, 0);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  host.appendChild(canvas);

  const scene = new THREE.Scene();
  scene.environment = studioEnvironment(renderer);
  scene.environmentIntensity = 0.75;
  const camera = new THREE.PerspectiveCamera(FOV, 1, 0.1, 200);
  camera.position.set(0, 0, CAM_Z);
  scene.add(new THREE.HemisphereLight(0xdfe8f2, 0x0e1013, 0.3));
  const key = new THREE.DirectionalLight(0xffffff, 1.1);
  key.position.set(4, 6, 8);
  scene.add(key);
  const rim = new THREE.DirectionalLight(COOL, 2.0);
  rim.position.set(-6, 2, -4);
  scene.add(rim);
  const warm = new THREE.PointLight(ACCENT, 14, 24, 1.6);
  warm.position.set(-3, -3, 6);
  scene.add(warm);

  const mats = {
    solid: new THREE.MeshStandardMaterial({ color: 0xaab4bf, metalness: 1.0, roughness: 0.28, side: THREE.DoubleSide, envMapIntensity: 1.3 }),
    edge: new THREE.LineBasicMaterial({ color: new THREE.Color(ACCENT).multiplyScalar(1.8), transparent: true, opacity: 0, depthWrite: false }),
  };
  const shell = surface(NU, NV, (u, v) => {
    const th = -ARC / 2 + u * ARC;
    const y = (v - 0.5) * H;
    return [[R * Math.cos(th), y, R * Math.sin(th)], [(u - 0.5) * W - SHIFT, y, 0]];
  });
  const branch = surface(BNU, BNV, (u, v) => {
    const ph = u * Math.PI * 2;
    const z = R * 0.85 + v * BL;
    return [[BR * Math.cos(ph), BRANCH_Y + BR * Math.sin(ph), z], [W / 2 - SHIFT + GAP + u * 2 * Math.PI * BR, (v - 0.5) * BL, 0]];
  });
  const shellPiece = makePiece(shell, mats);
  const branchPiece = makePiece(branch, mats);
  const group = new THREE.Group();
  group.add(shellPiece.group, branchPiece.group);
  scene.add(group);

  const stage = { canvas, p: 0, dirty: true, visible: true, disposed: false, onresize: null };
  let k = 1;             // world units per CSS pixel at z = 0
  let scale3D = 1;
  let scaleFlat = 1;
  let w = 1;
  let h = 1;

  function resize() {
    const rect = host.getBoundingClientRect();
    w = Math.max(1, Math.round(rect.width));
    h = Math.max(1, Math.round(rect.height));
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    k = (2 * CAM_Z * Math.tan(THREE.MathUtils.degToRad(FOV / 2))) / h;
    const worldW = w * k;
    const worldH = h * k;
    // the blank fits 92 % of the width; the 3D part fits the column and keeps clear of the caption
    scaleFlat = Math.min((0.92 * worldW) / FLAT.w, (0.7 * worldH) / FLAT.h);
    scale3D = Math.min((0.6 * worldW) / (2 * R + BL * 0.5), (0.7 * worldH) / H);
    stage.dirty = true;
    stage.layout = { w, h, k, scaleFlat, scale3D };
    if (stage.onresize) stage.onresize(stage.layout);
  }

  let raf = 0;
  const reduce = reducedMotion();
  const t0 = performance.now();
  const kick = () => { if (!raf && !stage.disposed) raf = requestAnimationFrame(frame); };
  function frame() {
    raf = 0;
    if (stage.disposed || !stage.visible || (reduce && !stage.dirty)) return;
    const t = reduce ? 0 : (performance.now() - t0) / 1000;
    const s = steelPose(stage.p, t);
    applyFold(shellPiece, s.unroll);
    applyFold(branchPiece, s.unroll);
    group.scale.setScalar(scale3D + (scaleFlat - scale3D) * s.unroll);
    group.rotation.set(s.rotX, s.rotY, 0);
    mats.edge.opacity = Math.max(s.glow, s.unroll * 0.85);
    renderer.render(scene, camera);
    stage.dirty = false;
    if (!reduce) raf = requestAnimationFrame(frame);
  }

  // Scroll progress drives the pose; the loop runs only while the host is on screen.
  stage.set = (p) => {
    const q = clamp01(p);
    if (Math.abs(q - stage.p) > 1e-4) { stage.p = q; stage.dirty = true; kick(); }
  };
  stage.dispose = () => {
    stage.disposed = true;
    if (raf) cancelAnimationFrame(raf);
    ro.disconnect();
    io.disconnect();
    scene.environment?.dispose?.();
    shellPiece.geo.dispose();
    branchPiece.geo.dispose();
    mats.solid.dispose();
    mats.edge.dispose();
    renderer.dispose();
    renderer.forceContextLoss();
    canvas.remove();
  };

  const ro = new ResizeObserver(() => { resize(); kick(); });
  ro.observe(host);
  const io = new IntersectionObserver((entries) => {
    stage.visible = entries.some((e) => e.isIntersecting);
    if (stage.visible) { stage.dirty = true; kick(); }
  });
  io.observe(host);
  resize();
  kick();
  return stage;
}
