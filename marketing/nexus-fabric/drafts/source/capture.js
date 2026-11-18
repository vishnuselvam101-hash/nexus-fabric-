// Captures real application screens for the draft animatic. Needs a running server that already
// holds the data created by tests/ui_smoke.js (admin / correct-horse-9, jobs 1-3).
const { chromium } = require('playwright');
(async () => {
  const out = process.argv[2]; const B = process.env.NF_BASE || 'http://127.0.0.1:8793';
  const b = await chromium.launch(); const p = await b.newPage({ viewport: { width: 1600, height: 900 } });
  await p.goto(B); await p.fill('input[name=u]', 'admin'); await p.fill('input[name=p]', 'correct-horse-9'); await p.click('button'); await p.waitForSelector('text=Recent engineering jobs');
  const shot = async (hash, name, sel) => { await p.goto(B + '/#/' + hash); await p.waitForTimeout(700); if (sel) await p.locator(sel).first().scrollIntoViewIfNeeded(); await p.waitForTimeout(200); await p.screenshot({ path: `${out}/${name}.png` }); };
  await shot('dashboard', 'app-dashboard');
  await shot('calc/3', 'app-pattern-header', 'text=Flat pattern: header');
  await shot('calc/3', 'app-pattern-branch', 'text=Flat pattern: branch');
  await shot('calc/3', 'app-verification', 'text=Independent verification');
  await shot('calc/3', 'app-obligations', 'text=Scrutiny obligations from the correction');
  await shot('calc/3', 'app-gates', 'text=Release gates');
  await shot('amoeba', 'app-amoeba');
  await b.close();
})().catch((e) => { console.error(e.message); process.exit(1); });
