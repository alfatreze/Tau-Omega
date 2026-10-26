<script lang="ts">
  // Tau Alpha's own meter lab (a pinned vendor drop, interop D-007): ONE self-contained page with the real meter modules, themes,
  // palette and registry, including the SOUND MAP WebAssembly twin that runs the pack's own C code. It is shown as it is, in a
  // sandboxed frame; Tau Omega never edits it (to change it, ask Tau Alpha for a new drop: docs/interop/).
  import { onMount } from 'svelte';
  const LAB = '/tau-lab/meter_lab.html';
  let state: 'checking' | 'ready' | 'missing' = 'checking';
  onMount(async () => {
    try {
      const r = await fetch(LAB);
      // A dev server answers an unknown path with the app's own page, so look for the lab itself.
      state = r.ok && (await r.text()).includes('Tau meter lab') ? 'ready' : 'missing';
    } catch { state = 'missing'; }
  });
</script>
<section class="rml" aria-labelledby="rml-title">
  <h2 id="rml-title">Tau Alpha's meter lab</h2>
  {#if state === 'checking'}
    <p class="rml-note" role="status">Looking for the pinned lab…</p>
  {:else if state === 'ready'}
    <p class="rml-note">The real meters, themes and registry from Tau Alpha's own preview stack (the same drawing code as the Pocket; estimates, not measured). Pinned and never edited here.</p>
    <iframe title="Tau Alpha meter lab" src={LAB} sandbox="allow-scripts" loading="lazy"></iframe>
  {:else}
    <p class="rml-note" data-testid="rml-missing">The pinned lab page is not installed in this build yet. It comes from a Tau Alpha lab drop that includes the built <code>meter_lab.html</code> (see <code>vendor/tau-lab/README.md</code>); until then the editor below uses placeholder meters.</p>
  {/if}
</section>
<style>
  .rml{margin:0 0 22px}.rml h2{font-size:16px;margin:0 0 8px}
  .rml-note{font-size:12.5px;color:#a6b3b2;line-height:1.55;margin:0 0 10px;max-width:760px}
  iframe{display:block;width:100%;height:min(900px,82vh);border:1px solid #2c393a;border-radius:10px;background:#14161a}
  code{font-size:11.5px;background:#101617;border-radius:5px;padding:1px 5px}
</style>
