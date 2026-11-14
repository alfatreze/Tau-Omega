// The feedback rules: what may be attached and what is missing before sending. Pure logic.
// Usage: node scripts/feedback-logic-test.cjs
const assert = require('node:assert/strict');
let passed = 0; const ok = (n) => { passed++; console.log(`ok - ${n}`); };
(async () => {
  const f = await import('../src/lib/feedback.ts');
  const { checkFile, problems, githubOk, emptyDraft, LIMITS } = f;
  const MB = 1024 * 1024;
  const pic = (name, size = 1000, type = 'image/png') => ({ name, size, type });

  assert.equal(checkFile(pic('shot.png'), []), null); assert.equal(checkFile(pic('Shot.JPG', 10, 'image/jpeg'), []), null);
  assert.equal(checkFile(pic('run.log', 10, ''), []), null); assert.equal(checkFile(pic('report.zip', 10, 'application/x-zip-compressed'), []), null); ok('pictures, text, logs and a zip can be attached (any letter case, with or without a type from the system)');
  assert.match(checkFile(pic('tool.exe', 10, ''), []), /not a type we can take[\s\S]*png, jpg, gif, webp/); assert.match(checkFile(pic('photo.heic', 10, ''), []), /not a type/); assert.match(checkFile(pic('noext', 10, ''), []), /not a type/); ok('other types are refused and the message lists what is allowed');
  assert.match(checkFile(pic('pic.png', 10, 'application/x-msdownload'), []), /does not look like a \.png/); ok('a file whose own type disagrees with its name is refused');
  assert.match(checkFile(pic('big.png', 6 * MB), []), /big\.png is 6\.0 MB\. Each file can be up to 5\.0 MB/); assert.match(checkFile(pic('empty.png', 0), []), /is empty/); assert.equal(checkFile(pic('edge.png', 5 * MB), []), null); ok('a file over 5 MB (or empty) is refused, exactly 5 MB is fine');
  const four = [pic('a.png'), pic('b.png'), pic('c.png'), pic('d.png')];
  assert.match(checkFile(pic('e.png'), four), /up to 4 files/); assert.match(checkFile(pic('a.png'), [pic('a.png')]), /already attached/); assert.equal(checkFile(pic('a.png', 2000), [pic('a.png')]), null); ok('at most 4 files, and the same file is not added twice');
  assert.match(checkFile(pic('c.png', 5 * MB), [pic('a.png', 5 * MB), pic('b.png', 5 * MB)]), /would pass 12\.0 MB in total/); assert.equal(checkFile(pic('c.png', 2 * MB), [pic('a.png', 5 * MB), pic('b.png', 5 * MB)]), null); ok('the total stays within 12 MB');

  assert.ok(['jpg', 'jpeg', 'png', 'gif', 'webp'].every((e) => f.isImage('x.' + e)) && !f.isImage('x.txt') && !f.isImage('x.zip')); ok('pictures are told apart from other files (they are shown as small pictures)');
  assert.deepEqual([githubOk(''), githubOk('alfatreze'), githubOk('a-b'), githubOk('-a'), githubOk('a-'), githubOk('a--b'), githubOk('a b'), githubOk('x'.repeat(40)), githubOk('@me')], [true, true, true, false, false, false, false, false, false]); ok('a GitHub name is checked (optional, letters, digits, single hyphens, up to 39)');

  const d = { ...emptyDraft(), title: 'Marketplace crash', text: 'It closes when I open a core.' };
  assert.deepEqual(problems(d, 0), []); ok('a titled and described message is ready');
  assert.deepEqual(problems(emptyDraft(), 0).length, 2); assert.match(problems({ ...d, title: '  ' }, 0)[0], /short title/); assert.match(problems({ ...d, text: 'too short' }, 0)[0], /a little more/); assert.match(problems({ ...d, github: 'no good' }, 0)[0], /GitHub name/); ok('each missing piece is named in plain words');

  assert.equal(LIMITS.files, 4); assert.equal(f.osName('macos'), 'macOS'); assert.equal(f.osName('freebsd'), 'freebsd'); ok('the limits and system names are as the page shows them');
  console.log(`\n${passed} checks passed`);
})().catch((e) => { console.error('FAILED:', e.message); process.exit(1); });
