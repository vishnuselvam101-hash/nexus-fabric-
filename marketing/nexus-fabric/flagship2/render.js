#!/usr/bin/env node
// Renders the film frame by frame. Frame f shows film time f / 30 s (window.seek(f / 30)); each frame is a PNG piped
// into ffmpeg (libx264, crf 15, yuv420p, bt709 matrix and tags). 1500 frames = 50.0 s at 30 fps.
//
//   node render.js video <en|ja> <out.mp4> [--audio <mix.wav>] [--frames <n>] [--preset slow]
//       --frames <n>   render only the first n frames (checks); --audio muxes AAC audio (48 kHz stereo) into the file
//   node render.js stills <en|ja> <dir> <t1,t2,...>
//   node render.js cues <en|ja> <out.json>
//       the sound cues the picture asks for (window.NF_CUES) and the timeline anchors: audio/make_music.py reads them
//       full-size PNG stills at the given film times in seconds, named <lang>-<t with two decimals>.png
//
// Needs the film's footage (capture/footage/<lang>) and fonts (fonts/); nothing is fetched over the network.
const { chromium } = require('playwright');
const { spawn } = require('child_process');
const fs = require('fs'), path = require('path');

const HERE = __dirname, FPS = 30, DUR_FRAMES = 1500;
const [mode, lang, a3, a4] = process.argv.slice(2);
const opt = (k, d) => { const i = process.argv.indexOf('--' + k); return i > 0 ? process.argv[i + 1] : d; };
if (!['video', 'stills', 'cues'].includes(mode) || !['en', 'ja'].includes(lang)) {
  console.error('usage: node render.js video <en|ja> <out.mp4> [--audio <wav>] [--frames n] [--preset slow] | stills <en|ja> <dir> <t,t,...>');
  process.exit(2);
}

(async () => {
  const br = await chromium.launch({ args: ['--allow-file-access-from-files', '--font-render-hinting=none'] });
  const page = await br.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  const errs = [];
  page.on('pageerror', (e) => errs.push('pageerror: ' + e.message));
  page.on('console', (m) => { if (m.type() === 'error' || m.type() === 'warning') errs.push(m.type() + ': ' + m.text()); });
  page.on('requestfailed', (r) => errs.push('requestfailed: ' + r.url()));
  page.on('request', (r) => { if (!r.url().startsWith('file:')) errs.push('network request: ' + r.url()); });
  await page.goto('file://' + path.join(HERE, 'film.html') + '?lang=' + lang);
  await page.waitForFunction('window.READY !== undefined', null, { timeout: 120000 });
  if ((await page.evaluate('window.READY')) !== true) throw new Error('the film did not start: ' + errs.join(' | '));

  if (mode === 'cues') {
    const data = await page.evaluate(() => ({ cues: window.NF_CUES, info: window.NF_INFO }));
    fs.mkdirSync(path.dirname(path.resolve(a3)), { recursive: true }); fs.writeFileSync(path.resolve(a3), JSON.stringify(data, null, 1));
    console.log('wrote', path.resolve(a3), data.cues.length, 'cues');
  } else if (mode === 'stills') {
    const dir = path.resolve(a3); fs.mkdirSync(dir, { recursive: true });
    for (const t of a4.split(',').map(Number)) {
      await page.evaluate((x) => window.seek(x), t);
      const f = path.join(dir, `${lang}-${t.toFixed(2)}.png`);
      await page.screenshot({ path: f, type: 'png' });
      console.log('still', f);
    }
  } else if (mode === 'video') {
    const out = path.resolve(a3), audio = opt('audio', ''), n = Number(opt('frames', DUR_FRAMES)), preset = opt('preset', 'slow');
    fs.mkdirSync(path.dirname(out), { recursive: true });
    const args = ['-hide_banner', '-loglevel', 'error', '-y', '-f', 'image2pipe', '-framerate', String(FPS), '-c:v', 'png', '-i', '-'];
    if (audio) args.push('-i', path.resolve(audio));
    args.push('-map', '0:v:0');
    if (audio) args.push('-map', '1:a:0', '-c:a', 'aac', '-b:a', '192k', '-ar', '48000', '-ac', '2');
    args.push('-vf', 'scale=out_color_matrix=bt709:out_range=tv,format=yuv420p', '-c:v', 'libx264', '-preset', preset, '-crf', '15',
      '-profile:v', 'high', '-colorspace', 'bt709', '-color_primaries', 'bt709', '-color_trc', 'bt709', '-color_range', 'tv',
      '-r', String(FPS), '-frames:v', String(n), '-movflags', '+faststart', out);
    const ff = spawn('ffmpeg', args, { stdio: ['pipe', 'inherit', 'inherit'] });
    const done = new Promise((res, rej) => { ff.on('close', (c) => (c === 0 ? res() : rej(new Error('ffmpeg exited ' + c)))); });
    for (let f = 0; f < n; f++) {
      await page.evaluate((x) => window.seek(x), f / FPS);            // frame-exact: film time = f / 30
      const png = await page.screenshot({ type: 'png' });
      if (!ff.stdin.write(png)) await new Promise((r) => ff.stdin.once('drain', r));
      if (f % 150 === 0) console.log(`frame ${f}/${n}`);
    }
    ff.stdin.end();
    await done;
    console.log('wrote', out, `${n} frames`);
  }
  console.log('errors:', errs.length, errs.slice(0, 5).join(' | '));
  await br.close();
  if (errs.length) process.exitCode = 1;
})().catch((e) => { console.error('render failed:', e && e.message || e); process.exit(1); });
