<script lang="ts">
  // DESIGN PREVIEW of a coming feature: choosing which meters are on the Pocket. Nothing here reads the card or writes to it.
  // The Pocket loads its extra meters from one file in the core's own folder (`tau-packs.bin`, a bundle of meter packs, one per slot, at most
  // eight). Tau Alpha's rule today is that Omega reads that file and never changes it (its contract for a user-chosen meter file is not
  // published yet: interop Q-018), so every control below is switched off and the rows are an example, not your card.
  // When the contract exists the same layout becomes real: a tick per meter, a slot counter, a reviewed plan and a backup before anything is written.
  import type { MeterDef } from './meters/registry';

  export let meters: MeterDef[] = [];

  const BUILT_IN = 12; // Winamp Bars is built into the core: always on the Pocket, and it is slot 0 of the 8.
  const SLOTS = 8;
  // An example state only, to show what the page will look like: the last two meters are drawn as "not on the Pocket".
  $: example = meters.map((m, i) => ({ m, builtIn: m.id === BUILT_IN, on: m.id === BUILT_IN || i < meters.length - 2 }));
  $: used = example.filter((e) => e.on).length;
</script>

<section class="ml" aria-labelledby="ml-title" data-testid="meter-list-preview">
  <div class="ml-banner" role="note">
    <b>Preview of a coming feature.</b> Nothing on this page reads your card or changes it. Choosing which meters are on the Pocket needs a release from Tau Alpha that allows it; until then Omega leaves the Pocket's meters exactly as they are. The rows below are an example of how it will look.
  </div>

  <header class="ml-head">
    <div>
      <h2 id="ml-title">Meters on the Pocket</h2>
      <p class="ml-sub">Tick the meters you want in the Pocket's meter list. The Pocket has {SLOTS} meter slots in all, and the built-in one uses the first, so seven are left for others.</p>
    </div>
    <div class="ml-slots" aria-label="Meter slots used (example)">
      <span><b>{used}</b> of {SLOTS} slots used</span>
      <span class="ml-bar" aria-hidden="true"><i style="width:{(used / SLOTS) * 100}%"></i></span>
    </div>
  </header>

  <ul class="ml-list" aria-label="Meters (example)">
    {#each example as e (e.m.id)}
      <li class:off={!e.on}>
        <input type="checkbox" checked={e.on} disabled aria-label="{e.m.name} on the Pocket (example, not available yet)" />
        <span class="ml-thumb" aria-hidden="true"></span>
        <span class="ml-name"><b>{e.m.name}</b><small>{e.builtIn ? 'Built into the Pocket: always on, uses the first slot' : 'Meter pack · takes one slot'}</small></span>
        <span class="ml-state">{e.on ? 'On the Pocket' : 'Not on the Pocket'}</span>
      </li>
    {/each}
  </ul>

  <div class="ml-actions">
    <button type="button" class="primary" disabled>Save meters to the Pocket…</button>
    <button type="button" disabled>Add meters from Tau Alpha's library…</button>
    <span class="ml-soon">Not available yet</span>
  </div>
</section>

<style>
  .ml{display:grid;gap:14px;max-width:760px}
  .ml-banner{padding:11px 14px;border:1px dashed #6f6a45;background:#211f14;color:#e5d9a3;border-radius:10px;font-size:13px;line-height:1.55}.ml-banner b{color:#f0e6b8}
  .ml-head{display:flex;gap:16px;justify-content:space-between;align-items:flex-end;flex-wrap:wrap;margin:0}
  .ml h2{margin:0 0 4px;font-size:18px}.ml-sub{margin:0;color:#aab8b7;font-size:13px;max-width:52ch;line-height:1.5}
  .ml-slots{display:grid;gap:6px;font-size:12.5px;color:#aab8b7;min-width:170px}.ml-slots b{color:#e8ecec}
  .ml-bar{display:block;height:6px;border-radius:99px;background:#26302f;overflow:hidden}.ml-bar i{display:block;height:100%;background:#5f7a62}
  .ml-list{list-style:none;margin:0;padding:0;display:grid;gap:6px;opacity:.78}
  .ml-list li{display:flex;align-items:center;gap:12px;padding:9px 12px;border:1px solid #2c393a;border-radius:10px;background:#1a2325}
  .ml-list li.off{border-style:dashed;background:#161c1d}.ml-list li.off .ml-name b{color:#98a5a4}
  .ml-list input{width:16px;height:16px;margin:0;accent-color:#8fa88c}
  .ml-thumb{flex:none;width:44px;height:36px;border-radius:6px;background:linear-gradient(135deg,#243033,#1b2527);border:1px solid #2c393a}
  .ml-name{display:grid;gap:2px;min-width:0;flex:1}.ml-name b{font-size:13px}.ml-name small{color:#8f9e9d;font-size:11.5px}
  .ml-state{font-size:12px;color:#8f9e9d;white-space:nowrap}
  .ml-actions{display:flex;gap:8px;flex-wrap:wrap;align-items:center}
  .ml-actions button{padding:8px 12px;font-size:13px;background:#202b2d;color:#e8ecec;border:1px solid #2c393a;border-radius:8px}
  .ml-actions button.primary{background:#c1f0ad;color:#142015;font-weight:700;border-color:#c1f0ad}
  .ml-actions button:disabled{opacity:.45;cursor:not-allowed}
  .ml-soon{font-size:12px;color:#d9c47e;margin-left:4px}
</style>
