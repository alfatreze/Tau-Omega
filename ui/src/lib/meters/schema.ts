// Meter manifest shape and a synthetic (not real) registry, scaffolding the
// UI structure ahead of tau-alpha's own meter-module work.
//
// STATUS (2026-09-26): tau-alpha's `docs/METER_MODULE_SPEC.md` fully designs
// this interface (decisions D-M01-M13 already resolved), but as of this
// writing tau-alpha is only mid-M0 of its own build order -- no tagged
// release contains a real `tools/meters_schema.json`, and that project's own
// rule (section 6) is that Omega reads it "from a tagged release, verified
// by hash", never a live working tree. Building against nothing real yet
// would repeat the exact fixture-vs-reality mistake this project has already
// been bitten by twice (`docs/FIRMWARE_SYNC.md`).
//
// So: everything in this file is a PLACEHOLDER, by explicit owner decision
// (2026-09-26) -- scaffold the UI structure and preview plumbing now, swap
// this file's contents for the real generated schema the moment tau-alpha
// tags a release containing `meters_schema.json`. The three sample meters
// below are not invented from nothing: their parameter shapes are drawn from
// tau-alpha's own real, hardware-shipped `wviz_bars_cfg_t`/`wviz_scope_cfg_t`
// fields (Preset/Mode/Bands/Easing/Attack/Release/Peak Cap/Peak Fall
// style/Peak Hold/Peak Fall Speed/Scope Smooth/Scope Trail, per that
// project's own audit trail), so the editor and preview are exercised
// against a realistic shape rather than an arbitrary one -- but the exact
// field names, ids, ranges and defaults are still a guess and MUST be
// re-verified against the real manifest before this app trusts them for
// anything written to a card.

export type MeterParamType = 'u8' | 'u16' | 'bool' | 'enum';

export type MeterParam = {
  key: string;
  label: string;
  type: MeterParamType;
  min?: number;
  max?: number;
  step?: number;
  default: number | boolean;
  values?: string[]; // for type: 'enum'
  unit?: string;
  /** Only shown/enabled when every named param currently equals its value. */
  when?: Record<string, number | boolean>;
};

export type MeterPreset = {
  name: string;
  values: Record<string, number | boolean>;
};

/** Mirrors `docs/METER_MODULE_SPEC.md` section 4's manifest shape exactly,
 * field for field, so swapping in the real `meters_schema.json` later is a
 * data change, not a UI rewrite. */
export type MeterSchema = {
  schema: number;
  key: string;
  id: number;
  name: string;
  flags: string[];
  cost_class: number;
  params: MeterParam[];
  presets: MeterPreset[];
  roles_used: string[];
};

const EASING_VALUES = ['INSTANT', 'LINEAR', 'EXPONENTIAL', 'SPRING'];
const PEAK_FALL_VALUES = ['LINEAR', 'GRAVITY'];

export const SYNTHETIC_METERS: MeterSchema[] = [
  {
    schema: 1,
    key: 'winamp_bars',
    id: 12,
    name: 'WINAMP BARS',
    flags: ['selectable', 'needs_spec'],
    cost_class: 0,
    params: [
      { key: 'bands', label: 'Bands', type: 'u8', min: 4, max: 16, step: 1, default: 16 },
      { key: 'ease', label: 'Easing', type: 'enum', values: EASING_VALUES, default: 2 },
      { key: 'attack_ms', label: 'Attack', type: 'u16', min: 0, max: 200, step: 5, default: 20, unit: 'ms' },
      { key: 'release_ms', label: 'Release', type: 'u16', min: 0, max: 800, step: 10, default: 220, unit: 'ms' },
      { key: 'peak_on', label: 'Peak cap', type: 'bool', default: true },
      { key: 'peak_fall', label: 'Peak fall style', type: 'enum', values: PEAK_FALL_VALUES, default: 1, when: { peak_on: true } },
      { key: 'hold_ms', label: 'Peak hold', type: 'u16', min: 0, max: 800, step: 10, default: 300, unit: 'ms', when: { peak_on: true } },
    ],
    presets: [
      { name: 'FLUID', values: { bands: 16, ease: 2, attack_ms: 20, release_ms: 220, peak_on: true, peak_fall: 1, hold_ms: 300 } },
      { name: 'SNAPPY', values: { bands: 16, ease: 0, attack_ms: 5, release_ms: 80, peak_on: true, peak_fall: 0, hold_ms: 150 } },
      { name: 'CALM', values: { bands: 8, ease: 3, attack_ms: 60, release_ms: 500, peak_on: false, peak_fall: 1, hold_ms: 300 } },
    ],
    roles_used: ['accent', 'accent-2', 'surface-track'],
  },
  {
    schema: 1,
    key: 'winamp_scope',
    id: 13,
    name: 'WINAMP SCOPE',
    flags: ['selectable', 'needs_spec'],
    cost_class: 0,
    params: [
      { key: 'smooth', label: 'Scope smoothing', type: 'u8', min: 0, max: 100, step: 5, default: 40, unit: '%' },
      { key: 'trail', label: 'Trail fade', type: 'u8', min: 0, max: 100, step: 5, default: 25, unit: '%' },
    ],
    presets: [
      { name: 'CLEAN', values: { smooth: 20, trail: 10 } },
      { name: 'DREAMY', values: { smooth: 70, trail: 60 } },
    ],
    roles_used: ['accent', 'surface-track'],
  },
  {
    schema: 1,
    key: 'chladni',
    id: 14,
    name: 'CHLADNI',
    flags: ['selectable'],
    cost_class: 1,
    params: [
      { key: 'template', label: 'Template', type: 'enum', values: ['CROSS', 'GRID', 'RADIAL', 'SPIRAL', 'PETALS'], default: 0 },
      { key: 'tiles', label: 'Tiling', type: 'u8', min: 1, max: 6, step: 1, default: 4 },
      { key: 'fps', label: 'Frame rate', type: 'u8', min: 5, max: 30, step: 1, default: 15, unit: 'fps' },
    ],
    presets: [
      { name: 'DEFAULT', values: { template: 0, tiles: 4, fps: 15 } },
    ],
    roles_used: ['accent', 'accent-2'],
  },
];

export function defaultValues(meter: MeterSchema): Record<string, number | boolean> {
  const values: Record<string, number | boolean> = {};
  for (const param of meter.params) values[param.key] = param.default;
  return values;
}

/** Whether `param` should currently be shown, per its own `when` clause. */
export function paramVisible(param: MeterParam, values: Record<string, number | boolean>): boolean {
  if (!param.when) return true;
  return Object.entries(param.when).every(([key, expected]) => values[key] === expected);
}
