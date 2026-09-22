<script lang="ts">
  import type { Setting } from './types';
  export let settingsPath = '';
  export let settings: Setting[] = [];
  export let notice = '';
  export let choose: () => void;
  export let read: () => void;
  export let done: () => void;
  export let label: (id: number) => string;
  export let value: (setting: Setting) => string;
</script>

<section class="settings-view page" aria-labelledby="settings-title">
  <header><div><p class="eyebrow">PREFERENCES</p><h1 id="settings-title">Settings</h1><p class="lede">Tau Omega keeps your library work local and reviewable.</p></div></header>
  <section class="settings-card"><div><h2>Inspect persisted settings</h2><p>Choose a core’s <code>interact_persist.json</code> to read its saved values. This never writes to the card.</p><div class="picker-row"><input bind:value={settingsPath} placeholder="/Volumes/Pocket/Settings/.../interact_persist.json"/><button class="picker" on:click={choose}>Choose</button><button class="primary" on:click={read}>Read</button></div><p class="notice" role="status">{notice}</p>{#if settings.length}<div class="settings-values">{#each settings as setting}<div><span>{label(setting.id)}</span><code>{setting.kind}</code><strong>{value(setting)}</strong></div>{/each}</div>{/if}</div></section>
  <section class="settings-card"><div><h2>Safety defaults</h2><p>Card writes are disabled until you review and confirm an explicit plan. Move operations require a verified external backup and a second confirmation.</p></div><span class="chip capable">Enabled</span></section>
  <section class="settings-card"><div><h2>Data handling</h2><p>Paths, reports, and job journals stay on this computer. Tau Omega does not upload your media or metadata.</p></div><span class="chip capable">Local only</span></section>
  <button class="primary" on:click={done}>Done</button>
</section>
