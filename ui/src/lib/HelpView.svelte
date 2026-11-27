<script lang="ts">
  // Dedicated "Using Tau Omega" help guide (moved out of the floating ? modal into the Help section).
  export let feedbackOn = false;
  export let openFeedback: () => void = () => {};
</script>

<section class="page help-page" aria-labelledby="help-page-title">
  <header class="help-head">
    <div>
      <p class="eyebrow">HELP</p>
      <h1 id="help-page-title">Using Tau Omega</h1>
      <p class="lede">How Tau Omega reads your cards safely, manages cores and games, and syncs music.</p>
    </div>
    {#if feedbackOn}
      <button class="quiet" type="button" on:click={openFeedback}>Send feedback…</button>
    {/if}
  </header>

  <div class="help-cards">
    <article class="help-card">
      <h2>Opening cards</h2>
      <p><strong>Open folder</strong> browses to an Analogue Pocket or Game Bub SD card, or any plain staging folder. A mounted card and any card you have opened before appear under <strong>Known cards</strong> for one-click reopening.</p>
    </article>

    <article class="help-card">
      <h2>Cores, games &amp; artwork</h2>
      <p>Open any core under <strong>Cores</strong> to browse its ROMs in List or Grid view, filter by genre category, and click a game to see its cover, title screen, screenshot, developer, publisher, and release year.</p>
      <p>Clicking <strong>Art</strong> on any game or core lets you pick artwork from Libretro or your computer. Game covers and desktop artwork are stored locally inside Tau Omega and never modify your SD card unless you review a Pocket banner plan.</p>
      <p>On Pocket cores, <strong>Visible on the Pocket</strong> and BIOS/configuration files live under the core’s <strong>Settings</strong> tab. Core details lets you update, downgrade, or remove a core (with a prompt to remove shared ROMs when deleting the last core for a platform).</p>
    </article>

    <article class="help-card">
      <h2>Music &amp; playlists</h2>
      <p>Cores that look like media players (Tau, or anything that can play your music files) get a large card on the Card screen. Anything else can be promoted with <strong>Set as player</strong>.</p>
      <p><strong>Music</strong> shows the selected card’s music on the right and a folder on this computer on the left. Tick albums (or drag them across) to add them, mark albums on the Pocket to remove, rename or change their cover, then press <strong>Start sync</strong>. Everything you stage sits in <strong>Pending changes</strong> until you confirm.</p>
    </article>

    <article class="help-card">
      <h2>Safety rules &amp; shortcuts</h2>
      <p>Every write anywhere on a card goes through <strong>plan → review → confirm</strong> — nothing is copied, moved, or deleted until you have seen what will change and confirmed it. Replaced or removed files are backed up outside the card first.</p>
      <ul>
        <li><code>I</code> toggles the right-hand details sidebar when viewing a core or game.</li>
        <li><code>Escape</code> closes sheets, pickers, and lightboxes.</li>
      </ul>
    </article>
  </div>
</section>

<style>
  .help-page { display: grid; gap: 18px; align-content: start; }
  .help-head { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; flex-wrap:wrap; }
  .help-cards { display: grid; gap: 14px; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); }
  .help-card { background: var(--bg-surface); border: 1px solid var(--border); border-radius: 12px; padding: 16px 18px; display: grid; gap: 8px; align-content: start; }
  .help-card h2 { margin: 0; font-size: 16px; color: var(--text); }
  .help-card p, .help-card li { margin: 0; font-size: 13px; color: var(--text-secondary); line-height: 1.55; }
  .help-card ul { margin: 4px 0 0; padding-left: 18px; display: grid; gap: 4px; }
  code { font-size: 11.5px; background: var(--bg-input); border-radius: 5px; padding: 1px 5px; color: var(--accent); }
</style>
