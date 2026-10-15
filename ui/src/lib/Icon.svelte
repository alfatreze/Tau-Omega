<script lang="ts">
  // One icon from the Phosphor Duotone set (see icons/phosphor.ts). Decorative by default; pass `label` when the icon alone carries meaning.
  import { ICONS } from './icons/phosphor';
  export let name = 'package'; export let size = 18; export let label = '';
  $: body = ICONS[name] ?? ICONS.package;
</script>
<svg class="ico" viewBox="0 0 256 256" width={size} height={size} fill="currentColor" role={label ? 'img' : undefined} aria-label={label || undefined} aria-hidden={label ? undefined : 'true'}>{@html body}</svg>
<style>.ico{flex:none;display:inline-block;vertical-align:middle}</style>
