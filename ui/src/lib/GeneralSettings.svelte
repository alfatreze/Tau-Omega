<script lang="ts">
  // Settings -> General: card & core behaviour preferences, update checks, and developer tools.
  import { onMount } from 'svelte';
  import {
    cardMarkerStatus,
    errorMessage,
    getPrefs,
    removeCardMarker,
    setCardMarker,
    setPrefs,
  } from './tau-api';
  import type { MarkerStatus, Prefs, PrefsView } from './types';
  import { confirmRealCard, developerTools } from './devStore';
  import { showToast } from './toastStore';
  import ToggleSwitch from './ToggleSwitch.svelte';

  export let card = '';

  let marker: MarkerStatus | null = null;
  let prefs: PrefsView | null = null;
  let errorMsg = '';

  async function loadMarker() {
    try {
      marker = card ? await cardMarkerStatus(card) : null;
    } catch {
      marker = null;
    }
  }

  async function toggleMarker() {
    if (!prefs) return;
    const on = prefs.card_marker !== 'on';
    errorMsg = '';
    try {
      marker = await setCardMarker(on ? 'on' : 'off', card || null);
      prefs = await getPrefs();
      showToast(on ? 'Spotlight will be kept off Pocket cards.' : 'Cards will no longer get the marker.');
    } catch (error) {
      errorMsg = `Could not save: ${errorMessage(error)}`;
    }
  }

  async function dropMarker() {
    errorMsg = '';
    try {
      marker = await removeCardMarker(card);
      showToast('The marker was removed from this card.');
    } catch (error) {
      errorMsg = `Could not remove it: ${errorMessage(error)}`;
    }
  }

  async function update(change: Partial<Prefs>, message = 'Saved.') {
    if (!prefs) return;
    errorMsg = '';
    try {
      prefs = await setPrefs({ ...prefs, ...change });
      showToast(message);
    } catch (error) {
      errorMsg = `Could not save: ${errorMessage(error)}`;
    }
  }

  onMount(async () => {
    try {
      prefs = await getPrefs();
    } catch (error) {
      errorMsg = `Could not read preferences: ${errorMessage(error)}`;
    }
    await loadMarker();
  });
</script>

{#if errorMsg}
  <p class="gs-err" role="alert">{errorMsg}</p>
{/if}

{#if prefs}
  <section class="gs-card" aria-labelledby="gs-card-title">
    <h2 id="gs-card-title">Cards &amp; cores</h2>
    <div class="ls-row gs-first">
      <div>
        <b>Keep Spotlight off Pocket cards</b>
        <small>
          Adds an empty <code>.metadata_never_index</code> file at the card’s root when Tau Omega writes to a card, so macOS stops scanning it (faster writes, fewer “in use” errors).
          {prefs.card_marker === 'ask' ? 'Not decided yet.' : prefs.card_marker === 'on' ? 'On.' : 'Off.'}{#if marker?.is_card}{' '}This card: {marker.present ? 'has the marker' : 'no marker'}.{/if}
        </small>
      </div>
      <div class="ls-btns">
        <ToggleSwitch checked={prefs.card_marker === 'on'} label="Keep Spotlight off Pocket cards" on:click={toggleMarker} />
        {#if marker?.present}
          <button type="button" class="quiet" on:click={dropMarker}>Remove from this card</button>
        {/if}
      </div>
    </div>

    <div class="ls-row">
      <div>
        <b>Cores the Pocket cannot use</b>
        <small>
          When a core is missing a file it needs, or is broken, it clutters the Pocket’s menu. Nothing is ever deleted: a hidden core is moved to <code>Disabled Cores</code> and can be enabled again from Cores.
        </small>
      </div>
      <div>
        <label class="ls-sel">
          <span class="sr">What to do about cores the Pocket cannot use</span>
          <select
            value={prefs.core_disable}
            on:change={(e) => void update({ core_disable: e.currentTarget.value as 'ask' | 'auto' | 'never' }, 'Saved.')}
          >
            <option value="ask">Ask me when I open a card</option>
            <option value="auto">Hide them automatically</option>
            <option value="never">Never suggest hiding</option>
          </select>
        </label>
      </div>
    </div>

    <div class="ls-row">
      <div>
        <b>Check for Tau updates</b>
        <small>When the app starts, ask GitHub for the public release list (nothing about you or your card is sent). Installing is always your action.</small>
      </div>
      <div>
        <ToggleSwitch
          checked={prefs.check_updates}
          label="Check for Tau updates"
          on:click={() => {
            const on = !prefs?.check_updates;
            void update({ check_updates: on, update_notice_shown: true }, on ? 'Update checks are on.' : 'Update checks are off.');
          }}
        />
      </div>
    </div>

    <div class="ls-row">
      <div>
        <b>Notices</b>
        <small>Show the one-time messages again.</small>
      </div>
      <div>
        <button
          type="button"
          class="quiet"
          disabled={!prefs.remove_explained && !prefs.slow_alert_suppressed && prefs.card_marker === 'ask' && !prefs.update_notice_shown}
          on:click={() =>
            update(
              { remove_explained: false, slow_alert_suppressed: false, update_notice_shown: false, card_marker: 'ask' },
              'Notices will show again.',
            )}
        >
          Reset notices
        </button>
      </div>
    </div>
  </section>

  <section class="gs-card" aria-labelledby="gs-dev-title">
    <h2 id="gs-dev-title">Developer tools</h2>
    <div class="ls-row gs-first">
      <div>
        <b>Show developer tools</b>
        <small>
          For people who build cores. Adds a card role (Real or Test) on the Card page and the Builds, Results, Check and Builder pages. Off by default; nothing changes for everyday use.
        </small>
      </div>
      <div>
        <ToggleSwitch
          checked={prefs.developer_tools}
          label="Show developer tools"
          on:click={async () => {
            const on = !prefs?.developer_tools;
            await update({ developer_tools: on }, on ? 'Developer tools are on.' : 'Developer tools are off.');
            developerTools.set(!!prefs?.developer_tools);
          }}
        />
      </div>
    </div>
    {#if prefs.developer_tools}
      <div class="ls-row">
        <div>
          <b>Ask for the card’s name before installing</b>
          <small>
            When installing onto a card marked as Real, type its name to confirm. Off by default: the reviewed plan and the Install button are the confirmation.
          </small>
        </div>
        <div>
          <ToggleSwitch
            checked={$confirmRealCard}
            label="Ask for the card’s name before installing"
            on:click={() => confirmRealCard.update((v) => !v)}
          />
        </div>
      </div>
    {/if}
  </section>
{:else if !errorMsg}
  <section class="gs-card"><p class="gs-loading">Loading preferences…</p></section>
{/if}

<style>
  .gs-card {
    max-width: 820px;
    margin-bottom: 14px;
    padding: 22px 24px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg-card);
  }
  h2 {
    margin: 0 0 12px;
  }
  .gs-loading {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }
  .gs-err {
    max-width: 820px;
    margin: 0 0 12px;
    padding: 10px 14px;
    border-radius: 10px;
    border: 1px solid var(--danger-border);
    background: var(--danger-bg);
    color: var(--danger-text);
    font-size: 13px;
  }
  .ls-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--border);
  }
  .ls-row.gs-first {
    border-top: 0;
    padding-top: 4px;
  }
  .ls-row > div:first-child {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .ls-row small {
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
  }
  .ls-btns {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .ls-sel select {
    font: inherit;
    font-size: 13px;
    color: var(--text);
    background: var(--bg-input);
    border: 1px solid var(--border-strong);
    border-radius: 9px;
    padding: 8px 10px;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }
</style>
