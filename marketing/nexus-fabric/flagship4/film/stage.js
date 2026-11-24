// Film 4 stage: the three.js world (bright studio set, brushed steel, key + rim light, soft contact shadow) and the
// parts built from the product's plates. The stage holds no clock: shots.js sets every value for a film time t.
'use strict';
import * as THREE from 'three';
import { RoomEnvironment } from '../lib/RoomEnvironment.js';
import { columnMesh, headerColumns, branchColumns, rectColumns, wrapPoint, conePoint, eccTable, lerp } from './geo.js';

const D = window.NF4, deg = Math.PI / 180;
export const C3 = { blue: 0x0a66ff, red: 0xff3b30, amber: 0xff9f0a, green: 0x1fa463, ink: 0x1d1d1f };

export class Stage {
  constructor(scale) {
    this.W = 1920; this.H = 1080; this.scale = scale;
    const cv = document.createElement('canvas'); cv.width = 1920 * scale; cv.height = 1080 * scale; this.canvas = cv;
    const r = new THREE.WebGLRenderer({ canvas: cv, antialias: true, alpha: true, preserveDrawingBuffer: true, premultipliedAlpha: true, powerPreference: 'high-performance' });
    r.setPixelRatio(1); r.setSize(1920 * scale, 1080 * scale, false);
    r.setClearColor(0x000000, 0);
    r.toneMapping = THREE.NeutralToneMapping; r.toneMappingExposure = 1.0; r.outputColorSpace = THREE.SRGBColorSpace;
    this.r = r;
    const s = new THREE.Scene(); this.scene = s;
    const pm = new THREE.PMREMGenerator(r); const env = pm.fromScene(new RoomEnvironment(), 0.03).texture; s.environment = env; s.environmentIntensity = 0.9; pm.dispose();
    this.cam = new THREE.PerspectiveCamera(24, 16 / 9, 50, 40000);
    // key light (soft studio key, upper left front) and a rim light from behind; the shadow is a contact shadow (below)
    const key = new THREE.DirectionalLight(0xffffff, 2.0); s.add(key, key.target); this.key = key;
    const rim = new THREE.DirectionalLight(0xf2f6ff, 2.8); s.add(rim, rim.target); this.rim = rim;
    const fill = new THREE.HemisphereLight(0xffffff, 0xd9dce2, 0.45); s.add(fill);
    r.shadowMap.enabled = false;
    // contact shadow (soft, per frame): the parts are seen from the floor by an orthographic camera looking up; each
    // fragment writes a darkness that falls off with its height above the floor; the image is blurred twice and laid
    // on the floor as a transparent texture. No shadow map is used.
    const RT = (n) => new THREE.WebGLRenderTarget(n, n, { type: THREE.HalfFloatType, depthBuffer: true });
    this.shRT = RT(512); this.shRT2 = RT(512);
    this.shCam = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    this.shMat = new THREE.ShaderMaterial({ uniforms: { floorY: { value: 0 }, height: { value: 1 } }, side: THREE.DoubleSide,
      vertexShader: 'uniform float floorY; varying float h; void main(){ vec4 w = modelMatrix * vec4(position,1.0); h = w.y - floorY; gl_Position = projectionMatrix * viewMatrix * w; }',
      fragmentShader: 'uniform float height; varying float h; void main(){ float a = clamp(1.0 - h / height, 0.0, 1.0); gl_FragColor = vec4(0.0, 0.0, 0.0, a * a); }' });
    this.blurMat = new THREE.ShaderMaterial({ uniforms: { tex: { value: null }, dir: { value: new THREE.Vector2() } }, depthTest: false, depthWrite: false,
      vertexShader: 'varying vec2 uv0; void main(){ uv0 = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }',
      fragmentShader: 'uniform sampler2D tex; uniform vec2 dir; varying vec2 uv0; void main(){ vec4 c = vec4(0.0); float w[7]; w[0]=0.1964825501511404; w[1]=0.2969069646728344; w[2]=0.09447039785044732; w[3]=0.010381362401148057; float s = 0.0; for (int i = -3; i <= 3; i++){ float k = w[abs(i)] ; c += texture2D(tex, uv0 + dir * float(i) * 1.6) * k; s += k; } gl_FragColor = c / s; }' });
    this.quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), this.blurMat); this.quadScene = new THREE.Scene(); this.quadScene.add(this.quad); this.quadCam = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    const floor = new THREE.Mesh(new THREE.PlaneGeometry(1, 1), new THREE.MeshBasicMaterial({ map: this.shRT.texture, transparent: true, depthWrite: false, toneMapped: false, color: 0x1a2030, opacity: 0.5 }));
    floor.material.onBeforeCompile = (sh) => { sh.fragmentShader = sh.fragmentShader.replace('#include <map_fragment>', 'vec4 sampledDiffuseColor = texture2D( map, vMapUv ); diffuseColor = vec4(diffuseColor.rgb, diffuseColor.a * sampledDiffuseColor.a);'); };
    floor.rotation.x = -Math.PI / 2; floor.renderOrder = -1; s.add(floor); this.floor = floor;
    this.shadow = { cx: 0, cz: 0, size: 2000, floorY: -300, height: 400, opacity: 0.5 };
    // brushed steel: physical material, anisotropic highlight along the rolling direction (uv u), fine brushing in roughness
    const br = document.createElement('canvas'); br.width = 1024; br.height = 64; const bx = br.getContext('2d');
    bx.fillStyle = '#808080'; bx.fillRect(0, 0, 1024, 64);
    let seed = 7; const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);   // fixed-seed generator: same texture every load
    for (let i = 0; i < 2600; i++) { const y = rnd() * 64, l = 60 + rnd() * 500, x = rnd() * 1024, v = 110 + rnd() * 50; bx.fillStyle = `rgba(${v},${v},${v},0.35)`; bx.fillRect(x, y, l, 0.6 + rnd() * 0.8); }
    const brushed = new THREE.CanvasTexture(br); brushed.wrapS = brushed.wrapT = THREE.RepeatWrapping; brushed.colorSpace = THREE.NoColorSpace; brushed.anisotropy = 8;
    this.steel = new THREE.MeshPhysicalMaterial({ color: 0xdfe2e6, metalness: 1.0, roughness: 0.34, roughnessMap: brushed, anisotropy: 0.55, anisotropyRotation: 0, clearcoat: 0.25, clearcoatRoughness: 0.18, side: THREE.DoubleSide });
    // the inner side of a plate (seen where the stored outward normal faces away from the camera) is darkened, so pipe
    // interiors, the opening and any gap read as dark against the bright outer steel
    this.steel.onBeforeCompile = (sh) => {
      sh.fragmentShader = sh.fragmentShader.replace('#include <color_fragment>', `#include <color_fragment>
        float nfInside = step(dot(normalize(vNormal), normalize(vViewPosition)), 0.0);
        diffuseColor.rgb *= mix(1.0, 0.30, nfInside);`);
    };
    this.steelFlat = this.steel;
    // the sectors (cone, eccentric) use the same steel without the brushing texture: its direction does not follow a
    // sector's generators and showed as diagonal streaks
    this.steelPlain = new THREE.MeshPhysicalMaterial({ color: 0xdfe2e6, metalness: 1.0, roughness: 0.3, clearcoat: 0.25, clearcoatRoughness: 0.18, side: THREE.DoubleSide });
    this.steelPlain.onBeforeCompile = this.steel.onBeforeCompile;
    this.amberMat = new THREE.MeshPhysicalMaterial({ color: 0xffb340, metalness: 0.6, roughness: 0.35, side: THREE.DoubleSide, emissive: 0xff9f0a, emissiveIntensity: 0.0 });
    this.parts = {}; this.tubes = {};
    this.root = new THREE.Group(); s.add(this.root);
  }

  // ------------------------------------------------------------------ plate parts
  // A plate part: the product's plate as a structured mesh, wrapped by frame F. map(x, y) -> [u, v] gives along/arc.
  makePlate(name, cols, rows, F, R, map, uvScale = [1 / 400, 1 / 400]) {
    const m = columnMesh(cols, rows), g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.BufferAttribute(new Float32Array(m.nV * 3), 3));
    g.setAttribute('normal', new THREE.BufferAttribute(new Float32Array(m.nV * 3), 3));
    const uv = new Float32Array(m.nV * 2); for (let i = 0; i < m.nV; i++) { const [u, v] = map(m.plate[2 * i], m.plate[2 * i + 1]); uv[2 * i] = u * uvScale[0]; uv[2 * i + 1] = v * uvScale[1]; }
    g.setAttribute('uv', new THREE.BufferAttribute(uv, 2));
    g.setIndex(new THREE.BufferAttribute(m.index, 1));
    const mesh = new THREE.Mesh(g, this.steel); mesh.frustumCulled = false;
    const grp = new THREE.Group(); grp.add(mesh); this.root.add(grp);
    const part = { name, mesh, grp, g, plate: m.plate, nV: m.nV, F, R, map, kappa: -1 };
    part.set = (kappa, Rnew) => {
      if (Rnew !== undefined && Rnew !== part.R) { part.R = Rnew; part.kappa = -1; }
      if (Math.abs(kappa - part.kappa) < 1e-7) return; part.kappa = kappa;
      const P = g.attributes.position.array, N = g.attributes.normal.array;
      for (let i = 0; i < part.nV; i++) { const [u, v] = map(part.plate[2 * i], part.plate[2 * i + 1], part.R); wrapPoint(F, part.R, kappa, u, v, P, N, 3 * i); }
      g.attributes.position.needsUpdate = true; g.attributes.normal.needsUpdate = true; g.computeBoundingSphere();
    };
    // a plate point (x, y) in world space for the current state (callouts, tubes)
    part.at = (x, y, kappa = part.kappa) => { const P = [0, 0, 0], N = [0, 0, 0]; const [u, v] = map(x, y, part.R); wrapPoint(F, part.R, kappa, u, v, P, N, 0); const w = new THREE.Vector3(...P); grp.updateMatrixWorld(); return w.applyMatrix4(grp.matrixWorld); };
    this.parts[name] = part; return part;
  }

  // the tee of job 5 (header + branch), plus job 6's plates for the correction and the wrong-curve hook
  buildTee() {
    const req = D.pat.header5.request;
    const L = req.header_length, W = D.pat.header5.quantities.developed_width, b = req.beta_deg * deg;
    const headerFrame = { O: [0, 0, 0], a: [1, 0, 0], na: [0, 1, 0], ta: [0, 0, 1] };
    // header plate (x along the axis, y around): phi = phi0 + s y / R_dev with phi0 = 180 deg, s = -1 (fit in NF4.fit.header5),
    // i.e. the opening centre y = W/2 at the top (+Y) and the seam (y = 0 and y = W) at the bottom. Arc from the anchor
    // (the top line, y = W/2): v = -(y - W/2) * Rc / R_dev.
    const hmap = (Rd) => (x, y, Rc = Rd) => [x, -(y - W / 2) * Rc / Rd];
    for (const [key, P] of [['header5', D.pat.header5], ['header6', D.pat.header6]]) {
      const Rq = P.request; const cols = headerColumns(P.holes[0], L, W, 12);
      this.makePlate(key, cols, 56, headerFrame, Rq.header_radius, hmap(Rq.header_dev_radius), [1 / 300, 1 / 300]);
    }
    // branch plate (x around, y along the axis from O). Fit: th = th0 + s x / r_dev with s = -1, th0 = 90 deg; the joint is
    // symmetric about the XY plane (delta = 0), so the mirror th = -90 deg + x / r_dev fits the same and puts the seam at -Z
    // (away from the camera). Anchor x_a = W/2 (th = 90 deg, facing +Z): n_a = e2 = +Z, t_a = -e1.
    const e1 = [-Math.sin(b), Math.cos(b), 0];
    for (const key of ['branch5', 'branch6', 'branch4']) {
      const P = D.pat[key], Rq = P.request, Wb = P.quantities.developed_width;
      const F = { O: [Rq.branch_station ?? req.branch_station, 0, Rq.delta], a: [Math.cos(b), Math.sin(b), 0], na: [0, 0, 1], ta: [-e1[0], -e1[1], -e1[2]] };
      this.makePlate(key, branchColumns(P.curves.saddle_cut, Rq.s_end, 2), 40, F, Rq.branch_radius, (x, y, r = Rq.branch_radius) => [y, (x - Wb / 2) * r / Rq.branch_dev_radius], [1 / 300, 1 / 300]);
    }
  }

  // ------------------------------------------------------------------ the five families (scene 4)
  buildFamilies() {
    // shell (job 1): x around 0 .. 2 pi r_dev, y along 0 .. length; anchor at the plate middle
    { const P = D.pat.shell, Wd = P.quantities.developed_width, Lg = P.request.length;
      this.makePlate('shell', rectColumns(Wd, Lg, 220), 24, { O: [0, 0, 0], a: [1, 0, 0], na: [0, 1, 0], ta: [0, 0, 1] }, P.request.r_dev, (x, y) => [y, x - Wd / 2], [1 / 600, 1 / 600]); }
    // header alone (job 5 header) and branch alone (job 4) reuse the tee builders' plates: 'header5', 'branch4'
    // cone (job 2): sector in polar coordinates about the apex (NF4.fit.cone)
    { const P = D.pat.cone, f = D.fit.cone, nP = 140, nR = 16; const nV = (nP + 1) * (nR + 1);
      const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.BufferAttribute(new Float32Array(nV * 3), 3)); g.setAttribute('normal', new THREE.BufferAttribute(new Float32Array(nV * 3), 3));
      const pol = new Float32Array(nV * 2), uv = new Float32Array(nV * 2); let k = 0;
      for (let i = 0; i <= nP; i++) for (let j = 0; j <= nR; j++) { const psi = f.psi0 + (f.psi1 - f.psi0) * i / nP, rho = f.rhoS + (f.rhoL - f.rhoS) * j / nR; pol[2 * k] = rho; pol[2 * k + 1] = psi; uv[2 * k] = rho / 300; uv[2 * k + 1] = psi * rho / 300; k++; }
      g.setAttribute('uv', new THREE.BufferAttribute(uv, 2));
      const idx = []; for (let i = 0; i < nP; i++) for (let j = 0; j < nR; j++) { const p = i * (nR + 1) + j, q = p + nR + 1; idx.push(p, q, p + 1, p + 1, q, q + 1); } g.setIndex(idx);
      const mesh = new THREE.Mesh(g, this.steel); mesh.frustumCulled = false; const grp = new THREE.Group(); grp.add(mesh); this.root.add(grp);
      const psiA = (f.psi0 + f.psi1) / 2;
      const part = { name: 'cone', mesh, grp, g, kappa: -1 };
      part.set = (kappa) => { if (Math.abs(kappa - part.kappa) < 1e-7) return; part.kappa = kappa;
        const qq = 1 / (1 + kappa * (1 / f.q - 1));          // q' from 1 (flat) to q (the cone): 1/q' = 1 + kappa (1/q - 1)
        const Pp = g.attributes.position.array, Nn = g.attributes.normal.array;
        for (let i = 0; i < nV; i++) conePoint(pol[2 * i], pol[2 * i + 1], psiA, qq, Pp, Nn, 3 * i);
        g.attributes.position.needsUpdate = true; g.attributes.normal.needsUpdate = true; };
      this.parts.cone = part; }
    // eccentric reducer (job 3): oblique cone; the plate maps to (t, rho) exactly at both ends; between them the
    // generators turn from the flat plane to their 3D directions (an approximation: labelled as a visualisation)
    { const P = D.pat.ecc, req = P.request, T = eccTable(req), A = T.A, nP = 180, nR = 14, nV = (nP + 1) * (nR + 1);
      const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.BufferAttribute(new Float32Array(nV * 3), 3)); g.setAttribute('normal', new THREE.BufferAttribute(new Float32Array(nV * 3), 3));
      const data = []; let k = 0; const uv = new Float32Array(nV * 2);
      const psiA = T.total / 2;
      for (let i = 0; i <= nP; i++) { const psi = T.total * i / nP, t = T.tOf(psi); const p1 = [req.r1 * Math.cos(t), req.r1 * Math.sin(t), 0];
        const d = [p1[0] - A[0], p1[1] - A[1], p1[2] - A[2]], rl = Math.hypot(...d);
        for (let j = 0; j <= nR; j++) { const rho = rl * (T.A[2] - req.h) / T.A[2] + (rl - rl * (T.A[2] - req.h) / T.A[2]) * j / nR; data.push({ psi, t, d: d.map((v) => v / rl), rho }); uv[2 * k] = rho / 300; uv[2 * k + 1] = psi * rho / 300; k++; } }
      g.setAttribute('uv', new THREE.BufferAttribute(uv, 2));
      const idx = []; for (let i = 0; i < nP; i++) for (let j = 0; j < nR; j++) { const p = i * (nR + 1) + j, q = p + nR + 1; idx.push(p, q, p + 1, p + 1, q, q + 1); } g.setIndex(idx);
      const mesh = new THREE.Mesh(g, this.steel); mesh.frustumCulled = false; const grp = new THREE.Group(); grp.add(mesh); this.root.add(grp);
      // 3D frame: job axis z -> world +Y down from the apex; job x -> world -Z ... (local: apex at origin)
      // the generator at t = pi (plate middle, psi_a) faces +Z in both end states
      const to3 = (v) => [-v[1], v[2], -v[0]];          // job (x, y, z) -> local (X, Y, Z): x -> -Z (t = pi faces +Z), z -> +Y, y -> -X
      const part = { name: 'ecc', mesh, grp, g, kappa: -1 };
      part.set = (kappa) => { if (Math.abs(kappa - part.kappa) < 1e-7) return; part.kappa = kappa;
        const Pp = g.attributes.position.array;
        for (let i = 0; i < nV; i++) { const q = data[i], a = q.psi - psiA;
          const f = [Math.sin(a), 0, Math.cos(a)];                  // flat: in the horizontal plane through the apex
          const d3 = to3(q.d);                                       // 3D: the oblique-cone generator
          let x = lerp(f[0], d3[0], kappa), y = lerp(f[1], d3[1], kappa), z = lerp(f[2], d3[2], kappa); const n = Math.hypot(x, y, z) || 1;
          Pp[3 * i] = q.rho * x / n; Pp[3 * i + 1] = q.rho * y / n; Pp[3 * i + 2] = q.rho * z / n; }
        g.attributes.position.needsUpdate = true; g.computeVertexNormals();
        // orient the normals outward: in the flat state the outer side faces up (+Y); the sign holds through the morph
        if (part.flip === undefined && kappa === 0) { const mid = ((nP >> 1) * (nR + 1) + (nR >> 1)) * 3 + 1; part.flip = g.attributes.normal.array[mid] < 0; }
        if (part.flip) { const nn = g.attributes.normal.array; for (let i = 0; i < nn.length; i++) nn[i] = -nn[i]; g.attributes.normal.needsUpdate = true; } };
      part.set(0); part.kappa = -1;
      this.parts.ecc = part; }
  }

  // ------------------------------------------------------------------ highlight tubes (curves on the steel)
  tube(name, pts, radius, color, closed = true, segs = 600) {
    const curve = new THREE.CatmullRomCurve3(pts.map((p) => new THREE.Vector3(...p)), closed, 'centripetal');
    const g = new THREE.TubeGeometry(curve, segs, radius, 10, closed);
    const mat = new THREE.MeshBasicMaterial({ color, toneMapped: false, transparent: true });
    const m = new THREE.Mesh(g, mat); m.frustumCulled = false; m.renderOrder = 2; this.root.add(m);
    const t = { mesh: m, mat, total: g.index.count, segs };
    t.draw = (u, opacity = 1) => { m.visible = u > 0.001 && opacity > 0.001; const n = Math.round(Math.max(0, Math.min(1, u)) * segs) * 10 * 6; g.setDrawRange(0, n); mat.opacity = opacity; };
    this.tubes[name] = t; return t;
  }
  dropTube(name) { const t = this.tubes[name]; if (!t) return; this.root.remove(t.mesh); t.mesh.geometry.dispose(); t.mat.dispose(); delete this.tubes[name]; }

  // ------------------------------------------------------------------ camera and lights for a frame
  // cam: {tx, ty, tz (target), az, el (deg), dist, fov, roll}
  setCamera(c) {
    const az = c.az * deg, el = c.el * deg;
    this.cam.fov = c.fov || 24; this.cam.updateProjectionMatrix();
    this.cam.position.set(c.tx + c.dist * Math.cos(el) * Math.sin(az), c.ty + c.dist * Math.sin(el), c.tz + c.dist * Math.cos(el) * Math.cos(az));
    this.cam.up.set(0, 1, 0); this.cam.lookAt(c.tx, c.ty, c.tz);
    if (c.roll) this.cam.rotateZ(c.roll * deg);
    if (c.sx || c.sy) { this.cam.setViewOffset(1920, 1080, -(c.sx || 0), -(c.sy || 0), 1920, 1080); } else this.cam.clearViewOffset();
    this.cam.updateMatrixWorld();
  }
  setLights(cx, cy, cz, span) {
    this.key.position.set(cx - 0.6 * span, cy + 2.0 * span, cz + 1.4 * span); this.key.target.position.set(cx, cy, cz);
    this.rim.position.set(cx + 0.8 * span, cy + 1.0 * span, cz - 2.0 * span); this.rim.target.position.set(cx, cy, cz);
  }
  // contact shadow area: centre (cx, cz), square side size, floor height, falloff height, opacity
  setShadow(o) { Object.assign(this.shadow, o); }
  renderShadow() {
    const sh = this.shadow, r = this.r, s = this.scene, c = this.shCam;
    this.floor.position.set(sh.cx, sh.floorY, sh.cz); this.floor.scale.set(sh.size, sh.size, 1); this.floor.material.opacity = sh.opacity;
    this.floor.visible = sh.opacity > 0.005; if (!this.floor.visible) return;
    c.left = -sh.size / 2; c.right = sh.size / 2; c.top = sh.size / 2; c.bottom = -sh.size / 2; c.near = 0; c.far = sh.height * 4;
    c.position.set(sh.cx, sh.floorY, sh.cz); c.up.set(0, 0, -1); c.lookAt(sh.cx, sh.floorY + 1, sh.cz); c.updateProjectionMatrix(); c.updateMatrixWorld();
    // the camera looks up, so its image is mirrored in x relative to the floor texture: flip with scale
    c.scale.set(-1, 1, 1); c.updateMatrixWorld();
    this.shMat.uniforms.floorY.value = sh.floorY; this.shMat.uniforms.height.value = sh.height;
    const hidden = []; s.traverse((o) => { if (o.isMesh && o !== this.floor && (o.renderOrder > 0 || !o.visible)) { if (o.visible) { hidden.push(o); o.visible = false; } } });
    this.floor.visible = false; const bg = s.background, env = s.environment; s.overrideMaterial = this.shMat;
    r.setRenderTarget(this.shRT); r.setClearColor(0x000000, 0); r.clear(); r.render(s, c);
    s.overrideMaterial = null; this.floor.visible = true; hidden.forEach((o) => (o.visible = true));
    for (let k = 0; k < 2; k++) {
      this.blurMat.uniforms.tex.value = this.shRT.texture; this.blurMat.uniforms.dir.value.set(1 / 512, 0); r.setRenderTarget(this.shRT2); r.clear(); r.render(this.quadScene, this.quadCam);
      this.blurMat.uniforms.tex.value = this.shRT2.texture; this.blurMat.uniforms.dir.value.set(0, 1 / 512); r.setRenderTarget(this.shRT); r.clear(); r.render(this.quadScene, this.quadCam);
    }
    r.setRenderTarget(null);
  }
  project(v) { const p = v.clone().project(this.cam); return [(p.x + 1) / 2 * 1920, (1 - p.y) / 2 * 1080, p.z]; }
  render() { this.renderShadow(); this.r.setClearColor(0x000000, 0); this.r.render(this.scene, this.cam); }
}
