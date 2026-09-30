// Screenshots every sidebar screen against the dev mock.
// Usage: (npm run dev &) && node scripts/screenshots.cjs [outDir]
const { chromium } = require('playwright');
const out = process.argv[2] || 'screenshots';
const main = ['Cards', 'Library', 'Playlists'];
const tools = ['Compare cores', 'Backup', 'Packages', 'Problems', 'Recent jobs', 'Settings'];
(async () => {
  require('fs').mkdirSync(out, { recursive: true });
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1280, height: 800 } });
  p.on('pageerror', (e) => console.log('pageerror:', e.message));
  p.on('console', (m) => (m.type() === 'error' || m.text().startsWith('[dev-mock]')) && console.log(m.text()));
  await p.goto(process.env.URL || 'http://localhost:5173');
  await p.evaluate(() => localStorage.clear()); await p.reload();
  await p.waitForTimeout(1000);
  const shot = async (name) => { await p.waitForTimeout(400); await p.screenshot({ path: `${out}/${name.toLowerCase().replace(/\s+/g, '-')}.png` }); };
  for (const name of main) { await p.getByRole('button', { name, exact: true }).first().click(); await shot(name); }
  await p.getByRole('button', { name: /Tools & settings/ }).click();
  for (const name of tools) { await p.getByRole('button', { name, exact: true }).first().click(); await shot(name); }
  await b.close();
})();
