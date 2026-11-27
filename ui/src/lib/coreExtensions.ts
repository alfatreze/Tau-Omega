// Registry of optional platform/core-specific feature tabs (e.g., Game Boy Palettes, future MiSTer/DSpico/Tau modules).
// Keeping them registered here avoids hardcoding per-core branches inside CoresView.svelte.
import GbPalettesTab from './GbPalettesTab.svelte';
import type { DeviceKind } from './types';

export type CoreExtensionContext = {
  device: DeviceKind;
  coreId: string;
  platformIds: string[];
};

export type CoreExtension = {
  id: string;
  label: string;
  match: (ctx: CoreExtensionContext) => boolean;
  component: typeof GbPalettesTab;
};

export const CORE_EXTENSIONS: CoreExtension[] = [
  {
    id: 'palettes',
    label: 'Palettes',
    match: (ctx) => ctx.device === 'analogue_pocket' && ctx.platformIds.includes('gb'),
    component: GbPalettesTab,
  },
];

export function extensionsForCore(ctx: CoreExtensionContext): CoreExtension[] {
  return CORE_EXTENSIONS.filter((ext) => ext.match(ctx));
}

export function extensionsForPlatform(device: DeviceKind, platformId: string): CoreExtension[] {
  return CORE_EXTENSIONS.filter((ext) =>
    ext.match({ device, coreId: '', platformIds: [platformId] }),
  );
}
