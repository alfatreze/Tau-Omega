// Procedural default banner backgrounds for platforms and cores that have no image on disk or in the catalog.
// Inspired by AnaloguePocketWorld's `newsart.js` approach: original vector SVG data URLs with a platform-family
// colour palette, subtle radial glow, fine retro pixel-dot matrix, diagonal scan sheen, and a crisp hardware
// line-art glyph — deterministically varied per platform/core seed so adjacent cards never look identical.

export type PlatformArtInput = {
  id?: string;
  name?: string;
  category?: string;
  manufacturer?: string;
  device?: string;
};

type Family = 'gamebub' | 'handheld' | 'console' | 'arcade' | 'computer' | 'media' | 'fpga';

const FAMILY_HUE: Record<Family, number> = {
  gamebub: 162,   // emerald-teal
  handheld: 266,  // retro indigo-violet
  console: 212,   // cobalt-cerulean
  arcade: 24,     // warm amber-crimson
  computer: 192,  // cyan-slate
  media: 146,     // mint-jade
  fpga: 234,      // cool slate-indigo
};

// Crisp 48x48 line-art hardware silhouettes (drawn at 2.2px stroke, scaled & centered cleanly inside 520x165).
const GLYPH: Record<Family, string> = {
  // Horizontal handheld (Game Bub)
  gamebub:
    '<rect x="4" y="12" width="40" height="24" rx="6"/>' +
    '<rect x="14" y="16" width="20" height="14" rx="2"/>' +
    '<path d="M9.5 20.5v5M7 23h5"/>' +
    '<circle cx="38" cy="21" r="1.5" fill="#fff" stroke="none"/>' +
    '<circle cx="40.5" cy="24.5" r="1.5" fill="#fff" stroke="none"/>' +
    '<path d="M19 33h3M26 33h3"/>',
  // Vertical pocket handheld
  handheld:
    '<rect x="13" y="4" width="22" height="40" rx="4.5"/>' +
    '<rect x="17" y="8" width="14" height="13" rx="1.5"/>' +
    '<path d="M20 31v6M17 34h6"/>' +
    '<circle cx="29" cy="32.5" r="1.7" fill="#fff" stroke="none"/>' +
    '<circle cx="32" cy="35.5" r="1.7" fill="#fff" stroke="none"/>',
  // Home console gamepad
  console:
    '<rect x="5" y="14" width="38" height="20" rx="10"/>' +
    '<path d="M14 20.5v7M10.5 24h7"/>' +
    '<circle cx="32" cy="22" r="1.8" fill="#fff" stroke="none"/>' +
    '<circle cx="36" cy="25.5" r="1.8" fill="#fff" stroke="none"/>' +
    '<path d="M22 26h4"/>',
  // Arcade cabinet
  arcade:
    '<path d="M13 6h22l2 14-3 6v16H14V26l-3-6z"/>' +
    '<rect x="16" y="11" width="16" height="10" rx="1.5"/>' +
    '<path d="M18 26h12M20 23v3"/>' +
    '<circle cx="20" cy="22" r="1.5" fill="#fff" stroke="none"/>' +
    '<circle cx="26" cy="24.5" r="1.1" fill="#fff" stroke="none"/>' +
    '<circle cx="29" cy="24.5" r="1.1" fill="#fff" stroke="none"/>',
  // Classic home computer (CRT + keyboard)
  computer:
    '<rect x="9" y="6" width="30" height="22" rx="3"/>' +
    '<rect x="13" y="10" width="22" height="14" rx="1.5"/>' +
    '<path d="M17 15l3 2.5-3 2.5M22 20h5"/>' +
    '<path d="M6 34h36l2 7H4z"/>' +
    '<path d="M11 37.5h4M18 37.5h12M33 37.5h4"/>',
  // Media / Audio player (VU bars)
  media:
    '<rect x="6" y="10" width="36" height="28" rx="4"/>' +
    '<path d="M13 31V21M18 31V15M23 31V19M28 31V14M33 31V23"/>' +
    '<path d="M11 34h26"/>',
  // FPGA chip
  fpga:
    '<rect x="12" y="12" width="24" height="24" rx="3"/>' +
    '<rect x="18" y="18" width="12" height="12" rx="1"/>' +
    '<path d="M17 6v6M24 6v6M31 6v6M17 36v6M24 36v6M31 36v6M6 17h6M6 24h6M6 31h6M36 17h6M36 24h6M36 31h6"/>',
};

function classifyFamily(info: PlatformArtInput): Family {
  const hay = `${info.id ?? ''} ${info.name ?? ''} ${info.category ?? ''} ${info.manufacturer ?? ''}`.toLowerCase();
  if (/\b(media|music|audio|tau|mp3|flac)\b/.test(hay)) return 'media';
  if (/\b(arcade|cps|neo ?geo|mcr|midway|capcom|namco|konami|toaplan|cave|irems?|shmup)\b/.test(hay)) return 'arcade';
  if (/\b(computer|amiga|minimig|apple ?ii|macintosh|maclc|c64|commodore|msx|zx ?spectrum|amstrad|atari ?st|486|dos|x68000|sharp)\b/.test(hay)) return 'computer';
  if (/\b(handheld|game ?boy|gbc|gba|game ?gear|\bgg\b|lynx|wonderswan|ngpc|arduboy|supervision|pok[eé]mon mini|virtual ?boy)\b/.test(hay)) return 'handheld';
  if (/\b(console|nintendo|nes|snes|super nintendo|n64|mirlo|genesis|mega ?drive|master system|sms|saturn|32x|playstation|psx|3do|pc ?engine|turbografx|coleco|atari|2600|5200|7800|intellivision|vectrex|odyssey|channel ?f)\b/.test(hay)) return 'console';
  if (info.device === 'game_bub' || /\bgame ?bub\b/.test(hay)) return 'gamebub';
  return 'fpga';
}

const cache = new Map<string, string>();

/**
 * Generates a crisp, cached SVG data URL banner (520x165) tailored to the platform's hardware family and seeded
 * deterministically by id/name so adjacent cards have distinct shades and subtle visual motion.
 */
export function platformFallbackArt(info: PlatformArtInput): string {
  const family = classifyFamily(info);
  const seed = `${info.id ?? ''}|${info.name ?? ''}|${family}`;
  const hit = cache.get(seed);
  if (hit) return hit;

  const h = [...seed].reduce((a, ch) => (a * 33 + ch.charCodeAt(0)) >>> 0, 5381);
  const shift = ((h % 9) - 4) * 7; // -28..+28 deg within the family palette
  const hue = (FAMILY_HUE[family] + shift + 360) % 360;
  const hue2 = (hue + 34) % 360;
  const tilt = ((h >>> 4) % 5 - 2) * 3; // -6..+6 deg
  const dx = ((h >>> 8) % 9 - 4) * 5;   // -20..+20 px
  const glowX = 260 + dx * 2;
  const glyph = GLYPH[family];

  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 520 165" preserveAspectRatio="xMidYMid slice">` +
    `<defs>` +
    `<linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">` +
    `<stop offset="0%" stop-color="hsl(${hue} 54% 26%)"/>` +
    `<stop offset="55%" stop-color="hsl(${(hue + 16) % 360} 52% 17%)"/>` +
    `<stop offset="100%" stop-color="hsl(${hue2} 58% 11%)"/>` +
    `</linearGradient>` +
    `<radialGradient id="glow" cx="${glowX}" cy="82" r="170" gradientUnits="userSpaceOnUse">` +
    `<stop offset="0%" stop-color="hsl(${hue} 78% 62%)" stop-opacity="0.28"/>` +
    `<stop offset="100%" stop-color="hsl(${hue} 78% 62%)" stop-opacity="0"/>` +
    `</radialGradient>` +
    `<pattern id="dots" width="10" height="10" patternUnits="userSpaceOnUse">` +
    `<rect width="2" height="2" fill="#fff" opacity="0.09"/>` +
    `</pattern>` +
    `<pattern id="scan" width="4" height="4" patternUnits="userSpaceOnUse">` +
    `<rect width="4" height="1" fill="#000" opacity="0.14"/>` +
    `</pattern>` +
    `</defs>` +
    `<rect width="520" height="165" fill="url(#bg)"/>` +
    `<rect width="520" height="165" fill="url(#glow)"/>` +
    `<rect width="520" height="165" fill="url(#dots)"/>` +
    `<rect width="520" height="165" fill="url(#scan)"/>` +
    `<g transform="translate(${260 + dx} 82) rotate(${tilt}) scale(2.35) translate(-24 -24)" ` +
    `fill="none" stroke="#fff" stroke-opacity="0.86" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${glyph}</g>` +
    `</svg>`;

  const url = 'data:image/svg+xml;utf8,' + encodeURIComponent(svg);
  cache.set(seed, url);
  return url;
}
