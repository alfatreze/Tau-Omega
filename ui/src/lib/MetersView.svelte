<script lang="ts">
  // Meters: UI-structure scaffold only (owner decision, 2026-09-26). Every
  // meter, parameter and preset here is SYNTHETIC (see `meters/schema.ts`'s
  // own header) -- tau-alpha's real meter-module work is mid-M0, with no
  // tagged release yet containing a real `meters_schema.json`
  // (`docs/FIRMWARE_SYNC.md`'s "Watched interfaces"). This page proves out
  // the editor/preset/preview *structure* against a realistic placeholder
  // shape so swapping in the real schema later is a data change, not a
  // rewrite -- per `docs/METER_MODULE_SPEC.md` section 22's own description
  // of what Omega eventually does (meter list, generated editor, presets,
  // live preview, capture-from-device), built ahead of the data being real.
  import { SYNTHETIC_METERS, defaultValues, paramVisible, type MeterSchema } from './meters/schema';
  import MeterPreview from './meters/MeterPreview.svelte';

  let selected: MeterSchema = SYNTHETIC_METERS[0];
  // One independent value set per meter, so switching meters doesn't lose
  // in-progress edits on the others.
  let valuesByMeter: Record<string, Record<string, number | boolean>> = Object.fromEntries(
    SYNTHETIC_METERS.map((meter) => [meter.key, defaultValues(meter)])
  );
  let activePresetByMeter: Record<string, string> = Object.fromEntries(
    SYNTHETIC_METERS.map((meter) => [meter.key, meter.presets[0]?.name ?? 'CUSTOM'])
  );

  $: values = valuesByMeter[selected.key];
  $: activePreset = activePresetByMeter[selected.key];

  function selectMeter(meter: MeterSchema) {
    selected = meter;
  }

  function applyPreset(name: string) {
    const preset = selected.presets.find((p) => p.name === name);
    if (!preset) return;
    valuesByMeter = { ...valuesByMeter, [selected.key]: { ...preset.values } };
    activePresetByMeter = { ...activePresetByMeter, [selected.key]: name };
  }

  function setParam(key: string, value: number | boolean) {
    valuesByMeter = { ...valuesByMeter, [selected.key]: { ...values, [key]: value } };
    activePresetByMeter = { ...activePresetByMeter, [selected.key]: 'CUSTOM' };
  }

  function resetToTemplate() {
    valuesByMeter = { ...valuesByMeter, [selected.key]: defaultValues(selected) };
    activePresetByMeter = { ...activePresetByMeter, [selected.key]: selected.presets[0]?.name ?? 'CUSTOM' };
  }

  const costLabel = (costClass: number) => (['Baseline', 'Modest', 'Higher'][costClass] ?? `Class ${costClass}`);
</script>
<section class="page" aria-labelledby="meters-title">
  <header><div><p class="eyebrow">METER LAB</p><h1 id="meters-title">Meter Lab</h1><p class="lede">Scaffold only: every meter and parameter below is a placeholder, not tau-alpha's real registry. Nothing here writes to a card yet.</p></div></header>

  <section class="settings-card meters-placeholder-notice">
    <p><strong>Not real yet.</strong> tau-alpha's own meter-module work (<code>docs/METER_MODULE_SPEC.md</code>) is still mid-build; this screen proves out the editor/preset/preview structure against a synthetic schema so the real one drops in later without a rewrite.</p>
  </section>

  <div class="meters-layout">
    <section class="meter-list" aria-label="Meters">
      {#each SYNTHETIC_METERS as meter (meter.key)}
        <button class="meter-list-item" class:active={selected.key === meter.key} on:click={() => selectMeter(meter)}>
          <span class="meter-list-name">{meter.name}</span>
          <span class="meter-list-cost">{costLabel(meter.cost_class)}</span>
        </button>
      {/each}
    </section>

    <section class="meter-detail" aria-label="Meter editor">
      <div class="meter-detail-header">
        <h2>{selected.name}</h2>
        <select aria-label="Preset" value={activePreset} on:change={(e) => applyPreset(e.currentTarget.value)}>
          {#each selected.presets as preset}<option value={preset.name}>{preset.name}</option>{/each}
          <option value="CUSTOM" disabled selected={activePreset === 'CUSTOM'}>CUSTOM</option>
        </select>
        <button class="quiet" on:click={resetToTemplate}>Reset</button>
      </div>

      <MeterPreview meter={selected} {values} />

      <div class="cost-panel">
        <span>Cost class: <strong>{costLabel(selected.cost_class)}</strong></span>
        <span class="model-estimate">model estimate — no hardware Check has measured this meter</span>
      </div>

      <div class="meter-params">
        {#each selected.params as param (param.key)}
          {#if paramVisible(param, values)}
            <label class="meter-param">
              <span class="meter-param-label">{param.label}{param.unit ? ` (${param.unit})` : ''}</span>
              {#if param.type === 'bool'}
                <input type="checkbox" checked={Boolean(values[param.key])} on:change={(e) => setParam(param.key, e.currentTarget.checked)} />
              {:else if param.type === 'enum'}
                <select value={values[param.key]} on:change={(e) => setParam(param.key, Number(e.currentTarget.value))}>
                  {#each param.values ?? [] as option, i}<option value={i}>{option}</option>{/each}
                </select>
              {:else}
                <span class="meter-param-range">
                  <input type="range" min={param.min} max={param.max} step={param.step ?? 1} value={values[param.key]} on:input={(e) => setParam(param.key, Number(e.currentTarget.value))} />
                  <output>{values[param.key]}</output>
                </span>
              {/if}
            </label>
          {/if}
        {/each}
      </div>
    </section>
  </div>
</section>
<style>
  .meters-placeholder-notice p { margin: 0; color: #d9c47e; font-size: 13px; line-height: 1.5; }
  .meters-placeholder-notice code { background: #101617; padding: 1px 5px; border-radius: 4px; }
  .meters-layout { display: grid; grid-template-columns: 220px 1fr; gap: 16px; margin-top: 16px; }
  .meter-list { display: grid; gap: 8px; align-content: start; }
  .meter-list-item { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; padding: 12px 14px; border-radius: 10px; border: 1px solid #2c393a; background: #1a2325; color: #c7d1d0; text-align: left; }
  .meter-list-item.active { border-color: #c1f0ad; }
  .meter-list-name { font-weight: 650; font-size: 13px; color: #e8ecec; }
  .meter-list-cost { font-size: 11px; color: #8f9e9d; }
  .meter-detail { border: 1px solid #2c393a; border-radius: 15px; padding: 22px 24px; background: #1a2325; display: grid; gap: 16px; align-content: start; }
  .meter-detail-header { display: flex; align-items: center; gap: 10px; }
  .meter-detail-header h2 { margin: 0; flex: 1; }
  .meter-detail-header select { background: #101617; color: #e8ecec; border: 1px solid #3b4a4b; border-radius: 8px; padding: 8px 10px; font: inherit; font-size: 13px; }
  .cost-panel { display: flex; align-items: center; gap: 12px; font-size: 12px; color: #aab8b7; }
  .model-estimate { color: #718180; }
  .meter-params { display: grid; gap: 12px; }
  .meter-param { display: grid; grid-template-columns: 160px 1fr; align-items: center; gap: 12px; font-size: 13px; color: #c7d1d0; }
  .meter-param-range { display: flex; align-items: center; gap: 10px; }
  .meter-param-range input[type='range'] { flex: 1; }
  .meter-param-range output { min-width: 42px; text-align: right; color: #8f9e9d; font-size: 12px; }
  @media (max-width: 720px) { .meters-layout { grid-template-columns: 1fr; } }
</style>
