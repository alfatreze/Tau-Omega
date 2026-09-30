// Drives the Library workbench through its key states against the dev mock
// and screenshots each. Usage: (npm run dev &) && node scripts/workbench-states.cjs [outDir]
const { chromium } = require('playwright');
const out = process.argv[2] || 'screenshots/workbench';
(async () => {
  require('fs').mkdirSync(out, { recursive: true });
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  p.on('pageerror', (e) => console.log('pageerror:', e.message));
  p.on('console', (m) => { if (m.type() === 'error' || m.text().startsWith('[dev-mock]')) console.log(m.text()); });
  const url = process.env.URL || 'http://localhost:5173';
  await p.goto(url); await p.evaluate(() => localStorage.clear()); await p.reload(); await p.waitForTimeout(900);
  const pk = p.getByRole('region', { name: 'On Pocket' }); const pc = p.getByRole('region', { name: 'This computer' });
  const shot = async (n) => { await p.waitForTimeout(300); await p.screenshot({ path: `${out}/${n}.png` }); };
  await p.getByRole('button', { name: 'Library', exact: true }).click();
  await shot('01-choose-folder');
  await p.getByRole('button', { name: 'Choose folder…' }).click(); await p.waitForTimeout(500); await shot('02-library-loaded');
  await pc.getByLabel('Select Mingus Ah Um').check(); await pc.getByLabel('Select Head Hunters').check();
  await p.getByRole('button', { name: /^Add 2 to Analogue Pocket/ }).click(); await shot('03-added');
  await p.getByRole('button', { name: 'Start sync' }).click(); await shot('04-review-with-slow-warning');
  await p.getByLabel(/Don't ask again/).check(); await p.getByRole('button', { name: 'Sync anyway' }).click(); await p.waitForTimeout(1500); await shot('06-progress');
  await p.waitForSelector('text=Sync complete', { timeout: 20000 }); await shot('07-done');
  await p.getByRole('button', { name: 'Done' }).click();
  await pk.getByLabel('Select Time Out').check(); await p.getByRole('button', { name: 'Remove', exact: true }).click(); await shot('08-first-remove');
  await p.getByRole('button', { name: 'Mark for removal' }).click(); await shot('09-removal-staged');
  await pk.getByLabel('Select Blue Train').check(); await p.getByRole('button', { name: 'Edit…' }).click(); await shot('10-edit-drawer');
  await p.getByRole('button', { name: 'Choose image…' }).click(); await p.waitForTimeout(400); await shot('11-edit-cover');
  await p.getByRole('button', { name: 'Save (applies when you sync)' }).click(); await shot('12-pending-mixed');
  await p.getByRole('button', { name: 'Clear all' }).click();
  for (const t of ['Bitches Brew', 'The Black Saint', 'Moanin', 'Saxophone Colossus', 'A Love Supreme', 'Kind of Blue']) await pc.getByLabel(`Select ${t}`).check().catch(() => {});
  await p.getByRole('button', { name: /^Add \d+ to Analogue Pocket/ }).click(); await shot('13-over-capacity');
  await p.getByRole('button', { name: 'Clear all' }).click();
  await pk.getByRole('tab', { name: 'Tracks' }).click(); await shot('14-tracks');
  await p.getByRole('button', { name: /Direct USB/ }).click(); await shot('15-connection-menu');
  await p.keyboard.press('Escape');
  await p.getByRole('button', { name: /Tools & settings/ }).click(); await p.getByRole('button', { name: 'Settings', exact: true }).click(); await shot('16-settings-library');
  await b.close();
})();
