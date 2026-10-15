// Which icon stands for what: platform categories (the openFPGA Library's), and the kinds of file a core folder holds.
export const CATEGORY_ICON: Record<string, string> = {
  Console: 'game-controller', Handheld: 'device-mobile', Computer: 'desktop-tower', Arcade: 'joystick', 'Arcade Multi': 'joystick',
  'Media Players': 'music-notes', 'Music Players': 'music-notes', Media: 'music-notes', Other: 'package', Others: 'package', Tools: 'package', Utility: 'package', 'No platform': 'package',
};
export const categoryIcon = (c: string) => CATEGORY_ICON[c] ?? 'package';

/** File groups from the engine's file-purpose inference. */
export const GROUP_ICON: Record<string, string> = {
  'BIOS and system': 'cpu', Palettes: 'palette', Disks: 'floppy-disk', 'Memory and saves': 'memory', 'Game setup': 'sliders-horizontal', 'Game data': 'cube', 'Other files': 'package',
};
const EXT_ICON: Record<string, string> = { hdf: 'hard-drive', vhd: 'hard-drive', img: 'hard-drive', iso: 'disc', cue: 'disc', chd: 'disc', bin: '', tap: 'cassette-tape', cas: 'cassette-tape', tzx: 'cassette-tape', t64: 'cassette-tape', zip: 'file-zip', '7z': 'file-zip', png: 'image', sta: 'clock-counter-clockwise', sav: 'memory', json: 'sliders-horizontal', pal: 'palette' };
/** The icon of one file: its extension when that says more than its group (a hard-drive image, a tape), else its group, else by kind. */
export function fileIcon(name: string, group = '', kind = ''): string {
  const ext = name.includes('.') ? name.split('.').pop()!.toLowerCase() : '';
  if (EXT_ICON[ext]) return EXT_ICON[ext];
  if (group && GROUP_ICON[group]) return GROUP_ICON[group];
  if (kind === 'Save') return 'memory';
  if (kind === 'State') return 'clock-counter-clockwise';
  if (kind === 'Tape') return 'cassette-tape';
  if (kind === 'Disk') return 'floppy-disk';
  if (kind === 'Bios') return 'cpu';
  return 'cube';
}
