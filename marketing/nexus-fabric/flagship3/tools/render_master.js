// Renders a film straight from the page's canvas into ffmpeg: one frame at a time, no frames stored on disk.
// Frame f is film time f / fps (window.seek). --scale 2 renders the 3840x2160 master (device scale factor 2).
// usage: node tools/render_master.js <en|ja> <out.mp4> [--scale 1|2] [--fps 30|60] [--crf 12] [--preset slow]
//        [--abitrate 320] [--audio audio/mix-<lang>.wav] [--from 0] [--to <last frame>] [--noaudio]
const { chromium } = require('playwright');
const { spawn } = require('child_process');
const crypto = require('crypto');
const path = require('path');
const [lang, out] = process.argv.slice(2, 4);
const opt = (k, d) => { const i = process.argv.indexOf('--' + k); return i > 0 ? process.argv[i + 1] : d; };
const SCALE = Number(opt('scale', '1')), FPS = Number(opt('fps', '30')), CRF = opt('crf', '12'), PRESET = opt('preset', 'slow');
const ABIT = opt('abitrate', '320'), AUDIO = opt('audio', `audio/mix-${lang}.wav`), NOAUDIO = process.argv.includes('--noaudio');
const LABELS = opt('labels', '1') === '0' ? '0' : '1';   // 0 = the owner's clean variant without the film's disclosure captions
const FROM = Number(opt('from', '0')), TO = Number(opt('to', String(54 * FPS - 1)));
(async () => {
  const t0 = Date.now();
  const br = await chromium.launch({ args: ['--allow-file-access-from-files'] });
  const page = await br.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  const errs = []; page.on('pageerror', (e) => errs.push(e.message));
  await page.goto('file://' + path.resolve(__dirname, '..', 'film.html') + `?lang=${lang}&scale=${SCALE}&labels=${LABELS}`);
  await page.waitForFunction('window.READY !== undefined', null, { timeout: 300000 });
  if ((await page.evaluate('window.READY')) !== true) throw new Error('film not ready');
  const args = ['-hide_banner', '-loglevel', 'error', '-y', '-f', 'image2pipe', '-framerate', String(FPS), '-c:v', 'png', '-i', '-'];
  if (!NOAUDIO) args.push('-i', path.resolve(__dirname, '..', AUDIO));
  args.push('-map', '0:v:0');
  if (!NOAUDIO) args.push('-map', '1:a:0', '-c:a', 'aac', '-b:a', `${ABIT}k`, '-ar', '48000', '-ac', '2', '-t', String(54));
  args.push('-vf', 'scale=out_color_matrix=bt709:out_range=tv,format=yuv420p', '-c:v', 'libx264', '-preset', PRESET, '-crf', CRF,
    '-profile:v', 'high', '-colorspace', 'bt709', '-color_primaries', 'bt709', '-color_trc', 'bt709', '-color_range', 'tv',
    '-r', String(FPS), '-movflags', '+faststart', out);
  const ff = spawn('ffmpeg', args, { stdio: ['pipe', 'inherit', 'inherit'] });
  const done = new Promise((res, rej) => ff.on('close', (c) => (c === 0 ? res() : rej(new Error('ffmpeg exited ' + c)))));
  let prev = null, dups = 0, n = 0;
  for (let f = FROM; f <= TO; f++) {
    const url = await page.evaluate((t) => { window.seek(t); return document.getElementById('gfx').toDataURL('image/png'); }, f / FPS);
    const b64 = url.slice(url.indexOf(',') + 1);
    const h = crypto.createHash('md5').update(b64).digest('hex');
    if (h === prev) dups++;
    prev = h; n++;
    const buf = Buffer.from(b64, 'base64');
    if (!ff.stdin.write(buf)) await new Promise((r) => ff.stdin.once('drain', r));
    if (n % 300 === 0) console.log(`frame ${f} (${n} rendered, ${((Date.now() - t0) / 1000).toFixed(0)} s)`);
  }
  ff.stdin.end();
  await done;
  await br.close();
  console.log(`wrote ${out}: ${n} frames at ${FPS} fps, scale ${SCALE}, ${dups} frames identical to the previous one, ${((Date.now() - t0) / 1000).toFixed(0)} s, page errors ${errs.length}`);
})().catch((e) => { console.error('render failed:', e.message); process.exit(1); });
