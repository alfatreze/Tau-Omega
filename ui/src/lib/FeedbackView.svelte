<script lang="ts">
  // Send feedback: a message that the project's feedback service files as a public GitHub issue (see docs/FEEDBACK_SERVICE.md).
  // This page is the whole sender side: pick what it is and where, write it, attach a few pictures or files, and send it. Until the service is connected the page works for writing and reviewing but Send is off.
  import { onDestroy, onMount } from 'svelte';
  import { feedbackDraft as draft, feedbackFiles as files } from './feedbackStore';
  import { ACCEPT, AREAS, KINDS, LIMITS, TYPE_NAMES, checkFile, emptyDraft, isImage, megabytes, osName, problems } from './feedback';
  import { errorMessage } from './backend';
  import { feedbackInfo, feedbackOpenIssue, feedbackSend } from './tau-api';
  import type { FeedbackInfo, FeedbackSent } from './types';

  let info: FeedbackInfo | null = null;
  let fileErrors: string[] = []; let sending = false; let sendError = ''; let sent: FeedbackSent | null = null; let picker: HTMLInputElement;
  onMount(async () => { try { info = await feedbackInfo(); } catch { info = { enabled: false, version: '', os: '', arch: '' }; } });

  $: kind = KINDS.find((k) => k.id === $draft.kind)!;
  $: basics = info && $draft.basics && info.version ? { version: info.version, os: info.os, arch: info.arch } : null;
  $: todo = problems($draft, $files.length);
  $: total = $files.reduce((n, f) => n + f.size, 0);
  $: canSend = !!info?.enabled && todo.length === 0 && !sending;

  // Pictures are shown from short-lived local addresses, released when the file is removed or the page closes.
  const urls = new Map<File, string>();
  const urlOf = (f: File) => { let u = urls.get(f); if (!u) { u = URL.createObjectURL(f); urls.set(f, u); } return u; };
  const release = (f: File) => { const u = urls.get(f); if (u) { URL.revokeObjectURL(u); urls.delete(f); } };
  onDestroy(() => { for (const u of urls.values()) URL.revokeObjectURL(u); urls.clear(); });

  function add(list: FileList | File[] | null | undefined) {
    if (!list) return;
    const errs: string[] = []; let have = $files;
    for (const f of Array.from(list)) {
      const why = checkFile({ name: f.name, type: f.type, size: f.size }, have);
      if (why) errs.push(why); else have = [...have, f];
    }
    files.set(have); fileErrors = errs;
  }
  const remove = (f: File) => { release(f); files.update((l) => l.filter((x) => x !== f)); fileErrors = []; };
  function onPaste(e: ClipboardEvent) { const pics = Array.from(e.clipboardData?.files ?? []); if (pics.length) { e.preventDefault(); add(pics); } }
  const toBase64 = (f: File) => new Promise<string>((resolve, reject) => { const r = new FileReader(); r.onload = () => resolve(String(r.result).split(',')[1] ?? ''); r.onerror = () => reject(new Error(`${f.name} could not be read.`)); r.readAsDataURL(f); });

  async function send() {
    if (!canSend) return;
    sending = true; sendError = '';
    try {
      const data = await Promise.all($files.map(async (f) => ({ name: f.name, mime: f.type, data: await toBase64(f) })));
      sent = await feedbackSend({ kind: $draft.kind, area: $draft.area, title: $draft.title.trim(), text: $draft.text.trim(), basics, github: $draft.github.trim(), files: data });
      for (const f of $files) release(f);
      draft.set(emptyDraft()); files.set([]); fileErrors = [];
    } catch (e) { sendError = errorMessage(e); }
    finally { sending = false; }
  }
  async function openIssue(e: Event) { e.preventDefault(); if (sent) { try { await feedbackOpenIssue(sent.number); } catch { /* the number stays visible */ } } }
  const again = () => { sent = null; sendError = ''; };
</script>
<section class="page fb" aria-labelledby="fb-title">
  <header><div><p class="eyebrow">FEEDBACK</p><h1 id="fb-title">Send feedback</h1><p class="lede">Tell us what to fix or improve. Your message becomes a public issue on the Tau Omega GitHub page, where you and others can follow it.</p></div></header>

  {#if sent}
    <div class="fb-card fb-done" role="status">
      <h2>Thank you</h2>
      <p>Your feedback is now issue <strong>#{sent.number}</strong>. <a class="fb-link" href={sent.url} on:click={openIssue}>Open it on GitHub</a></p>
      <button class="primary" on:click={again}>Send more feedback</button>
    </div>
  {:else}
    {#if info && !info.enabled}<p class="fb-callout" role="note"><strong>Sending is not connected yet.</strong> You can write and check your feedback here, but nothing is sent.</p>{/if}

    <form class="fb-card" on:submit|preventDefault={send} novalidate>
      <fieldset class="fb-kinds"><legend>What is it?</legend>
        {#each KINDS as k}
          <label class="fb-kind" class:on={$draft.kind === k.id}><input type="radio" name="fb-kind" value={k.id} bind:group={$draft.kind} /><span class="fb-kind-name">{k.label}</span><span class="fb-kind-hint">{k.hint}</span></label>
        {/each}
      </fieldset>

      <div class="fb-row">
        <label for="fb-area">Where in Omega?</label>
        <select id="fb-area" bind:value={$draft.area}>{#each AREAS as a}<option value={a}>{a}</option>{/each}</select>
      </div>

      <div class="fb-row">
        <label for="fb-subject">Title</label>
        <input id="fb-subject" type="text" maxlength={LIMITS.title} placeholder="A few words" bind:value={$draft.title} autocomplete="off" />
        <small class="fb-count">{$draft.title.length} / {LIMITS.title}</small>
      </div>

      <div class="fb-row">
        <label for="fb-text">Description</label>
        <textarea id="fb-text" rows="8" maxlength={LIMITS.text} placeholder={kind.placeholder} bind:value={$draft.text} on:paste={onPaste}></textarea>
        <small class="fb-count">{$draft.text.length} / {LIMITS.text}</small>
      </div>

      <div class="fb-row">
        <span class="fb-label" id="fb-files-label">Pictures or files <span class="fb-opt">(optional)</span></span>
        <div class="fb-attach">
          <button type="button" class="fb-add" aria-describedby="fb-files-limits" disabled={$files.length >= LIMITS.files} on:click={() => picker.click()}>Add pictures or files…</button>
          <input bind:this={picker} class="fb-picker" type="file" multiple accept={ACCEPT} tabindex="-1" aria-labelledby="fb-files-label" on:change={(e) => { add(e.currentTarget.files); e.currentTarget.value = ''; }} />
          <small id="fb-files-limits">Up to {LIMITS.files} files, {megabytes(LIMITS.fileBytes)} each, {megabytes(LIMITS.totalBytes)} in all: {TYPE_NAMES}. You can also paste a picture into the description.</small>
        </div>
        {#if fileErrors.length}<ul class="fb-errors" role="alert">{#each fileErrors as e}<li>{e}</li>{/each}</ul>{/if}
        {#if $files.length}
          <ul class="fb-files" aria-label="Attached files">
            {#each $files as f}
              <li>
                {#if isImage(f.name)}<img src={urlOf(f)} alt="" />{:else}<span class="fb-doc" aria-hidden="true">{f.name.split('.').pop()?.toUpperCase()}</span>{/if}
                <span class="fb-fname">{f.name}<small>{megabytes(f.size)}</small></span>
                <button type="button" class="quiet" aria-label={`Remove ${f.name}`} on:click={() => remove(f)}>Remove</button>
              </li>
            {/each}
          </ul>
          <small class="fb-total">{$files.length} {$files.length === 1 ? 'file' : 'files'}, {megabytes(total)} of {megabytes(LIMITS.totalBytes)}</small>
        {/if}
      </div>

      <div class="fb-row fb-opts">
        <label class="fb-check"><input type="checkbox" bind:checked={$draft.basics} /> <span>Add my Omega version and system{#if info?.version}{' '}<small>(Omega {info.version}, {osName(info.os)}, {info.arch})</small>{/if}<small class="fb-sub">Nothing about your card, music or computer's files is ever added.</small></span></label>
        <div class="fb-github"><label for="fb-gh">Your GitHub name <span class="fb-opt">(optional)</span></label><input id="fb-gh" type="text" maxlength={LIMITS.github} placeholder="so we can reply to you there" bind:value={$draft.github} autocomplete="off" spellcheck="false" /></div>
      </div>

      <p class="fb-callout" role="note"><strong>This will be public.</strong> Your message and anything you attach can be seen by everyone on GitHub. Leave out anything private, such as personal details or the names of files on your card.</p>

      <div class="fb-send">
        <button class="primary" type="submit" disabled={!canSend}>{sending ? 'Sending…' : 'Send feedback'}</button>
        {#if info && !info.enabled}<small>Sending is not connected yet.</small>
        {:else if todo.length && ($draft.title || $draft.text)}<small class="fb-todo">{todo[0]}</small>{/if}
      </div>
      {#if sendError}<p class="fb-error" role="alert">{sendError}</p>{/if}
    </form>
  {/if}
</section>
<style>
  .fb{max-width:820px}
  .fb-card{background:var(--bg-surface);border:1px solid var(--border);border-radius:14px;padding:26px 28px;display:grid;gap:22px}
  .fb-done{gap:12px}.fb-done h2{margin:0}.fb-done p{margin:0;color:var(--text-secondary);line-height:1.6}.fb-done .primary{justify-self:start;margin-top:6px}
  .fb-callout{margin:0 0 16px;padding:12px 14px;border:1px solid var(--border-strong);border-left:3px solid var(--accent);border-radius:10px;background:var(--bg-card);color:var(--text-secondary);font-size:13px;line-height:1.55}
  .fb-card .fb-callout{margin:0}
  fieldset{border:0;margin:0;padding:0;min-width:0}
  legend,.fb-row>label,.fb-github label,.fb-label{font-size:12px;font-weight:650;color:var(--text-secondary);margin-bottom:8px;padding:0;display:block}
  .fb-opt{font-weight:500;color:var(--text-muted)}
  .fb-kinds{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:10px}.fb-kinds legend{grid-column:1/-1}
  .fb-kind{position:relative;display:grid;gap:3px;padding:12px 14px;border:1px solid var(--border-strong);border-radius:10px;background:var(--bg-input);cursor:pointer}
  .fb-kind input{position:absolute;opacity:0;inset:0;cursor:pointer}
  .fb-kind.on{border-color:var(--accent);background:var(--info-bg)}.fb-kind:has(input:focus-visible){outline:2px solid var(--accent);outline-offset:2px}
  .fb-kind-name{font-size:14px;font-weight:650;color:var(--text)}.fb-kind-hint{font-size:12px;color:var(--text-muted);line-height:1.4}
  .fb-row{display:grid}.fb-row input[type=text],.fb-row select,.fb-row textarea{width:100%;box-sizing:border-box;font:inherit;font-size:14px;color:var(--text);background:var(--bg-input);border:1px solid var(--border-strong);border-radius:8px;padding:11px 12px}
  .fb-row select{max-width:280px}.fb-row textarea{resize:vertical;min-height:140px;line-height:1.55}
  .fb-row::is(input,select,textarea):focus-visible{outline:2px solid var(--accent);outline-offset:1px;border-color:var(--accent)}
  .fb-count{justify-self:end;margin-top:5px;font-size:11px;color:var(--text-muted)}
  .fb-attach{display:flex;flex-wrap:wrap;align-items:center;gap:6px 14px}.fb-attach small{font-size:12px;color:var(--text-muted);line-height:1.5;flex:1 1 260px}
  .fb-add{padding:10px 14px;background:var(--bg-hover);color:var(--text);border:1px solid var(--border-strong);font-weight:600}.fb-add:hover:not(:disabled){border-color:var(--accent)}.fb-add:disabled{opacity:.5;cursor:not-allowed}
  .fb-picker{position:absolute;width:1px;height:1px;opacity:0;pointer-events:none}
  .fb-errors{margin:10px 0 0;padding:0;list-style:none;color:var(--danger-text);font-size:12.5px;display:grid;gap:4px}
  .fb-files{list-style:none;margin:12px 0 0;padding:0;display:grid;gap:8px}
  .fb-files li{display:flex;align-items:center;gap:12px;padding:8px 10px;border:1px solid var(--border);border-radius:10px;background:var(--bg-input)}
  .fb-files img{width:56px;height:40px;object-fit:cover;border-radius:6px;background:#000;flex:none}
  .fb-doc{width:56px;height:40px;flex:none;display:grid;place-items:center;border-radius:6px;background:var(--bg-hover);color:var(--accent);font-size:11px;font-weight:700;letter-spacing:.06em}
  .fb-fname{flex:1;min-width:0;display:grid;font-size:13px;color:var(--text);overflow-wrap:anywhere}.fb-fname small{color:var(--text-muted);font-size:11px}
  .fb-total{margin-top:6px;font-size:11px;color:var(--text-muted)}
  .fb-opts{gap:14px}
  .fb-check{display:flex;gap:10px;align-items:flex-start;font-size:13px;color:var(--text);font-weight:500;margin:0;cursor:pointer}.fb-check input{margin-top:3px;flex:none}
  .fb-check small{color:var(--text-muted)}.fb-sub{display:block;margin-top:3px;font-size:12px}
  .fb-github{display:grid;max-width:340px}
  .fb-send{display:flex;align-items:center;gap:14px;flex-wrap:wrap}.fb-send small{color:var(--text-muted);font-size:12px}.fb-todo{color:var(--warn-text)!important}
  .fb-error{margin:0;color:var(--danger-text);font-size:13px}
  .fb-link{color:var(--accent-soft-text);text-decoration:underline;text-underline-offset:2px;cursor:pointer}
  @media(max-width:720px){.fb-kinds{grid-template-columns:1fr}.fb-card{padding:20px 16px}}
</style>
