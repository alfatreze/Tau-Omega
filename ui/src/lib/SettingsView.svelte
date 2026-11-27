<script lang="ts">
  import OmegaUpdate from './OmegaUpdate.svelte';
  import type { CheckSummary, LayoutView, ScreenshotEntry, Setting, TaudReport } from './types';

  type SettingsTab = 'general' | 'appearance' | 'sync' | 'artwork' | 'diagnostics';
  export let tab: SettingsTab = 'general';

  export let settingsPath = '';
  export let settings: Setting[] = [];
  export let layoutView: LayoutView | null = null;
  export let checkSummary: CheckSummary | null = null;
  const groups = (v: LayoutView) => [...new Set(v.values.map((x) => x.group))];
  export let notice = '';
  export let choose: () => void;
  export let read: () => void;
  export let done: () => void = () => {};
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

  const TABS: [SettingsTab, string][] = [
    ['general', 'General'],
    ['appearance', 'Appearance'],
    ['sync', 'Music & Sync'],
    ['artwork', 'Artwork'],
    ['diagnostics', 'Diagnostics'],
  ];

  function tabKey(e: KeyboardEvent) {
    const i = TABS.findIndex(([t]) => t === tab);
    const next =
      e.key === 'ArrowRight'
        ? (i + 1) % TABS.length
        : e.key === 'ArrowLeft'
          ? (i + TABS.length - 1) % TABS.length
          : e.key === 'Home'
            ? 0
            : e.key === 'End'
              ? TABS.length - 1
              : -1;
    if (next < 0) return;
    e.preventDefault();
    tab = TABS[next][0];
    requestAnimationFrame(() => document.getElementById(`settings-tab-${tab}`)?.focus());
  }

  let showManualPath = false;
  const size = (bytes: number) =>
    bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(0)} KB` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  $: void done;
</script>

<section class="settings-view page" aria-labelledby="settings-title">
  <header>
    <div>
      <p class="eyebrow">PREFERENCES</p>
      <h1 id="settings-title">Settings</h1>
      <p class="lede">Tau Omega keeps your library work local and reviewable.</p>
    </div>
  </header>

  <div class="settings-tabs" role="tablist" aria-label="Settings categories" tabindex="-1" on:keydown={tabKey}>
    {#each TABS as [id, text] (id)}
      <button
        id={`settings-tab-${id}`}
        type="button"
        role="tab"
        class:current={tab === id}
        aria-selected={tab === id}
        aria-controls={`settings-panel-${id}`}
        tabindex={tab === id ? 0 : -1}
        on:click={() => (tab = id)}
      >
        {text}
      </button>
    {/each}
  </div>

  <div id={`settings-panel-${tab}`} role="tabpanel" aria-labelledby={`settings-tab-${tab}`} class="settings-panel" tabindex="-1">
    {#if tab === 'general'}
      <slot name="general" />
      <OmegaUpdate manual />
      <section class="settings-card safety-card" aria-labelledby="safety-privacy-title">
        <div class="safety-head">
          <h2 id="safety-privacy-title">Safety &amp; privacy</h2>
        </div>
        <div class="safety-cols">
          <div>
            <div class="safety-row">
              <strong>Card write protection</strong>
              <span class="chip capable">Enabled</span>
            </div>
            <p>Card writes stay off until you review and confirm an explicit plan. Moves require a verified external backup and a second confirmation.</p>
          </div>
          <div>
            <div class="safety-row">
              <strong>Local-only storage</strong>
              <span class="chip capable">Local only</span>
            </div>
            <p>Paths, reports, and job journals stay on this computer. Tau Omega never uploads your media or metadata.</p>
          </div>
        </div>
      </section>
    {:else if tab === 'appearance'}
      <slot name="appearance" />
    {:else if tab === 'sync'}
      <slot name="sync" />
    {:else if tab === 'artwork'}
      <slot name="artwork" />
    {:else if tab === 'diagnostics'}
      <section class="settings-card">
        <div style="width:100%">
          <h2>Inspect persisted settings</h2>
          <p>Choose a core’s <code>interact_persist.json</code> to read its saved values. This never writes to the card.</p>
          <div class="picker-row">
            <input bind:value={settingsPath} placeholder="/Volumes/Pocket/Settings/.../interact_persist.json" />
            <button class="picker" on:click={choose}>Choose</button>
            <button class="primary" on:click={read}>Read</button>
          </div>
          <p class="notice" role="status">{notice}</p>
          {#if layoutView}
            <div class="layout-view" data-testid="layout-view">
              <h3>{layoutView.layout === 'v2' ? 'Saved settings' : layoutView.layout === 'v1' ? 'Saved settings (older layout)' : layoutView.layout === 'legacy' ? 'Saved settings (legacy layout)' : 'Saved settings (newer than this Omega)'}</h3>
              {#if layoutView.layout === 'legacy'}
                <p class="notice">This file is in the legacy layout (v0.6.x). The player converts it by itself at its first start of v0.7.5 ({layoutView.migrated_words.length} words are written); Tau Omega never converts or copies it.</p>
              {:else if layoutView.layout === 'v1'}
                <p class="notice">This file is in the layout of Tau v0.7.0 and v0.7.1. The player converts it by itself at its first start of v0.7.5; Tau Omega never converts or copies it.</p>
              {:else if layoutView.layout === 'newer'}
                <p class="notice">This file was written by a newer Tau than this Omega understands, so its values are not shown. Update Tau Omega to read it. Nothing was changed.</p>
              {/if}
              {#each groups(layoutView) as g}
                <div class="settings-values">
                  <h4>{g}</h4>
                  {#each layoutView.values.filter((x) => x.group === g) as v}
                    <div><span>{v.label}</span><strong>{v.text}</strong></div>
                  {/each}
                </div>
              {/each}
            </div>
          {/if}
          {#if settings.length}
            <div class="settings-values">
              {#each settings as setting}
                <div><span>{label(setting.id)}</span><code>{setting.kind}</code><strong>{value(setting)}</strong></div>
              {/each}
            </div>
          {/if}
        </div>
      </section>

      {#if checkSummary}
        <section class="settings-card">
          <div style="width:100%">
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
          </div>
        </section>
      {/if}

      <section class="settings-card">
        <div style="width:100%">
          <h2>Screenshots &amp; Check reports</h2>
          <p>
            The Pocket saves every screenshot to <code>Memories/Screenshots/</code> on the card itself. Choose the card (not a single file) to browse them, newest first — pick one to see the actual image and, if it's a Check QR page, its fully decoded report: every test's value, build info, and memory-timing histograms. This never writes to the card.
          </p>
          <div class="picker-row">
            <input bind:value={screenshotCardPath} placeholder="/Volumes/Pocket" />
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
                    <p class="safety">No QR code in this image — it's a plain screenshot, not a Check QR page.</p>
                  {/if}
                  {#if qrReport}
                    <div class="settings-values">
                      <div><span>Profile</span><strong>{qrReport.profile} · <em>{qrReport.verdict}</em></strong></div>
                      {#if qrReport.entries.build}
                        <div><span>Build</span><strong>firmware {qrReport.entries.build.firmware} · bitstream {qrReport.entries.build.bitstream}</strong></div>
                      {/if}
                      {#if qrReport.entries.info_export}
                        {@const i = qrReport.entries.info_export}
                        <div><span>Info page</span><strong>firmware {i.firmware} · bitstream {i.fpga_rev} · free RAM {i.free_ram} B · {i.underruns} underruns · load {i.load_ms} ms · CPU {i.cpu_pct}%</strong></div>
                      {/if}
                      {#if qrReport.entries.stack}
                        <div><span>Stack</span><strong>peak {qrReport.entries.stack.peak_bytes} of {qrReport.entries.stack.stack_size} B ({qrReport.entries.stack.free_bytes} B free)</strong></div>
                      {/if}
                      {#if qrReport.entries.heap}
                        <div><span>Heap</span><strong>peak {qrReport.entries.heap.peak_bytes} of {qrReport.entries.heap.heap_size} B ({qrReport.entries.heap.free_bytes} B free)</strong></div>
                      {/if}
                      {#if qrReport.entries.load}
                        {@const l = qrReport.entries.load}
                        <div><span>CPU load</span><strong>{l.busy_pct}% busy (worst second {l.busy_worst_pct}%) · file waits {l.io_pct}% · {l.secs} s measured</strong></div>
                      {/if}
                      {#if qrReport.entries.load2}
                        {@const l = qrReport.entries.load2}
                        <div><span>MP3 loop</span><strong>decode {l.dec_pct}% · meters {l.feed_pct}% · sample push {l.push_pct}% · screen {l.ui_pct}% · waiting {l.wait_pct}%</strong></div>
                      {/if}
                      {#if qrReport.entries.now_playing}
                        {@const n = qrReport.entries.now_playing}
                        <div><span>Now playing</span><strong>{n.state}{n.title ? ` · ${n.title} · ${n.artist} · ${n.album}` : ''}{n.queue_len ? ` · track ${n.queue_pos} of ${n.queue_len}` : ''}</strong></div>
                      {/if}
                      {#if qrReport.entries.info_rows.length}
                        <div><span>Info page</span><strong>{qrReport.entries.info_rows.length} rows</strong></div>
                        {#each qrReport.entries.info_rows as r}
                          <div><span>{r.label}</span><strong>{r.value}</strong></div>
                        {/each}
                      {/if}
                      {#if qrReport.entries.decode_profile2}
                        {@const d = qrReport.entries.decode_profile2}
                        <div><span>Decode cost</span><strong>FLAC {d.t_pct}% of real time (channel 1 {d.c1_pct}%){d.lpc_max_ms === null ? '' : ` · worst LPC call ${d.lpc_max_ms >= 65535 ? '65 s or more' : `${d.lpc_max_ms} ms`}`} · MP3 dequantize {d.d_pct}%, anti-alias {d.a_pct}%, transform {d.x_pct}%</strong></div>
                      {/if}
                      {#if qrReport.entries.meter_config}
                        {@const m = qrReport.entries.meter_config}
                        <div><span>Meter settings</span><strong>meter {m.meter_id} · {m.preset === null ? 'no preset' : `preset ${m.preset}`} · {m.param_count} values (raw {m.raw_hex})</strong></div>
                      {/if}
                      {#if qrReport.entries.meter_trace.length}
                        <div><span>Meter trace</span><strong>{qrReport.entries.meter_trace.length} frames recorded</strong></div>
                      {/if}
                      {#if qrReport.unknown.length}
                        <div><span>Not decoded</span><strong>{qrReport.unknown.length} entr{qrReport.unknown.length === 1 ? 'y' : 'ies'} of a kind this version doesn't read yet (tag {[...new Set(qrReport.unknown.map((u) => u.tag))].join(', ')})</strong></div>
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

          <p class="manual-toggle">
            <button class="quiet" on:click={() => (showManualPath = !showManualPath)}>
              {showManualPath ? 'Hide' : 'Or decode a screenshot from elsewhere…'}
            </button>
          </p>
          {#if showManualPath}
            <div class="picker-row">
              <input bind:value={qrPath} placeholder="/path/to/screenshot.png" />
              <button class="picker" on:click={chooseQr}>Choose</button>
              <button class="primary" on:click={decodeQr}>Decode</button>
            </div>
          {/if}
        </div>
      </section>
    {/if}
  </div>
</section>

<style>
  .settings-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    border-bottom: 1px solid var(--border);
    margin: 0 0 18px;
  }
  .settings-tabs [role='tab'] {
    padding: 10px 14px;
    font-size: 13.5px;
    color: var(--text-secondary);
    background: transparent;
    border-radius: 8px 8px 0 0;
  }
  .settings-tabs [role='tab']:hover {
    color: var(--text);
    background: var(--bg-card);
  }
  .settings-tabs [role='tab'].current {
    color: var(--accent);
    box-shadow: inset 0 -2px 0 var(--accent-solid);
  }
  .settings-tabs [role='tab']:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .settings-panel {
    outline: none;
  }

  .safety-card {
    display: grid;
    gap: 14px;
  }
  .safety-head h2 {
    margin: 0;
  }
  .safety-cols {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
  }
  @media (max-width: 680px) {
    .safety-cols {
      grid-template-columns: 1fr;
    }
  }
  .safety-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 6px;
  }
  .safety-cols strong {
    font-size: 13.5px;
    color: var(--text-strong);
  }

  .qr-test-list { list-style: none; padding: 0; margin: 12px 0 0; border: 1px solid var(--border-strong); border-radius: 9px; overflow: auto; max-height: 320px; }
  .qr-test-list li { display: grid; grid-template-columns: 70px 1fr 70px; gap: 12px; align-items: center; padding: 8px 12px; border-bottom: 1px solid var(--border); font-size: 12px; color: var(--text-secondary); }
  .qr-test-list li:last-child { border-bottom: 0; }
  .qr-test-list li > span:last-child { color: var(--text-muted); text-align: right; }
  .qr-result { font-size: 11px; font-weight: 700; }
  .qr-result.pass { color: var(--accent); }
  .qr-result.fail { color: var(--danger-text); }
  .qr-result.skipped, .qr-result.na { color: var(--text-muted); }

  .screenshot-browser { display: grid; grid-template-columns: minmax(240px, 320px) 1fr; gap: 16px; margin-top: 14px; align-items: start; }
  .screenshot-rows { list-style: none; margin: 0; padding: 0; border: 1px solid var(--border-strong); border-radius: 9px; overflow: auto; max-height: 460px; }
  .screenshot-row { display: flex; flex-direction: column; gap: 2px; width: 100%; text-align: left; background: transparent; color: var(--text-secondary); padding: 9px 12px; border: 0; border-bottom: 1px solid var(--border); border-radius: 0; cursor: pointer; }
  .screenshot-rows li:last-child .screenshot-row { border-bottom: 0; }
  .screenshot-row:hover { background: var(--bg-card); }
  .screenshot-row.selected { background: var(--accent-soft-bg); }
  .shot-time { font-size: 12px; font-weight: 650; color: var(--text); }
  .shot-name { font-size: 11px; color: var(--text-muted); overflow-wrap: anywhere; }
  .shot-size { font-size: 11px; color: var(--text-faint); }
  .screenshot-detail { border: 1px solid var(--border-strong); border-radius: 9px; padding: 16px; min-height: 220px; min-width: 0; }
  .screenshot-image { display: block; max-width: 100%; height: auto; border-radius: 8px; border: 1px solid var(--border); margin-bottom: 12px; }
  .screenshot-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; height: 220px; color: var(--text-muted); font-size: 13px; text-align: center; }
  .spinner { width: 20px; height: 20px; border-radius: 50%; border: 2px solid var(--border-strong); border-top-color: var(--accent); animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .manual-toggle { margin: 14px 0 0; }
  .manual-toggle button { padding: 0; font-size: 12px; color: var(--text-muted); text-decoration: underline; }
</style>
