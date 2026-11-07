// Provenance of the lab's WebAssembly modules (Tau Alpha message 009, Q-035: checked, not parsed out of the JavaScript).
// The pinned drop's MANIFEST.json lists every module with the SHA-256 of its compiled `.wasm` and how it was built; the lab page reports
// the hash of each module it actually compiled (`tau-lab-ready`). They must agree: a module that does not match is not the one Tau Alpha
// recorded, and the lab preview should not be trusted as "the firmware's drawing code" for that meter.
import type { LabModule } from './labBus';

export type ManifestModule = { module: string; wasm_sha256: string; wasm_bytes?: number; clang?: string; built_from?: Record<string, string> };
export type Manifest = { tau_alpha_commit?: string; modules: ManifestModule[] };
export type ModuleCheck = {
  /** The module's file stem, e.g. `winamp_bars` or `layered_wave_timed`. */
  name: string;
  status: 'match' | 'mismatch' | 'unlisted';
  got: string;
  expected?: string;
  clang?: string;
  sources?: number;
};
export type Provenance = {
  commit: string;
  checks: ModuleCheck[];
  /** Modules the manifest lists that the lab did not report. */
  notReported: string[];
  /** Every reported module matches and nothing listed is missing. */
  ok: boolean;
};

/** `preview/meters/winamp_bars_wasm.js` -> `winamp_bars`. */
export function moduleName(path: string): string {
  const file = path.split('/').pop() ?? path;
  return file.replace(/_wasm\.js$/, '');
}

/** Reads a manifest, or null when it is not one (a dev server answers an unknown path with the app's own page). */
export function parseManifest(value: unknown): Manifest | null {
  if (!value || typeof value !== 'object') return null;
  const mods = (value as { modules?: unknown }).modules;
  if (!Array.isArray(mods)) return null;
  const modules = mods.filter((m): m is ManifestModule => !!m && typeof m.module === 'string' && typeof m.wasm_sha256 === 'string');
  if (!modules.length) return null;
  const commit = (value as { tau_alpha_commit?: unknown }).tau_alpha_commit;
  return { tau_alpha_commit: typeof commit === 'string' ? commit : undefined, modules };
}

/**
 * Compares the modules the lab reported with the manifest. The lab names a module by its meter key (`key`); Layered Wave also reports the
 * hash of its timed variant (`wasm_sha256_timed`), which the manifest lists as `layered_wave_timed`.
 */
export function checkProvenance(reported: (LabModule & { wasm_sha256_timed?: string })[], manifest: Manifest): Provenance {
  const listed = new Map(manifest.modules.map((m) => [moduleName(m.module), m]));
  const seen = new Set<string>();
  const checks: ModuleCheck[] = [];
  const add = (name: string, got: string) => {
    seen.add(name);
    const m = listed.get(name);
    checks.push(
      !m
        ? { name, status: 'unlisted', got }
        : {
            name,
            status: m.wasm_sha256.toLowerCase() === got.toLowerCase() ? 'match' : 'mismatch',
            got,
            expected: m.wasm_sha256,
            clang: m.clang,
            sources: m.built_from ? Object.keys(m.built_from).length : undefined,
          },
    );
  };
  for (const r of reported) {
    add(r.key, r.wasm_sha256);
    if (r.wasm_sha256_timed) add(`${r.key}_timed`, r.wasm_sha256_timed);
  }
  const notReported = [...listed.keys()].filter((n) => !seen.has(n)).sort();
  return { commit: manifest.tau_alpha_commit ?? '', checks, notReported, ok: checks.every((c) => c.status === 'match') && notReported.length === 0 };
}
