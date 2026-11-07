// The meters registry the preset editor works from: Tau Alpha's `meters_schema.json` content, delivered as `lab-data.json` in
// the pinned lab drop (interop R-017/R-025) and installed beside the lab page at `/tau-lab/lab-data.json`. Until a drop that has
// it is pinned the editor says so instead of inventing meters. The shape read here is the schema's own (`meters[]` with
// `id`, `key`, `name`, `params[]`, `presets[]`); the file may carry it at the root or under `schema`, `meters_schema` or `registry`.

export type ParamDef = {
  key: string; label: string; type: 'u8' | 'u16' | 'bool' | 'enum';
  min?: number; max?: number; step?: number; default?: number | boolean; values?: string[];
  help?: string; group?: string; unit?: string; offset?: number; experimental?: boolean;
  /** Only shown when every named parameter currently has this value. */
  when?: Record<string, number | boolean>;
};
export type MeterDef = { id: number; key: string; name: string; params: ParamDef[]; selectable?: boolean; retired?: boolean };
export type Registry = { json: string; meters: MeterDef[] };

const NESTED = ['schema', 'meters_schema', 'registry'];

export function parseRegistry(text: string): Registry {
  let root: any = JSON.parse(text);
  if (!Array.isArray(root?.meters)) root = NESTED.map((k) => root?.[k]).find((v) => Array.isArray(v?.meters));
  if (!root) throw new Error('the lab data has no meters list');
  const meters: MeterDef[] = root.meters
    .filter((m: any) => Array.isArray(m.params) && m.params.length > 0)
    .map((m: any) => ({ id: m.id, key: m.key, name: m.name ?? m.key, params: m.params, selectable: m.selectable, retired: m.retired }));
  return { json: JSON.stringify(root), meters };
}

/** The legal range of a parameter: what the engine and the Pocket both enforce (enum 0..n-1, bool 0..1, colour words the whole 16 bits). */
export function rangeOf(p: ParamDef): [number, number] {
  if (p.type === 'bool') return [0, 1];
  if (p.type === 'enum') return [0, (p.values?.length ?? 1) - 1];
  if (p.type === 'u16' && (p.key.startsWith('custom_') || ['color_green', 'color_yellow', 'color_red'].includes(p.key))) return [0, 65535];
  return [p.min ?? 0, p.max ?? 0];
}
export const defaultOf = (p: ParamDef): number => (p.type === 'bool' ? Number(Boolean(p.default)) : Number(p.default ?? p.min ?? 0));
export const isColour = (p: ParamDef) => p.type === 'u16' && rangeOf(p)[1] === 65535;
export function visible(p: ParamDef, values: Record<string, number>, all: ParamDef[]): boolean {
  if (!p.when) return true;
  return Object.entries(p.when).every(([k, v]) => {
    const dep = all.find((d) => d.key === k);
    return (values[k] ?? (dep ? defaultOf(dep) : 0)) === Number(v);
  });
}

let cached: Promise<Registry | null> | null = null;
/** Loads the registry once: the pinned drop's file, or (dev server only) the real fixture from `testdata/`. `null` when none is installed. */
export function loadRegistry(): Promise<Registry | null> {
  cached ??= (async () => {
    try {
      const r = await fetch('/tau-lab/lab-data.json');
      if (r.ok && !(r.headers.get('content-type') ?? '').includes('text/html')) return parseRegistry(await r.text());
    } catch { /* not installed */ }
    if (import.meta.env.DEV) {
      try { return parseRegistry((await import('../../../../testdata/preset_files/meters_schema.json?raw')).default); } catch { /* none */ }
    }
    return null;
  })();
  return cached;
}
