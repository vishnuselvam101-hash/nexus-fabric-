// Renders film 4 straight from the page into ffmpeg: frames are piped, never stored on disk.
// The 3D layer of a frame is drawn once and the overlay of every requested variant is drawn on it, so one pass can
// write several files (for instance en, ja, en-clean, ja-clean). Frame f is film time f / fps.
// usage: node tools/render_master.js --variants en:1,ja:1[,en:0,ja:0] [--scale 1|2] [--fps 30|60] [--crf 12] [--preset slow]
//        [--abitrate 320] [--suffix ''] [--from 0] [--to <last frame>] [--noaudio] [--outdir final]
// Output names: <outdir>/NEXUS-FABRIC-film4-54s-<lang>[-clean][<suffix>].mp4 ; audio: ../flagship3/audio/mix-<lang>.wav (locked mix).
const { chromium } = require('playwright');
const { spawn } = require('child_process');
const path = require('path');
const opt = (k, d) => { const i = process.argv.indexOf('--' + k); return i > 0 ? process.argv[i + 1] : d; };
const SCALE = Number(opt('scale', '1')), FPS = Number(opt('fps', '30')), CRF = opt('crf', '12'), PRESET = opt('preset', 'slow');
const ABIT = opt('abitrate', '320'), NOAUDIO = process.argv.includes('--noaudio'), SUFFIX = opt('suffix', ''), OUTDIR = path.resolve(path.resolve(__dirname, '..'), opt('outdir', 'final'));   // relative to flagship4, or absolute
const VARS = opt('variants', 'en:1,ja:1').split(',').map((v) => { const [lang, l] = v.split(':'); return { lang, labels: l !== '0' }; });
const FROM = Number(opt('from', '0')), TO = Number(opt('to', String(54 * FPS - 1)));
const ROOT = path.resolve(__dirname, '..');
(async () => {
  const t0 = Date.now();
  const br = await chromium.launch({ args: ['--allow-file-access-from-files', '--ignore-gpu-blocklist', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
  const page = await br.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  const errs = []; page.on('pageerror', (e) => errs.push(e.message)); page.on('console', (m) => { if (m.type() === 'error') errs.push(m.text()); });
  await page.goto('file://' + path.join(ROOT, 'film.html') + `?lang=${VARS[0].lang}&labels=${VARS[0].labels ? 1 : 0}&scale=${SCALE}`);
  await page.waitForFunction('window.READY !== undefined', null, { timeout: 600000 });
  if ((await page.evaluate('window.READY')) !== true) throw new Error('film not ready: ' + (await page.evaluate('window.READY_ERROR')));
  // WebGL must draw: the 3D layer at 1.0 s and 23.0 s must have drawn pixels that are not black
  for (const t of [1.0, 23.0]) { const g = await page.evaluate((x) => window.glCheck(x), t); console.log(`webgl check t=${t}: ${(g.drawn * 100).toFixed(1)} % drawn, mean ${g.mean.toFixed(0)}`); if (g.drawn < 0.05 || g.mean < 40) throw new Error('3D layer is empty or black'); }
  const outs = VARS.map((v) => path.join(OUTDIR, `NEXUS-FABRIC-film4-54s-${v.lang}${v.labels ? '' : '-clean'}${SUFFIX}.mp4`));
  const ffs = VARS.map((v, k) => {
    const args = ['-hide_banner', '-loglevel', 'error', '-y', '-f', 'image2pipe', '-framerate', String(FPS), '-c:v', 'png', '-i', '-'];
    if (!NOAUDIO) args.push('-i', path.resolve(ROOT, '..', 'flagship3', 'audio', `mix-${v.lang}.wav`));
    args.push('-map', '0:v:0');
    if (!NOAUDIO) args.push('-map', '1:a:0', '-c:a', 'aac', '-b:a', `${ABIT}k`, '-ar', '48000', '-ac', '2', '-t', '54');
    args.push('-vf', 'scale=out_color_matrix=bt709:out_range=tv,format=yuv420p', '-c:v', 'libx264', '-preset', PRESET, '-crf', CRF,
      '-profile:v', 'high', '-colorspace', 'bt709', '-color_primaries', 'bt709', '-color_trc', 'bt709', '-color_range', 'tv',
      '-r', String(FPS), '-movflags', '+faststart', outs[k]);
    const ff = spawn('ffmpeg', args, { stdio: ['pipe', 'inherit', 'inherit'] });
    ff.done = new Promise((res, rej) => ff.on('close', (c) => (c === 0 ? res() : rej(new Error('ffmpeg exited ' + c)))));
    return ff;
  });
  let n = 0;
  for (let f = FROM; f <= TO; f++) {
    const urls = await page.evaluate(([t, vs]) => window.frameVariants(t, vs), [f / FPS, VARS]);
    for (let k = 0; k < ffs.length; k++) { const buf = Buffer.from(urls[k].slice(urls[k].indexOf(',') + 1), 'base64'); if (!ffs[k].stdin.write(buf)) await new Promise((r) => ffs[k].stdin.once('drain', r)); }
    n++; if (n % 150 === 0) console.log(`frame ${f} (${n} rendered, ${((Date.now() - t0) / 1000).toFixed(0)} s)`);
  }
  ffs.forEach((ff) => ff.stdin.end()); await Promise.all(ffs.map((ff) => ff.done)); await br.close();
  console.log(`wrote ${outs.join(', ')}: ${n} frames each at ${FPS} fps, scale ${SCALE}, ${((Date.now() - t0) / 1000).toFixed(0)} s, page errors ${errs.length} ${errs.slice(0, 3).join(' | ')}`);
  if (errs.length) process.exit(2);
})().catch((e) => { console.error('render failed:', e.message); process.exit(1); });
