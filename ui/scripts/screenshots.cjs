// Screenshots every sidebar screen against the dev mock.
// Usage: (npm run dev &) && node scripts/screenshots.cjs [outDir]
const { chromium } = require('playwright');
const out = process.argv[2] || 'screenshots';
const pages = ['Cards', 'Library workbench', 'Compare cores', 'Sync library', 'Playlists', 'Backup', 'Packages', 'Problems', 'Library', 'Recent jobs', 'Settings'];
(async () => {
  require('fs').mkdirSync(out, { recursive: true });
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1280, height: 800 } });
  p.on('pageerror', (e) => console.log('pageerror:', e.message));
  p.on('console', (m) => m.text().startsWith('[dev-mock]') && console.log(m.text()));
  await p.goto(process.env.URL || 'http://localhost:5173');
  await p.waitForTimeout(1000);
  for (const name of pages) {
    await p.getByRole('button', { name, exact: true }).first().click();
    await p.waitForTimeout(400);
    await p.screenshot({ path: `${out}/${name.toLowerCase().replace(/\s+/g, '-')}.png` });
  }
  await b.close();
})();
