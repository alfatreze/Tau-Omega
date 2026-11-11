#!/usr/bin/env node
// Runs tau-core's own unit tests, built for WebAssembly (wasm32-wasip1), inside Node's WASI host. `cargo check` shows that the engine
// builds for WebAssembly; this shows that it RUNS there (Tau Alpha 016: a build that compiled still aborted at run time, because the scan
// used threads and WebAssembly has none).
//
//   cargo test -p tau-core --lib --target wasm32-wasip1 --no-run
//   node tools/run-wasi-tests.mjs scan::reader_tests          (the arguments are the test filters; none runs everything that can run on WASI)
//
// Not every unit test can run on WASI (many make temporary folders with `std::env::temp_dir()`, which panics there), so CI names the ones
// that are written for it. The host gives the tests an empty `/tmp`, and they run on one thread (WebAssembly has none).
import { WASI } from 'node:wasi';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const deps = path.join(root, 'target', 'wasm32-wasip1', 'debug', 'deps');
const builds = fs.existsSync(deps)
  ? fs.readdirSync(deps).filter((f) => /^tau_core-[0-9a-f]+\.wasm$/.test(f)).map((f) => path.join(deps, f))
  : [];
if (!builds.length) {
  console.error('no tau-core test build found: run `cargo test -p tau-core --lib --target wasm32-wasip1 --no-run` first');
  process.exit(2);
}
// The newest one: older builds of other hashes may be left behind.
const wasm = builds.sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs)[0];
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'tau-wasi-'));
const filters = process.argv.slice(2);
const wasi = new WASI({
  version: 'preview1',
  args: ['tau_core', ...filters, '--test-threads=1', '--nocapture'],
  env: { TMPDIR: '/tmp' },
  preopens: { '/tmp': tmp },
});
try {
  const module = await WebAssembly.compile(fs.readFileSync(wasm));
  const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
  process.exitCode = wasi.start(instance);
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
