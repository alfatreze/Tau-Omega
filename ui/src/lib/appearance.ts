// Appearance: theme editor model. The role list, the order and the 18 default colours mirror tau-alpha's
// `themes/tau.json` and the firmware's role table (`fw/theme.h`); the engine (`tau_core::assets`) is the authority for
// what a valid theme is, this only gives the editor something sensible to start from and the labels to show.
import type { PolarityInput, ThemeInput } from './types';

export type RoleGroup = { title: string; roles: { key: string; label: string; hint: string }[] };

export const ROLE_GROUPS: RoleGroup[] = [
  { title: 'Background and panels', roles: [
    { key: 'bg_bottom', label: 'Background (bottom)', hint: 'Bottom of the background gradient. The top comes from the accent colour.' },
    { key: 'surface', label: 'Panel', hint: 'Cards, rows and menus.' },
    { key: 'surface_track', label: 'Track', hint: 'Unfilled part of sliders and progress bars.' },
    { key: 'base', label: 'Base', hint: 'Behind panels and overlays.' },
    { key: 'chrome', label: 'Header and action bar', hint: 'Menu headers and the bottom action bar.' },
  ] },
  { title: 'Text', roles: [
    { key: 'text_primary', label: 'Main text', hint: 'Titles and values. Must be easy to read on every panel and accent.' },
    { key: 'text_secondary', label: 'Secondary text', hint: 'Artist, labels and hints.' },
    { key: 'faint', label: 'Faint text', hint: 'The file name line.' },
    { key: 'on_accent', label: 'Text on accent', hint: 'Text drawn on a filled accent colour.' },
  ] },
  { title: 'Status', roles: [
    { key: 'ok', label: 'OK', hint: 'Pass and good states.' },
    { key: 'warn', label: 'Warning', hint: 'Caution states.' },
    { key: 'danger', label: 'Danger', hint: 'Top of the level meter ladder.' },
    { key: 'error', label: 'Error', hint: 'Failure text.' },
  ] },
  { title: 'Other', roles: [
    { key: 'pill', label: 'Preset pill', hint: 'The small preset tag on the now-playing screen.' },
    { key: 'splash_bg', label: 'Splash background', hint: 'Start-up screen.' },
    { key: 'splash_bar', label: 'Splash bar', hint: 'Start-up loading bar.' },
    { key: 'fs_red', label: 'Fullscreen progress (paused)', hint: 'Fullscreen meter progress line while stopped or paused.' },
    { key: 'fs_track', label: 'Fullscreen track', hint: 'Unplayed part of the fullscreen progress line.' },
  ] },
];
export const ROLE_KEYS = ROLE_GROUPS.flatMap((g) => g.roles.map((r) => r.key));

const DARK: PolarityInput = { bg_luma: 45, colors: {
  bg_bottom: '#000000', surface: '#292829', surface_track: '#181C18', text_primary: '#FFFFFF', text_secondary: '#949594',
  on_accent: '#000000', ok: '#00C200', warn: '#FFCE00', danger: '#FF3800', base: '#080C10', chrome: '#101C29', pill: '#006500',
  error: '#FF0000', faint: '#6A696A', splash_bg: '#080808', splash_bar: '#20FF62', fs_red: '#F6484A', fs_track: '#182029' } };
const LIGHT: PolarityInput = { bg_luma: 205, colors: {
  bg_bottom: '#EEF2EE', surface: '#FFFFFF', surface_track: '#CDD2DE', text_primary: '#101820', text_secondary: '#525562',
  on_accent: '#FFFFFF', ok: '#107931', warn: '#B46900', danger: '#C52018', base: '#E6E6EE', chrome: '#DEE2E6', pill: '#BDE6C5',
  error: '#C52018', faint: '#626573', splash_bg: '#EEF2EE', splash_bar: '#109D6A', fs_red: '#D53439', fs_track: '#C5CED5' } };

/** A new theme starts as a copy of the built-in TAU theme, so every role has a sensible value to adjust. */
export function newTheme(name: string): ThemeInput {
  return { name, dark: { bg_luma: DARK.bg_luma, colors: { ...DARK.colors } }, light: { bg_luma: LIGHT.bg_luma, colors: { ...LIGHT.colors } } };
}
export const MAX_THEMES = 4;
export const BUILTIN_NAMES = ['TAU', 'OCEAN'];
