// Game Boy colour palettes: index loader (`/palettes.json`, from openfpga-palettes), 56-byte `.pal` parser,
// colour-family classifier, and three original 160x144 Game Boy sample screens (Action, Adventure, Puzzle)
// drawn across all four Pocket palette layers (BG, OBJ0, OBJ1, WIN). Adapted from AnaloguePocketWorld.

export const PALETTE_DIR = 'Assets/gb/common/Palettes';

export type PaletteIndexEntry = { p: string; g: number; b: string };
export type PaletteIndex = { generated?: string; repo?: string; release?: string; groups: string[]; palettes: PaletteIndexEntry[] };
export type Rgb = [number, number, number];
export type ParsedPal = { layers: [Rgb[], Rgb[], Rgb[], Rgb[]]; off: Rgb };

let indexPromise: Promise<PaletteIndex> | null = null;
export const loadPalettes = (): Promise<PaletteIndex> =>
  (indexPromise ||= fetch('/palettes.json')
    .then((r) => (r.ok ? r.json() : { groups: [], palettes: [] }))
    .catch(() => ({ groups: [], palettes: [] })));

export function fromHex(hex: string): Uint8Array {
  const pairs = hex.match(/../g) ?? [];
  return Uint8Array.from(pairs.map((h) => parseInt(h, 16)));
}

const trio = (b: Uint8Array, o: number): Rgb => [b[o] ?? 0, b[o + 1] ?? 0, b[o + 2] ?? 0];

export function parsePal(bytes: Uint8Array): ParsedPal {
  const layer = (o: number): Rgb[] => [0, 1, 2, 3].map((shade) => trio(bytes, o + (3 - shade) * 3));
  return { layers: [layer(0), layer(12), layer(24), layer(36)], off: trio(bytes, 48) };
}

export const rgb = (c: Rgb): string => `rgb(${c[0]},${c[1]},${c[2]})`;

export const FAMILIES = [
  { id: 'green', label: 'Green', swatch: '#6aa84f' },
  { id: 'teal', label: 'Teal', swatch: '#3fb6b2' },
  { id: 'blue', label: 'Blue', swatch: '#4a6fe3' },
  { id: 'purple', label: 'Purple', swatch: '#8e5bd1' },
  { id: 'pink', label: 'Pink', swatch: '#e86aa8' },
  { id: 'red', label: 'Red', swatch: '#d9473f' },
  { id: 'orange', label: 'Orange', swatch: '#ee8a2c' },
  { id: 'yellow', label: 'Yellow', swatch: '#e6c82e' },
  { id: 'brown', label: 'Brown', swatch: '#8a5a3b' },
  { id: 'grey', label: 'Grey', swatch: '#8d8d8d' },
  { id: 'multi', label: 'Multicolour', swatch: 'conic-gradient(#d9473f,#e6c82e,#6aa84f,#4a6fe3,#d9473f)' },
] as const;

function hsl([r0, g0, b0]: Rgb) {
  const r = r0 / 255, g = g0 / 255, b = b0 / 255;
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b), l = (mx + mn) / 2, d = mx - mn;
  let h = 0;
  if (d) {
    h = mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
    h = (h * 60 + 360) % 360;
  }
  return { h, s: d ? d / (1 - Math.abs(2 * l - 1)) : 0, l };
}

export function describe(pal: ParsedPal) {
  const bg = pal.layers[0];
  const mids = [hsl(bg[1]), hsl(bg[2])];
  const pick = mids[0].s >= mids[1].s ? mids[0] : mids[1];
  const lum = bg.reduce((n, c) => n + (0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]) / 255, 0) / 4;
  const hueGap = Math.min(Math.abs(mids[0].h - mids[1].h), 360 - Math.abs(mids[0].h - mids[1].h));
  let family: string;
  if (Math.max(mids[0].s, mids[1].s) < 0.14) family = 'grey';
  else if (mids[0].s > 0.28 && mids[1].s > 0.28 && hueGap > 70) family = 'multi';
  else {
    const h = pick.h;
    family = h >= 345 || h < 15 ? 'red'
      : h < 40 ? (pick.l < 0.42 ? 'brown' : 'orange')
      : h < 65 ? (pick.l < 0.4 ? 'brown' : 'yellow')
      : h < 165 ? 'green'
      : h < 200 ? 'teal'
      : h < 260 ? 'blue'
      : h < 300 ? 'purple'
      : 'pink';
  }
  return { family, hue: pick.h, lum, tone: lum > 0.6 ? 'light' : lum < 0.36 ? 'dark' : 'medium' };
}

export const nameOf = (path: string): string => (path.split('/').pop() ?? path).replace(/\.pal$/i, '');

const W = 160, H = 144;
const BG = 0, O0 = 1, O1 = 2, WIN = 3;
const FONT: Record<string, string> = {
  0: '111101101101111', 1: '010110010010111', 2: '111001111100111', 3: '111001111001111', 4: '101101111001001',
  5: '111100111001111', 6: '111100111101111', 7: '111001001001001', 8: '111101111101111', 9: '111101111001111',
  S: '111100111001111', C: '111100100100111', O: '111101101101111', R: '110101110101101', E: '111100110100111',
  L: '100100100100111', I: '111010010010111', N: '111101101101101', V: '101101101101010',
  T: '111010010010010', H: '101101111101101', P: '111101111100100', A: '010101111101101', X: '101101010101101',
};

function canvasOf() {
  const px = new Uint8Array(W * H);
  const put = (layer: number, shade: number, x: number, y: number) => {
    if (x >= 0 && y >= 0 && x < W && y < H) px[y * W + x] = (layer << 2) | shade;
  };
  return {
    px,
    rect: (l: number, s: number, x: number, y: number, w: number, h: number) => {
      for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) put(l, s, x + i, y + j);
    },
    dither: (l: number, a: number, b: number, x: number, y: number, w: number, h: number) => {
      for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) put(l, (i + j) & 1 ? a : b, x + i, y + j);
    },
    sprite: (l: number, x: number, y: number, rows: string[]) =>
      rows.forEach((row, j) => [...row].forEach((ch, i) => { if (ch !== '.') put(l, +ch, x + i, y + j); })),
    text: (l: number, s: number, x: number, y: number, str: string) => {
      let cx = x;
      for (const ch of str) {
        const g = FONT[ch];
        if (g) for (let k = 0; k < 15; k++) if (g[k] === '1') put(l, s, cx + (k % 3), y + Math.floor(k / 3));
        cx += 4;
      }
    },
    blob: (l: number, s: number, cx: number, cy: number, rx: number, ry: number) => {
      for (let j = -ry; j <= ry; j++) for (let i = -rx; i <= rx; i++) if ((i * i) / (rx * rx) + (j * j) / (ry * ry) <= 1) put(l, s, cx + i, cy + j);
    },
  };
}

const HERO = ['..3333..', '.322223.', '.300003.', '.303033.', '..3003..', '.322223.', '.322223.', '..3..3..'];
const BLOB = ['..3333..', '.311113.', '31311313', '31111113', '33111133', '.311113.', '.3.33.3.', '..3..3..'];
const COIN = ['.3333.', '321123', '321123', '321123', '321123', '.3333.'];
const HEART = ['.33.33.', '3223223', '3222223', '.32223.', '..323..', '...3...'];

export type SceneId = 'action' | 'adventure' | 'puzzle';

const SCENES: Record<SceneId, () => Uint8Array> = {
  action: () => {
    const c = canvasOf();
    c.rect(BG, 0, 0, 0, W, H);
    c.blob(BG, 1, 28, 38, 14, 5); c.blob(BG, 1, 40, 34, 10, 6); c.blob(BG, 1, 112, 52, 16, 5); c.blob(BG, 1, 124, 48, 10, 6);
    c.blob(BG, 1, 34, 112, 44, 18); c.blob(BG, 1, 120, 112, 52, 24);
    c.dither(BG, 1, 0, 0, 96, W, 16);
    c.rect(BG, 2, 0, 112, W, 32); c.rect(BG, 1, 0, 112, W, 4); for (let x = 0; x < W; x += 8) c.rect(BG, 3, x + 2, 114, 2, 2);
    c.dither(BG, 3, 2, 0, 120, W, 24);
    c.rect(BG, 3, 64, 76, 48, 8); c.rect(BG, 2, 65, 77, 46, 6); for (let x = 64; x < 112; x += 12) c.rect(BG, 3, x, 76, 1, 8); c.rect(BG, 3, 64, 80, 48, 1);
    c.rect(BG, 3, 78, 52, 10, 10); c.rect(BG, 1, 80, 54, 6, 6); c.rect(BG, 3, 82, 56, 2, 2);
    c.rect(BG, 3, 128, 92, 16, 20); c.rect(BG, 2, 130, 94, 12, 18); c.rect(BG, 3, 126, 92, 20, 4);
    c.sprite(O0, 24, 104, HERO); c.sprite(O1, 98, 104, BLOB); c.sprite(O1, 82, 64, COIN); c.sprite(O1, 68, 62, COIN); c.sprite(O1, 104, 40, COIN);
    c.rect(WIN, 0, 0, 0, W, 16); c.rect(WIN, 3, 0, 15, W, 1); c.text(WIN, 3, 6, 5, 'SCORE 000420'); c.sprite(WIN, 126, 4, HEART); c.sprite(WIN, 136, 4, HEART); c.sprite(WIN, 146, 4, HEART);
    return c.px;
  },
  adventure: () => {
    const c = canvasOf();
    c.rect(BG, 1, 0, 0, W, 100); c.dither(BG, 1, 0, 0, 0, W, 100);
    for (let i = 0; i < 24; i++) { const x = (i * 37) % 152, y = (i * 53) % 90; c.rect(BG, 2, x, y, 2, 1); }
    for (let y = 0; y < 100; y++) { const x = Math.round(66 + Math.sin(y / 11) * 14); c.rect(BG, 0, x, y, 14, 1); c.rect(BG, 1, x - 1, y, 1, 1); c.rect(BG, 1, x + 14, y, 1, 1); }
    c.blob(BG, 2, 128, 22, 20, 10); c.blob(BG, 3, 128, 22, 14, 6); c.blob(BG, 1, 124, 20, 5, 2);
    for (const [tx, ty] of [[12, 14], [34, 60], [96, 70], [20, 82], [140, 62], [100, 10]]) { c.rect(BG, 3, tx + 6, ty + 14, 4, 8); c.blob(BG, 3, tx + 8, ty + 8, 10, 9); c.blob(BG, 2, tx + 7, ty + 7, 7, 6); c.blob(BG, 1, tx + 5, ty + 5, 3, 2); }
    c.rect(BG, 0, 30, 18, 34, 22); c.rect(BG, 3, 28, 12, 38, 8); c.rect(BG, 2, 30, 14, 34, 4); c.rect(BG, 3, 42, 28, 8, 12); c.rect(BG, 1, 34, 24, 6, 6); c.rect(BG, 1, 54, 24, 6, 6); c.rect(BG, 3, 30, 40, 34, 1);
    c.sprite(O0, 72, 60, HERO); c.sprite(O1, 92, 56, BLOB);
    c.rect(WIN, 3, 4, 102, 152, 38); c.rect(WIN, 0, 6, 104, 148, 34); c.text(WIN, 3, 12, 110, 'HELLO TRAVELER'); c.text(WIN, 3, 12, 120, 'HP 12   LV 03'); c.rect(WIN, 3, 140, 128, 6, 1); c.rect(WIN, 3, 143, 130, 1, 1);
    return c.px;
  },
  puzzle: () => {
    const c = canvasOf();
    c.dither(BG, 1, 0, 0, 0, W, H);
    const block = (x: number, y: number, kind: number) => { c.rect(BG, 3, x, y, 8, 8); if (kind === 0) c.rect(BG, 2, x + 1, y + 1, 6, 6); else if (kind === 1) { c.rect(BG, 0, x + 1, y + 1, 6, 6); c.rect(BG, 3, x + 3, y + 3, 2, 2); } else c.dither(BG, 3, 1, x + 1, y + 1, 6, 6); };
    c.rect(BG, 3, 30, 4, 84, 136); c.rect(BG, 0, 34, 8, 76, 128);
    for (let x = 34; x < 110; x += 8) c.rect(BG, 1, x, 8, 1, 128); for (let y = 8; y < 136; y += 8) c.rect(BG, 1, 34, y, 76, 1);
    const rows = [[2, 2, 0, 1, 1, 0, 2, 2, 2], [0, 1, 1, 2, 0, 0, 1, 2, 0], [1, 1, 2, 2, 0, 1, 0, 0, 2], [2, 0, 0, 1, 2, 2, 1, 1, 0]];
    rows.forEach((r, j) => r.forEach((k, i) => block(34 + i * 8, 104 + j * 8, k)));
    [[2, 0], [2, 1], [3, 1], [3, 2]].forEach(([i, j]) => block(34 + (i + 2) * 8, 16 + j * 8 + 24, 1));
    c.sprite(O0, 40, 24, ['.33333.', '3211123', '3211123', '.33333.']);
    c.rect(WIN, 3, 118, 4, 40, 136); c.rect(WIN, 0, 120, 6, 36, 132);
    c.text(WIN, 3, 124, 12, 'SCORE'); c.text(WIN, 3, 124, 20, '000420'); c.text(WIN, 3, 124, 38, 'LINES'); c.text(WIN, 3, 124, 46, '012'); c.text(WIN, 3, 124, 64, 'LEVEL'); c.text(WIN, 3, 124, 72, '03'); c.text(WIN, 3, 124, 90, 'NEXT');
    c.rect(WIN, 3, 126, 100, 24, 20); c.rect(WIN, 0, 128, 102, 20, 16);
    [[0, 0], [1, 0], [2, 0], [1, 1]].forEach(([i, j]) => { c.rect(O1, 3, 130 + i * 6, 106 + j * 6, 6, 6); c.rect(O1, 1, 131 + i * 6, 107 + j * 6, 4, 4); });
    return c.px;
  },
};

export const SCENE_LIST: { id: SceneId; label: string }[] = [
  { id: 'action', label: 'Action' },
  { id: 'adventure', label: 'Adventure' },
  { id: 'puzzle', label: 'Puzzle' },
];

const sceneCache: Partial<Record<SceneId, Uint8Array>> = {};
const scenePixels = (id: SceneId): Uint8Array => (sceneCache[id] ??= SCENES[id]());

export function drawScene(canvas: HTMLCanvasElement, sceneId: SceneId, pal: ParsedPal): void {
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const img = ctx.createImageData(W, H);
  const px = scenePixels(sceneId);
  for (let i = 0; i < px.length; i++) {
    const col = pal.layers[px[i] >> 2][px[i] & 3];
    const o = i * 4;
    img.data[o] = col[0];
    img.data[o + 1] = col[1];
    img.data[o + 2] = col[2];
    img.data[o + 3] = 255;
  }
  ctx.putImageData(img, 0, 0);
}
