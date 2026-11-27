<script lang="ts">
  // Settings -> Appearance: lets users pick System, Light, or Dark theme and their primary accent color.
  import {
    ACCENT_PRESETS,
    accentPreference,
    setAccentPreset,
    setCustomAccent,
    setThemePreference,
    systemTheme,
    themePreference,
    type ThemePreference,
  } from './themeStore';

  const options: { value: ThemePreference; title: string; desc: string }[] = [
    {
      value: 'system',
      title: 'System',
      desc: 'Match your operating system setting automatically.',
    },
    {
      value: 'light',
      title: 'Light',
      desc: 'Bright neutral surfaces with high-contrast text.',
    },
    {
      value: 'dark',
      title: 'Dark',
      desc: 'Deep charcoal surfaces with calm contrast.',
    },
  ];
</script>

<section class="theme-settings" aria-labelledby="app-appearance-title">
  <h2 id="app-appearance-title">Appearance</h2>
  <p class="ts-lede">Choose how Tau Omega looks. System follows your OS appearance automatically.</p>

  <fieldset>
    <legend>Theme</legend>
    <div class="ts-grid">
      {#each options as opt (opt.value)}
        {@const active = $themePreference === opt.value}
        <label class="ts-opt" class:active>
          <input
            type="radio"
            name="app-theme"
            value={opt.value}
            checked={active}
            on:change={() => setThemePreference(opt.value)}
          />
          <span class="ts-swatch ts-swatch-{opt.value}" aria-hidden="true">
            <span class="ts-swatch-side"></span>
            <span class="ts-swatch-main"><i></i><u></u></span>
          </span>
          <span class="ts-copy">
            <strong>
              {opt.title}
              {#if opt.value === 'system'}
                <span class="ts-cur">(currently {$systemTheme === 'dark' ? 'dark' : 'light'})</span>
              {/if}
            </strong>
            <small>{opt.desc}</small>
          </span>
          <span class="ts-check" aria-hidden="true">{active ? '✓' : ''}</span>
        </label>
      {/each}
    </div>
  </fieldset>

  <fieldset class="ts-accent-fs">
    <div class="ts-accent-head">
      <legend>Primary color</legend>
      {#if $accentPreference.id !== 'sage'}
        <button type="button" class="quiet" on:click={() => setAccentPreset('sage')}>
          Reset to Sage
        </button>
      {/if}
    </div>
    <p class="ts-accent-help">Adapts automatically for readable contrast in both Light and Dark modes.</p>
    <div class="ts-accents">
      {#each ACCENT_PRESETS as preset (preset.id)}
        {@const selected = $accentPreference.id === preset.id}
        <label
          class="ts-acc"
          class:active={selected}
          style="--sw-l:{preset.swatchLight};--sw-d:{preset.swatchDark}"
        >
          <input
            type="radio"
            name="app-accent"
            value={preset.id}
            checked={selected}
            on:change={() => setAccentPreset(preset.id)}
          />
          <span class="ts-dot" aria-hidden="true">{selected ? '✓' : ''}</span>
          <span class="ts-acc-name">{preset.label}</span>
        </label>
      {/each}
      <label class="ts-acc ts-acc-custom" class:active={$accentPreference.id === 'custom'}>
        <input
          type="radio"
          name="app-accent"
          value="custom"
          checked={$accentPreference.id === 'custom'}
          on:change={() => setCustomAccent($accentPreference.customHex)}
        />
        <span class="ts-dot ts-dot-custom" style="background:{$accentPreference.customHex}" aria-hidden="true">
          {#if $accentPreference.id === 'custom'}✓{/if}
        </span>
        <span class="ts-acc-name">
          Custom
          <small>{$accentPreference.customHex.toUpperCase()}</small>
        </span>
        <input
          type="color"
          class="ts-color-picker"
          aria-label="Custom primary color"
          value={$accentPreference.customHex}
          on:input={(e) => setCustomAccent(e.currentTarget.value)}
          on:change={(e) => setCustomAccent(e.currentTarget.value)}
        />
      </label>
    </div>
  </fieldset>
</section>

<style>
  .theme-settings {
    max-width: 820px;
    margin-bottom: 14px;
    padding: 22px 24px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg-card);
  }
  h2 {
    margin: 0 0 6px;
  }
  .ts-lede {
    margin: 0 0 16px;
    font-size: 13px;
    color: var(--text-muted);
    line-height: 1.5;
  }
  fieldset {
    border: 0;
    padding: 0;
    margin: 0;
  }
  legend {
    font-weight: 700;
    font-size: 13px;
    margin-bottom: 10px;
    color: var(--text-strong);
  }
  .ts-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }
  @media (max-width: 680px) {
    .ts-grid {
      grid-template-columns: 1fr;
    }
  }
  .ts-opt {
    position: relative;
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-rows: auto auto;
    gap: 10px 8px;
    padding: 12px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    cursor: pointer;
    transition: border-color 0.14s ease, background 0.14s ease;
  }
  .ts-opt:hover {
    border-color: var(--border-strong);
    background: var(--bg-hover);
  }
  .ts-opt.active {
    border-color: var(--accent);
    background: var(--accent-soft-bg);
  }
  .ts-opt:has(input:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .ts-opt input {
    position: absolute;
    opacity: 0;
    width: 1px;
    height: 1px;
    margin: 0;
  }
  .ts-swatch {
    grid-column: 1 / -1;
    height: 44px;
    border-radius: 7px;
    border: 1px solid var(--border-strong);
    overflow: hidden;
    display: grid;
    grid-template-columns: 28% 1fr;
  }
  .ts-swatch-side {
    border-right: 1px solid rgba(128, 140, 138, 0.25);
  }
  .ts-swatch-main {
    padding: 8px;
    display: grid;
    gap: 5px;
    align-content: center;
  }
  .ts-swatch-main i,
  .ts-swatch-main u {
    display: block;
    height: 5px;
    border-radius: 99px;
    text-decoration: none;
  }
  .ts-swatch-main u {
    width: 64%;
  }
  .ts-swatch-light .ts-swatch-side {
    background: #f0f0f2;
  }
  .ts-swatch-light .ts-swatch-main {
    background: #f7f7f8;
  }
  .ts-swatch-light .ts-swatch-main i {
    background: var(--accent-solid-light);
  }
  .ts-swatch-light .ts-swatch-main u {
    background: #a6a6b0;
  }
  .ts-swatch-dark .ts-swatch-side {
    background: #0e0e10;
  }
  .ts-swatch-dark .ts-swatch-main {
    background: #131316;
  }
  .ts-swatch-dark .ts-swatch-main i {
    background: var(--accent-solid-dark);
  }
  .ts-swatch-dark .ts-swatch-main u {
    background: #454552;
  }
  .ts-swatch-system {
    background: linear-gradient(135deg, #f7f7f8 0 50%, #131316 50% 100%);
  }
  .ts-swatch-system .ts-swatch-side {
    background: linear-gradient(180deg, #f0f0f2 0 50%, #0e0e10 50% 100%);
  }
  .ts-swatch-system .ts-swatch-main {
    background: transparent;
  }
  .ts-swatch-system .ts-swatch-main i {
    background: linear-gradient(90deg, var(--accent-solid-light) 0 50%, var(--accent-solid-dark) 50% 100%);
  }
  .ts-swatch-system .ts-swatch-main u {
    background: linear-gradient(90deg, #a6a6b0 0 50%, #454552 50% 100%);
  }
  .ts-copy {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .ts-copy strong {
    font-size: 13px;
    color: var(--text-strong);
  }
  .ts-cur {
    font-weight: 500;
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .ts-copy small {
    font-size: 11.5px;
    line-height: 1.4;
    color: var(--text-muted);
  }
  .ts-check {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 1px solid var(--border-strong);
    display: grid;
    place-items: center;
    font-size: 11px;
    font-weight: 700;
    color: var(--accent-on-solid);
    align-self: start;
  }
  .ts-opt.active .ts-check {
    background: var(--accent-solid);
    border-color: var(--accent-solid);
  }

  .ts-accent-fs {
    margin-top: 20px;
    padding-top: 18px;
    border-top: 1px solid var(--border-subtle);
  }
  .ts-accent-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 4px;
  }
  .ts-accent-head legend {
    margin-bottom: 0;
  }
  .ts-accent-head .quiet {
    padding: 4px 10px;
    font-size: 12px;
    border: 1px solid var(--border);
    border-radius: 7px;
  }
  .ts-accent-help {
    margin: 0 0 12px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .ts-accents {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .ts-acc {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px 7px 8px;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--bg-surface);
    color: var(--text);
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
    transition: border-color 0.14s ease, background 0.14s ease;
  }
  .ts-acc:hover {
    border-color: var(--border-strong);
    background: var(--bg-hover);
  }
  .ts-acc.active {
    border-color: var(--accent);
    background: var(--accent-soft-bg);
    color: var(--accent-soft-text);
    font-weight: 650;
  }
  .ts-acc:has(input[type='radio']:focus-visible),
  .ts-acc:has(.ts-color-picker:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .ts-acc input[type='radio'] {
    position: absolute;
    opacity: 0;
    width: 1px;
    height: 1px;
    margin: 0;
  }
  .ts-dot {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    flex: none;
    display: grid;
    place-items: center;
    font-size: 11px;
    font-weight: 800;
    color: #ffffff;
    background: var(--sw-l);
    border: 1px solid rgba(128, 140, 138, 0.35);
  }
  @supports (color: light-dark(white, black)) {
    .ts-dot:not(.ts-dot-custom) {
      background: light-dark(var(--sw-l), var(--sw-d));
      color: light-dark(#ffffff, #111617);
    }
  }
  .ts-dot-custom {
    color: #ffffff;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.7);
  }
  .ts-acc-name {
    display: inline-flex;
    align-items: baseline;
    gap: 5px;
  }
  .ts-acc-name small {
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    color: var(--text-muted);
  }
  .ts-color-picker {
    width: 22px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
  }
  .ts-color-picker::-webkit-color-swatch-wrapper {
    padding: 1px;
  }
  .ts-color-picker::-webkit-color-swatch {
    border: 0;
    border-radius: 4px;
  }
</style>
