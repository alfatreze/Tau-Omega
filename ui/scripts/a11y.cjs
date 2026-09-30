// Runs axe-core over the Library workbench states against the dev mock.
// Usage: (npm run dev &) && AXE=/path/to/axe.min.js NODE_PATH=$(npm root -g) node scripts/a11y.cjs
const { chromium } = require('playwright');
const fs = require('fs');
const axeSource = fs.readFileSync(process.env.AXE || require.resolve('axe-core/axe.min.js'), 'utf8');
(async () => {
  const b = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.goto(process.env.URL || 'http://localhost:5173'); await p.evaluate(() => localStorage.clear()); await p.reload(); await p.waitForTimeout(800);
  const pk = p.getByRole('region', { name: 'On Pocket' }); const pc = p.getByRole('region', { name: 'This computer' });
  let failures = 0;
  const audit = async (name) => {
    await p.addScriptTag({ content: axeSource }).catch(() => {});
    const r = await p.evaluate(async () => await window.axe.run(document, { runOnly: ['wcag2a', 'wcag2aa', 'best-practice'] }));
    console.log(`\n== ${name}: ${r.violations.length} violation(s)`);
    for (const v of r.violations) { failures++; console.log(`- [${v.impact}] ${v.id}: ${v.help} (${v.nodes.length} node(s))\n    e.g. ${v.nodes[0].html.slice(0, 140)}`); }
  };
  await p.getByRole('button', { name: 'Library', exact: true }).click(); await p.waitForTimeout(500); await audit('Library, first run');
  await p.getByRole('button', { name: 'Choose folder…' }).click(); await p.waitForTimeout(500);
  await pc.getByLabel('Select Mingus Ah Um').check(); await p.getByRole('button', { name: /^Add 1 to Pocket/ }).click(); await audit('Library, with pending changes');
  await p.getByRole('button', { name: 'Start sync' }).click(); await audit('Review sheet with the slow-connection warning');
  await p.getByRole('button', { name: 'Back' }).click();
  await pk.getByLabel('Select Blue Train').check(); await p.getByRole('button', { name: 'Edit…' }).click(); await audit('Edit drawer');
  await p.getByRole('button', { name: 'Cancel' }).click();
  await p.getByRole('button', { name: /Tools & settings/ }).click();
  await p.getByRole('button', { name: 'Sync history' }).click(); await p.waitForTimeout(400); await audit('Sync history (empty)');
  await p.evaluate(() => window.__tauMock.seedHistory()); await p.getByRole('button', { name: 'Refresh' }).click(); await p.waitForTimeout(400); await audit('Sync history (with entries)');
  await p.getByRole('button', { name: 'Settings', exact: true }).click(); await p.waitForTimeout(400); await audit('Settings > Library, sync history preferences');
  await p.setViewportSize({ width: 720, height: 450 }); await p.waitForTimeout(300);
  await p.getByRole('button', { name: 'Menu' }).click(); await p.waitForTimeout(300); await audit('Narrow window, menu open');
  await b.close();
  console.log(failures ? `\n${failures} violation type(s)` : '\nno violations');
  process.exit(failures ? 1 : 0);
})();
