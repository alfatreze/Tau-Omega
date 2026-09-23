<script lang="ts">
  import type { CheckSummary, ScreenshotEntry, Setting, TaudReport } from './types';
  export let settingsPath = '';
  export let settings: Setting[] = [];
  export let checkSummary: CheckSummary | null = null;
  export let notice = '';
  export let choose: () => void;
  export let read: () => void;
  export let done: () => void;
  export let label: (id: number) => string;
  export let value: (setting: Setting) => string;

  export let qrPath = '';
  export let chooseQr: () => void;
  export let decodeQr: () => void;
  export let qrReport: TaudReport | null = null;
  export let qrNotFound = false;
  export let qrNotice = '';

  export let screenshotCardPath = '';
  export let chooseScreenshotCard: () => void;
  export let browseScreenshots: () => void;
  export let screenshots: ScreenshotEntry[] = [];
  export let screenshotsNotice = '';
  export let selectScreenshot: (path: string) => void;

  const size = (bytes: number) => bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(0)} KB` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
</script>

<section class="settings-view page" aria-labelledby="settings-title">
  <header><div><p class="eyebrow">PREFERENCES</p><h1 id="settings-title">Settings</h1><p class="lede">Tau Omega keeps your library work local and reviewable.</p></div></header>
  <section class="settings-card"><div><h2>Inspect persisted settings</h2><p>Choose a core’s <code>interact_persist.json</code> to read its saved values. This never writes to the card.</p><div class="picker-row"><input bind:value={settingsPath} placeholder="/Volumes/Pocket/Settings/.../interact_persist.json"/><button class="picker" on:click={choose}>Choose</button><button class="primary" on:click={read}>Read</button></div><p class="notice" role="status">{notice}</p>{#if settings.length}<div class="settings-values">{#each settings as setting}<div><span>{label(setting.id)}</span><code>{setting.kind}</code><strong>{value(setting)}</strong></div>{/each}</div>{/if}</div></section>
  {#if checkSummary}
    <section class="settings-card"><div style="width:100%">
      <h2>Diagnostic Check summary</h2>
      <p>{checkSummary.profile} · run {checkSummary.run} · <strong>{checkSummary.verdict}</strong></p>
      <div class="settings-values">
        {#if checkSummary.passed.length}<div><span>Passed</span><strong>{checkSummary.passed.join(', ')}</strong></div>{/if}
        {#if checkSummary.failed.length}<div><span>Failed</span><strong>{checkSummary.failed.join(', ')}</strong></div>{/if}
        <div><span>Access</span><strong>worst {checkSummary.worst_access_cycles} cycles · cold {checkSummary.cold_cycles_per_word} C/W</strong></div>
        <div><span>Playback</span><strong>{checkSummary.late_underruns} late underruns · {checkSummary.draw_stall_ms} ms draw stall</strong></div>
        <div><span>Library</span><strong>last load {checkSummary.last_load_s}s · error {checkSummary.library_error} · cold error {checkSummary.cold_error}</strong></div>
        <div><span>Firmware</span><strong>minor {checkSummary.firmware_minor}</strong></div>
      </div>
    </div></section>
  {/if}
  <section class="settings-card">
    <div style="width:100%">
      <h2>Browse screenshots on a card</h2>
      <p>The Pocket saves every screenshot to <code>Memories/Screenshots/</code> on the card itself.
        Choose the card (not a single file) to list them, newest first, then pick one below to
        decode instead of hunting for the file path yourself.</p>
      <div class="picker-row">
        <input bind:value={screenshotCardPath} placeholder="/Volumes/Pocket"/>
        <button class="picker" on:click={chooseScreenshotCard}>Choose</button>
        <button class="primary" on:click={browseScreenshots}>List screenshots</button>
      </div>
      <p class="notice" role="status">{screenshotsNotice}</p>
      {#if screenshots.length}
        <ul class="qr-test-list screenshot-list">
          {#each screenshots as shot}
            <li>
              <span>{shot.captured_at ?? shot.filename}</span>
              <span>{shot.filename}</span>
              <span>{size(shot.bytes)}</span>
              <button class="picker" on:click={() => selectScreenshot(shot.path)}>Decode</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </section>
  <section class="settings-card">
    <div style="width:100%">
      <h2>Full Check report (QR screenshot)</h2>
      <p>Choose a screenshot of the Pocket's Check QR page to decode the complete report — every
        test's value, build info, and memory-timing histograms — not just the tiny summary above.
        This never writes to the card; it only reads the image file.</p>
      <div class="picker-row">
        <input bind:value={qrPath} placeholder="/path/to/screenshot.png"/>
        <button class="picker" on:click={chooseQr}>Choose</button>
        <button class="primary" on:click={decodeQr}>Decode</button>
      </div>
      <p class="notice" role="status">{qrNotice}</p>
      {#if qrNotFound}
        <p class="safety">No QR code found in this image — a Check QR screenshot is a plain,
          unresized PNG showing the black-and-white QR square, not a result-page or now-playing
          screenshot.</p>
      {/if}
      {#if qrReport}
        <div class="settings-values">
          <div><span>Profile</span><strong>{qrReport.profile} · <em>{qrReport.verdict}</em></strong></div>
          {#if qrReport.entries.build}
            <div><span>Build</span><strong>firmware {qrReport.entries.build.firmware} · bitstream {qrReport.entries.build.bitstream}</strong></div>
          {/if}
        </div>
        <ul class="qr-test-list">
          {#each qrReport.tests as test}
            <li><span class={`qr-result ${test.result.toLowerCase().replace('/', '')}`}>{test.result}</span><span>{test.name}</span><span>{test.value}</span></li>
          {/each}
        </ul>
      {/if}
    </div>
  </section>
  <section class="settings-card"><div><h2>Safety defaults</h2><p>Card writes are disabled until you review and confirm an explicit plan. Move operations require a verified external backup and a second confirmation.</p></div><span class="chip capable">Enabled</span></section>
  <section class="settings-card"><div><h2>Data handling</h2><p>Paths, reports, and job journals stay on this computer. Tau Omega does not upload your media or metadata.</p></div><span class="chip capable">Local only</span></section>
  <button class="primary" on:click={done}>Done</button>
</section>
<style>
  .qr-test-list { list-style: none; padding: 0; margin: 12px 0 0; border: 1px solid #344244; border-radius: 9px; overflow: auto; max-height: 320px; }
  .qr-test-list li { display: grid; grid-template-columns: 70px 1fr 70px; gap: 12px; align-items: center; padding: 8px 12px; border-bottom: 1px solid #2c393a; font-size: 12px; color: #c7d1d0; }
  .qr-test-list li:last-child { border-bottom: 0; }
  .qr-test-list li > span:last-child { color: #8f9e9d; text-align: right; }
  .qr-result { font-size: 11px; font-weight: 700; }
  .qr-result.pass { color: #b9e9a5; }
  .qr-result.fail { color: #f29b83; }
  .qr-result.skipped, .qr-result.na { color: #8f9e9d; }
  .screenshot-list li { grid-template-columns: 150px 1fr 70px 70px; }
</style>
