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
  export let selectedScreenshotPath = '';
  export let screenshotPreviewUrl: string | null = null;
  export let screenshotPreviewLoading = false;

  let showManualPath = false;
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
      <h2>Screenshots &amp; Check reports</h2>
      <p>The Pocket saves every screenshot to <code>Memories/Screenshots/</code> on the card itself.
        Choose the card (not a single file) to browse them, newest first — pick one to see the
        actual image and, if it's a Check QR page, its fully decoded report: every test's value,
        build info, and memory-timing histograms. This never writes to the card.</p>
      <div class="picker-row">
        <input bind:value={screenshotCardPath} placeholder="/Volumes/Pocket"/>
        <button class="picker" on:click={chooseScreenshotCard}>Choose</button>
        <button class="primary" on:click={browseScreenshots}>Browse</button>
      </div>
      <p class="notice" role="status">{screenshotsNotice}</p>

      {#if screenshots.length}
        <div class="screenshot-browser">
          <ul class="screenshot-rows">
            {#each screenshots as shot}
              <li>
                <button class="screenshot-row" class:selected={selectedScreenshotPath === shot.path} on:click={() => selectScreenshot(shot.path)}>
                  <span class="shot-time">{shot.captured_at ?? shot.filename}</span>
                  <span class="shot-name">{shot.filename}</span>
                  <span class="shot-size">{size(shot.bytes)}</span>
                </button>
              </li>
            {/each}
          </ul>
          <div class="screenshot-detail">
            {#if screenshotPreviewLoading}
              <div class="screenshot-empty"><span class="spinner" aria-hidden="true"></span><p>Loading…</p></div>
            {:else if screenshotPreviewUrl}
              <img class="screenshot-image" src={screenshotPreviewUrl} alt="Selected Pocket screenshot" />
              <p class="notice" role="status">{qrNotice}</p>
              {#if qrNotFound}
                <p class="safety">No QR code in this image — it's a plain screenshot, not a Check
                  QR page.</p>
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
            {:else}
              <div class="screenshot-empty"><p>Pick a screenshot on the left to view it here.</p></div>
            {/if}
          </div>
        </div>
      {/if}

      <p class="manual-toggle"><button class="quiet" on:click={() => showManualPath = !showManualPath}>{showManualPath ? 'Hide' : 'Or decode a screenshot from elsewhere…'}</button></p>
      {#if showManualPath}
        <div class="picker-row">
          <input bind:value={qrPath} placeholder="/path/to/screenshot.png"/>
          <button class="picker" on:click={chooseQr}>Choose</button>
          <button class="primary" on:click={decodeQr}>Decode</button>
        </div>
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

  .screenshot-browser { display: grid; grid-template-columns: minmax(240px, 320px) 1fr; gap: 16px; margin-top: 14px; align-items: start; }
  .screenshot-rows { list-style: none; margin: 0; padding: 0; border: 1px solid #344244; border-radius: 9px; overflow: auto; max-height: 460px; }
  .screenshot-row { display: flex; flex-direction: column; gap: 2px; width: 100%; text-align: left; background: transparent; color: #c7d1d0; padding: 9px 12px; border: 0; border-bottom: 1px solid #2c393a; border-radius: 0; cursor: pointer; }
  .screenshot-rows li:last-child .screenshot-row { border-bottom: 0; }
  .screenshot-row:hover { background: #1c2628; }
  .screenshot-row.selected { background: #223d2a; }
  .shot-time { font-size: 12px; font-weight: 650; color: #eff5f4; }
  .shot-name { font-size: 11px; color: #93a3a2; overflow-wrap: anywhere; }
  .shot-size { font-size: 11px; color: #6f8180; }
  .screenshot-detail { border: 1px solid #344244; border-radius: 9px; padding: 16px; min-height: 220px; min-width: 0; }
  .screenshot-image { display: block; max-width: 100%; height: auto; border-radius: 8px; border: 1px solid #2c393a; margin-bottom: 12px; }
  .screenshot-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; height: 220px; color: #8c9c9b; font-size: 13px; text-align: center; }
  .spinner { width: 20px; height: 20px; border-radius: 50%; border: 2px solid #344244; border-top-color: #c1f0ad; animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .manual-toggle { margin: 14px 0 0; }
  .manual-toggle button { padding: 0; font-size: 12px; color: #93a3a2; text-decoration: underline; }
</style>
