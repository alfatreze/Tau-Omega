// Drives the Library workbench prototype through its key states and screenshots each.
// Usage: (npm run dev &) && node scripts/workbench-states.cjs [outDir]
const { chromium } = require('playwright');
const out = process.argv[2] || 'screenshots/workbench';
(async () => {
  require('fs').mkdirSync(out, { recursive: true });
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  p.on('pageerror', (e) => console.log('pageerror:', e.message));
  p.on('console', (m) => { if (m.type() === 'error' || m.text().startsWith('[dev-mock]')) console.log(m.text()); });
  await p.goto(process.env.URL || 'http://localhost:5173');
  await p.evaluate(() => localStorage.clear());
  await p.reload(); await p.waitForTimeout(800);
  const pk = p.getByRole('region', { name: 'On Pocket' }); const pc = p.getByRole('region', { name: 'This computer' });
  const shot = async (n) => { await p.waitForTimeout(250); await p.screenshot({ path: `${out}/${n}.png` }); };
  await p.getByRole('button', { name: 'Library workbench', exact: true }).click();
  await shot('01-empty');
  // add two albums by ticking
  await pc.getByLabel('Select Mingus Ah Um').check(); await pc.getByLabel('Select Head Hunters').check();
  await p.getByRole('button', { name: /^Add 2 to Pocket/ }).click(); await shot('02-added');
  // slow alert on first sync
  await p.getByRole('button', { name: 'Start sync' }).click(); await shot('03-slow-alert');
  await p.getByLabel(/Don't ask again/).check(); await p.getByRole('button', { name: 'Sync anyway' }).click(); await shot('04-review');
  await p.getByRole('button', { name: 'Confirm and start' }).click(); await p.waitForTimeout(2200); await shot('05-progress');
  await p.waitForSelector('text=Sync complete', { timeout: 15000 }); await shot('06-done');
  await p.getByRole('button', { name: 'Done' }).click();
  // remove flow with first-time explanation
  await pk.getByLabel('Select Time Out').check(); await p.getByRole('button', { name: 'Remove', exact: true }).click(); await shot('07-first-remove');
  await p.getByRole('button', { name: 'Mark for removal' }).click(); await shot('08-removal-staged');
  // edit drawer
  await pk.getByLabel('Select Blue Train').check(); await p.getByRole('button', { name: 'Edit info' }).click(); await shot('09-edit-drawer');
  await p.getByLabel('Use cover 2').click(); await p.getByRole('button', { name: 'Add to Pending changes' }).click(); await shot('10-pending-mixed');
  // over capacity
  await p.getByRole('button', { name: 'Clear all' }).click();
  for (const t of ['Bitches Brew','The Black Saint','Head Hunters','Mingus Ah Um','Moanin','Saxophone Colossus','Maiden Voyage','A Love Supreme']) await pc.getByLabel(`Select ${t}`).check().catch(()=>{});
  await p.getByRole('button', { name: /^Add \d+ to Pocket/ }).click(); await shot('11-over-capacity');
  await b.close();
})();
