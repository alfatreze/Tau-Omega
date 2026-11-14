#!/usr/bin/env node
// Runs tau-core's own unit tests, built for WebAssembly (wasm32-wasip1), inside Node's WASI host. `cargo check` shows that the engine
// builds for WebAssembly; this shows that it RUNS there (Tau Alpha 016: a build that compiled still aborted at run time, because the scan
// used threads and WebAssembly has none).
//
//   node tools/run-wasi-tests.mjs scan::reader_tests          (the arguments are the test filters; none runs everything that can run on WASI)
//
// Not every unit test can run on WASI (many make temporary folders with `std::env::temp_dir()`, which panics there), so CI names the ones
// that are written for it. The host gives the tests an empty `/tmp`, and they run on one thread (WebAssembly has none).
import { spawnSync } from 'node:child_process';
import { WASI } from 'node:wasi';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');

// Where cargo put the test build. Asked of cargo itself (its JSON messages name the executable), because the folder differs between
// cargo versions: older ones use target/<triple>/debug/deps, newer ones target/<triple>/debug/build/<crate>/<hash>/out (CI's newer
// stable did, on 2026-11-14, and a runner that guessed the folder found nothing). Nothing is rebuilt when the build is already there.
// TAU_CORE_WASM names a file to use instead.
function findBuild() {
  if (process.env.TAU_CORE_WASM) return process.env.TAU_CORE_WASM;
  const r = spawnSync('cargo', ['test', '-p', 'tau-core', '--lib', '--locked', '--target', 'wasm32-wasip1', '--no-run', '--message-format=json'],
    { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  if (r.status !== 0) { console.error(r.stderr || 'cargo could not build the tau-core tests for wasm32-wasip1'); process.exit(2); }
  const found = r.stdout.split('\n').filter((l) => l.startsWith('{')).map((l) => { try { return JSON.parse(l); } catch { return null; } })
    .filter((m) => m?.reason === 'compiler-artifact' && m.profile?.test && m.target?.name === 'tau_core' && m.executable);
  return found.at(-1)?.executable;
}
const wasm = findBuild();
if (!wasm || !fs.existsSync(wasm)) {
  console.error('no tau-core test build found: `cargo test -p tau-core --lib --target wasm32-wasip1 --no-run` did not report one');
  process.exit(2);
}
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
