import { derived, writable } from 'svelte/store';

export type ThemePreference = 'system' | 'light' | 'dark';
export type ResolvedTheme = 'light' | 'dark';
export type AccentId = 'sage' | 'teal' | 'cobalt' | 'indigo' | 'berry' | 'coral' | 'amber' | 'custom';

export type AccentPreference = {
  id: AccentId;
  customHex: string;
};

export const ACCENT_VAR_KEYS = [
  '--accent-light',
  '--accent-dark',
  '--accent-solid-light',
  '--accent-solid-dark',
  '--accent-on-solid-light',
  '--accent-on-solid-dark',
  '--accent-soft-bg-light',
  '--accent-soft-bg-dark',
  '--accent-soft-border-light',
  '--accent-soft-border-dark',
  '--accent-soft-text-light',
  '--accent-soft-text-dark',
  '--scroll-thumb-light',
  '--scroll-thumb-dark',
] as const;

export type AccentVars = Record<(typeof ACCENT_VAR_KEYS)[number], string>;

export type AccentPreset = {
  id: Exclude<AccentId, 'custom'>;
  label: string;
  swatchLight: string;
  swatchDark: string;
  vars: AccentVars;
};

export const ACCENT_PRESETS: AccentPreset[] = [
  {
    id: 'sage',
    label: 'Sage',
    swatchLight: '#1f6d3d',
    swatchDark: '#c1f0ad',
    vars: {
      '--accent-light': '#1c6438',
      '--accent-dark': '#c1f0ad',
      '--accent-solid-light': '#1f6d3d',
      '--accent-solid-dark': '#c1f0ad',
      '--accent-on-solid-light': '#ffffff',
      '--accent-on-solid-dark': '#142015',
      '--accent-soft-bg-light': '#e1f2e6',
      '--accent-soft-bg-dark': '#1d2c22',
      '--accent-soft-border-light': '#7bb78c',
      '--accent-soft-border-dark': '#3c5d41',
      '--accent-soft-text-light': '#15522c',
      '--accent-soft-text-dark': '#c5f7ad',
      '--scroll-thumb-light': 'rgba(28, 100, 56, 0.45)',
      '--scroll-thumb-dark': 'rgba(193, 240, 173, 0.5)',
    },
  },
  {
    id: 'teal',
    label: 'Teal',
    swatchLight: '#116d7d',
    swatchDark: '#8de4f0',
    vars: {
      '--accent-light': '#0f6270',
      '--accent-dark': '#8de4f0',
      '--accent-solid-light': '#116d7d',
      '--accent-solid-dark': '#8de4f0',
      '--accent-on-solid-light': '#ffffff',
      '--accent-on-solid-dark': '#0c1f24',
      '--accent-soft-bg-light': '#e1f3f5',
      '--accent-soft-bg-dark': '#142a2f',
      '--accent-soft-border-light': '#6eb4bf',
      '--accent-soft-border-dark': '#2f5d66',
      '--accent-soft-text-light': '#0c4f5b',
      '--accent-soft-text-dark': '#a9eef7',
      '--scroll-thumb-light': 'rgba(15, 98, 112, 0.45)',
      '--scroll-thumb-dark': 'rgba(141, 228, 240, 0.5)',
    },
  },
  {
    id: 'cobalt',
    label: 'Blue',
    swatchLight: '#285dc7',
    swatchDark: '#9ec3ff',
    vars: {
      '--accent-light': '#2455b8',
      '--accent-dark': '#9ec3ff',
      '--accent-solid-light': '#285dc7',
      '--accent-solid-dark': '#9ec3ff',
      '--accent-on-solid-light': '#ffffff',
      '--accent-on-solid-dark': '#0e192d',
      '--accent-soft-bg-light': '#e6eefc',
      '--accent-soft-bg-dark': '#182438',
      '--accent-soft-border-light': '#84a6e6',
      '--accent-soft-border-dark': '#38527d',
      '--accent-soft-text-light': '#1d4394',
      '--accent-soft-text-dark': '#bad4ff',
      '--scroll-thumb-light': 'rgba(36, 85, 184, 0.45)',
      '--scroll-thumb-dark': 'rgba(158, 195, 255, 0.5)',
    },
  },
  {
    id: 'indigo',
    label: 'Indigo',
    swatchLight: '#5b49c4',
    swatchDark: '#bfb5ff',
    vars: {
      '--accent-light': '#5241b5',
      '--accent-dark': '#bfb5ff',
      '--accent-solid-light': '#5b49c4',
      '--accent-solid-dark': '#bfb5ff',
      '--accent-on-solid-light': '#ffffff',
      '--accent-on-solid-dark': '#17132e',
      '--accent-soft-bg-light': '#ece9fc',
      '--accent-soft-bg-dark': '#211c38',
      '--accent-soft-border-light': '#9e92e0',
      '--accent-soft-border-dark': '#4b407d',
      '--accent-soft-text-light': '#403194',
      '--accent-soft-text-dark': '#d3ccff',
      '--scroll-thumb-light': 'rgba(82, 65, 181, 0.45)',
      '--scroll-thumb-dark': 'rgba(191, 181, 255, 0.5)',
    },
  },
  {
    id: 'berry',
    label: 'Berry',
    swatchLight: '#a82d6e',
    swatchDark: '#f7a6d0',
    vars: {
      '--accent-light': '#992964',
      '--accent-dark': '#f7a6d0',
      '--accent-solid-light': '#a82d6e',
      '--accent-solid-dark': '#f7a6d0',
      '--accent-on-solid-light': '#ffffff',
      '--accent-on-solid-dark': '#2b111f',
      '--accent-soft-bg-light': '#fae7f0',
      '--accent-soft-bg-dark': '#331a27',
      '--accent-soft-border-light': '#d688ae',
      '--accent-soft-border-dark': '#6e3854',
      '--accent-soft-text-light': '#7d1e50',
      '--accent-soft-text-dark': '#fbc2df',
      '--scroll-thumb-light': 'rgba(153, 41, 100, 0.45)',
      '--scroll-thumb-dark': 'rgba(247, 166, 208, 0.5)',
    },
  },
  {
    id: 'coral',
    label: 'Coral',
    swatchLight: '#c23a29',
    swatchDark: '#ffab9e',
    vars: {
      '--accent-light': '#b33425',
      '--accent-dark': '#ffab9e',
      '--accent-solid-light': '#c23a29',
      '--accent-solid-dark': '#ffab9e',
      '--accent-on-solid-light': '#ffffff',
      '--accent-on-solid-dark': '#2c120e',
      '--accent-soft-bg-light': '#fceae7',
      '--accent-soft-bg-dark': '#341a16',
      '--accent-soft-border-light': '#df9287',
      '--accent-soft-border-dark': '#703a32',
      '--accent-soft-text-light': '#8f2519',
      '--accent-soft-text-dark': '#ffc4bb',
      '--scroll-thumb-light': 'rgba(179, 52, 37, 0.45)',
      '--scroll-thumb-dark': 'rgba(255, 171, 158, 0.5)',
    },
  },
  {
    id: 'amber',
    label: 'Amber',
    swatchLight: '#965d06',
    swatchDark: '#f5cf7f',
    vars: {
      '--accent-light': '#875405',
      '--accent-dark': '#f5cf7f',
      '--accent-solid-light': '#965d06',
      '--accent-solid-dark': '#f5cf7f',
      '--accent-on-solid-light': '#ffffff',
      '--accent-on-solid-dark': '#261b08',
      '--accent-soft-bg-light': '#fcf2dc',
      '--accent-soft-bg-dark': '#2e2412',
      '--accent-soft-border-light': '#d4a957',
      '--accent-soft-border-dark': '#665128',
      '--accent-soft-text-light': '#6b4103',
      '--accent-soft-text-dark': '#fae0a6',
      '--scroll-thumb-light': 'rgba(135, 84, 5, 0.45)',
      '--scroll-thumb-dark': 'rgba(245, 207, 127, 0.5)',
    },
  },
];

const THEME_KEY = 'tau.theme';
const ACCENT_KEY = 'tau.accent';
const ACCENT_VARS_KEY = 'tau.accentVars';
const DEFAULT_CUSTOM_HEX = '#7c3aed';

export function normalizeHex(raw: string): string | null {
  const s = raw.trim().replace(/^#/, '').toLowerCase();
  if (/^[0-9a-f]{3}$/.test(s)) return `#${s[0]}${s[0]}${s[1]}${s[1]}${s[2]}${s[2]}`;
  if (/^[0-9a-f]{6}$/.test(s)) return `#${s}`;
  return null;
}

function hexToRgb(hex: string): [number, number, number] {
  const n = normalizeHex(hex) ?? DEFAULT_CUSTOM_HEX;
  return [
    parseInt(n.slice(1, 3), 16),
    parseInt(n.slice(3, 5), 16),
    parseInt(n.slice(5, 7), 16),
  ];
}

function rgbToHex(r: number, g: number, b: number): string {
  const f = (v: number) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, '0');
  return `#${f(r)}${f(g)}${f(b)}`;
}

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const d = max - min;
  const l = (max + min) / 2;
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
  let h = 0;
  if (d !== 0) {
    if (max === rn) h = ((gn - bn) / d + (gn < bn ? 6 : 0)) * 60;
    else if (max === gn) h = ((bn - rn) / d + 2) * 60;
    else h = ((rn - gn) / d + 4) * 60;
  }
  return [h, s * 100, l * 100];
}

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  const sn = Math.max(0, Math.min(100, s)) / 100;
  const ln = Math.max(0, Math.min(100, l)) / 100;
  const c = (1 - Math.abs(2 * ln - 1)) * sn;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = ln - c / 2;
  let r = 0;
  let g = 0;
  let b = 0;
  const hn = ((h % 360) + 360) % 360;
  if (hn < 60) {
    r = c; g = x; b = 0;
  } else if (hn < 120) {
    r = x; g = c; b = 0;
  } else if (hn < 180) {
    r = 0; g = c; b = x;
  } else if (hn < 240) {
    r = 0; g = x; b = c;
  } else if (hn < 300) {
    r = x; g = 0; b = c;
  } else {
    r = c; g = 0; b = x;
  }
  return [Math.round((r + m) * 255), Math.round((g + m) * 255), Math.round((b + m) * 255)];
}

function relLuminance([r, g, b]: [number, number, number]): number {
  const f = (v: number) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

function contrastRatio(a: [number, number, number], b: [number, number, number]): number {
  const la = relLuminance(a);
  const lb = relLuminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

function findLightnessBelow(h: number, s: number, startL: number, against: [number, number, number], minContrast: number): [number, number, number] {
  for (let l = Math.min(startL, 55); l >= 8; l -= 1) {
    const rgb = hslToRgb(h, s, l);
    if (contrastRatio(rgb, against) >= minContrast) return rgb;
  }
  return hslToRgb(h, s, 12);
}

function findLightnessAbove(h: number, s: number, startL: number, against: [number, number, number], minContrast: number): [number, number, number] {
  for (let l = Math.max(startL, 55); l <= 95; l += 1) {
    const rgb = hslToRgb(h, s, l);
    if (contrastRatio(rgb, against) >= minContrast) return rgb;
  }
  return hslToRgb(h, s, 90);
}

/** Derives accessible Light and Dark design token values from any user-chosen hex color. */
export function deriveCustomAccent(rawHex: string): AccentVars {
  const hex = normalizeHex(rawHex) ?? DEFAULT_CUSTOM_HEX;
  const [r, g, b] = hexToRgb(hex);
  const [h, s, l] = rgbToHsl(r, g, b);

  const white: [number, number, number] = [255, 255, 255];
  const lightCanvas: [number, number, number] = [247, 247, 248];
  const darkSurface: [number, number, number] = [24, 24, 28];
  const darkInk: [number, number, number] = [19, 19, 22];

  const solidLightRgb = findLightnessBelow(h, s, l, white, 4.9);
  const accentLightRgb = findLightnessBelow(h, s, l, lightCanvas, 5.1);
  const softBgLightRgb = hslToRgb(h, Math.min(s, 55), 93);
  const softBorderLightRgb = hslToRgb(h, Math.min(s, 45), 66);
  const softTextLightRgb = findLightnessBelow(h, Math.min(s + 6, 95), 34, softBgLightRgb, 5.6);

  const darkSat = s < 10 ? s : Math.min(Math.max(s, 42), 92);
  const accentDarkRgb = findLightnessAbove(h, darkSat, l, darkSurface, 7.5);
  const darkOnSolidContrastDark = contrastRatio(accentDarkRgb, darkInk);
  const darkOnSolidContrastWhite = contrastRatio(accentDarkRgb, white);
  const onSolidDarkHex = darkOnSolidContrastDark >= darkOnSolidContrastWhite ? '#131316' : '#ffffff';
  const softBgDarkRgb = hslToRgb(h, Math.min(s, 35), 15);
  const softBorderDarkRgb = hslToRgb(h, Math.min(s, 35), 32);
  const softTextDarkRgb = findLightnessAbove(h, Math.min(darkSat, 68), 80, softBgDarkRgb, 7.0);

  return {
    '--accent-light': rgbToHex(...accentLightRgb),
    '--accent-dark': rgbToHex(...accentDarkRgb),
    '--accent-solid-light': rgbToHex(...solidLightRgb),
    '--accent-solid-dark': rgbToHex(...accentDarkRgb),
    '--accent-on-solid-light': '#ffffff',
    '--accent-on-solid-dark': onSolidDarkHex,
    '--accent-soft-bg-light': rgbToHex(...softBgLightRgb),
    '--accent-soft-bg-dark': rgbToHex(...softBgDarkRgb),
    '--accent-soft-border-light': rgbToHex(...softBorderLightRgb),
    '--accent-soft-border-dark': rgbToHex(...softBorderDarkRgb),
    '--accent-soft-text-light': rgbToHex(...softTextLightRgb),
    '--accent-soft-text-dark': rgbToHex(...softTextDarkRgb),
    '--scroll-thumb-light': `rgba(${accentLightRgb[0]}, ${accentLightRgb[1]}, ${accentLightRgb[2]}, 0.45)`,
    '--scroll-thumb-dark': `rgba(${accentDarkRgb[0]}, ${accentDarkRgb[1]}, ${accentDarkRgb[2]}, 0.5)`,
  };
}

function readStoredPreference(): ThemePreference {
  try {
    const raw = localStorage.getItem(THEME_KEY);
    if (raw === 'light' || raw === 'dark') return raw;
  } catch {
    /* localStorage blocked or unavailable */
  }
  return 'system';
}

function readStoredAccent(): AccentPreference {
  try {
    const raw = localStorage.getItem(ACCENT_KEY);
    if (raw && raw.startsWith('custom:')) {
      const hex = normalizeHex(raw.slice(7));
      if (hex) return { id: 'custom', customHex: hex };
    }
    if (raw && ACCENT_PRESETS.some((p) => p.id === raw)) {
      return { id: raw as Exclude<AccentId, 'custom'>, customHex: DEFAULT_CUSTOM_HEX };
    }
  } catch {
    /* localStorage blocked or unavailable */
  }
  return { id: 'sage', customHex: DEFAULT_CUSTOM_HEX };
}

function readSystemTheme(): ResolvedTheme {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return 'dark';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export const themePreference = writable<ThemePreference>(readStoredPreference());
export const systemTheme = writable<ResolvedTheme>(readSystemTheme());
export const accentPreference = writable<AccentPreference>(readStoredAccent());

export const resolvedTheme = derived(
  [themePreference, systemTheme],
  ([$pref, $sys]): ResolvedTheme => ($pref === 'system' ? $sys : $pref),
);

/** Applies the scheme to <meta name="color-scheme"> and syncs native Tauri window chrome when present. */
export function applyThemeMeta(pref: ThemePreference): void {
  if (typeof document !== 'undefined') {
    let meta = document.querySelector('meta[name="color-scheme"]');
    if (!meta) {
      meta = document.createElement('meta');
      meta.setAttribute('name', 'color-scheme');
      document.head.appendChild(meta);
    }
    meta.setAttribute('content', pref === 'system' ? 'light dark' : pref);
  }
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    void import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(pref === 'system' ? null : pref))
      .catch(() => {});
  }
}

export function resolveAccentVars(pref: AccentPreference): AccentVars | null {
  if (pref.id === 'sage') return null;
  if (pref.id === 'custom') return deriveCustomAccent(pref.customHex);
  return ACCENT_PRESETS.find((p) => p.id === pref.id)?.vars ?? null;
}

/** Applies or clears root CSS accent properties. */
export function applyAccent(pref: AccentPreference): AccentVars | null {
  const vars = resolveAccentVars(pref);
  if (typeof document === 'undefined') return vars;
  const root = document.documentElement;
  if (!vars) {
    delete root.dataset.accent;
    for (const key of ACCENT_VAR_KEYS) root.style.removeProperty(key);
    return null;
  }
  root.dataset.accent = pref.id;
  for (const key of ACCENT_VAR_KEYS) {
    root.style.setProperty(key, vars[key]);
  }
  return vars;
}

/** Updates and persists the theme preference ('system' clears storage rather than pinning). */
export function setThemePreference(pref: ThemePreference): void {
  themePreference.set(pref);
  try {
    if (pref === 'system') localStorage.removeItem(THEME_KEY);
    else localStorage.setItem(THEME_KEY, pref);
  } catch {
    /* localStorage blocked; DOM theme still updates */
  }
  applyThemeMeta(pref);
}

/** Selects a built-in primary color preset ('sage' clears storage to default). */
export function setAccentPreset(id: Exclude<AccentId, 'custom'>): void {
  let currentHex = DEFAULT_CUSTOM_HEX;
  accentPreference.update((prev) => {
    currentHex = prev.customHex;
    return { id, customHex: prev.customHex };
  });
  const next: AccentPreference = { id, customHex: currentHex };
  const vars = applyAccent(next);
  try {
    if (id === 'sage' || !vars) {
      localStorage.removeItem(ACCENT_KEY);
      localStorage.removeItem(ACCENT_VARS_KEY);
    } else {
      localStorage.setItem(ACCENT_KEY, id);
      localStorage.setItem(ACCENT_VARS_KEY, JSON.stringify(vars));
    }
  } catch {
    /* storage unavailable */
  }
}

/** Sets and persists a custom primary hex color, deriving Light and Dark accessible tokens. */
export function setCustomAccent(rawHex: string): void {
  const hex = normalizeHex(rawHex) ?? DEFAULT_CUSTOM_HEX;
  const next: AccentPreference = { id: 'custom', customHex: hex };
  accentPreference.set(next);
  const vars = applyAccent(next);
  try {
    localStorage.setItem(ACCENT_KEY, `custom:${hex}`);
    if (vars) localStorage.setItem(ACCENT_VARS_KEY, JSON.stringify(vars));
  } catch {
    /* storage unavailable */
  }
}

let initialized = false;

/** Initializes theme meta sync, primary color vars, OS listener, and cross-tab storage listener. */
export function initTheme(): void {
  if (initialized) return;
  initialized = true;

  const current = readStoredPreference();
  const currentAccent = readStoredAccent();
  themePreference.set(current);
  accentPreference.set(currentAccent);
  systemTheme.set(readSystemTheme());
  applyThemeMeta(current);
  applyAccent(currentAccent);

  if (typeof window === 'undefined') return;

  if (typeof window.matchMedia === 'function') {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onMediaChange = (e: MediaQueryListEvent) => {
      systemTheme.set(e.matches ? 'dark' : 'light');
    };
    mq.addEventListener('change', onMediaChange);
  }

  window.addEventListener('storage', (e) => {
    if (e.key === THEME_KEY || e.key === null) {
      const next = readStoredPreference();
      themePreference.set(next);
      applyThemeMeta(next);
    }
    if (e.key === ACCENT_KEY || e.key === ACCENT_VARS_KEY || e.key === null) {
      const nextAcc = readStoredAccent();
      accentPreference.set(nextAcc);
      applyAccent(nextAcc);
    }
  });
}
