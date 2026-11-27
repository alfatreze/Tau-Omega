<script lang="ts">
  // The core checker's findings as a list: severity, the sentence, the fix and the documentation rule it comes from.
  import type { LintReport } from './types';
  import { plural } from './coresFormat';
  export let lint: LintReport;
</script>
<p class="lf-sum" role="status">{lint.errors ? `${plural(lint.errors, 'error')} found` : 'No errors'}{lint.warnings ? `, ${plural(lint.warnings, 'warning')}` : ''}{lint.notes ? `, ${plural(lint.notes, 'note')}` : ''}. Each finding names the documentation rule it comes from.</p>
<ul class="lf" aria-label="Core check findings">{#each lint.findings as f}
  <li class={f.severity.toLowerCase()}><span class="chip {f.severity === 'Error' ? 'bad' : f.severity === 'Warning' ? 'warn' : ''}">{f.severity}</span><div><span>{f.message}</span><small>{f.fix} <em>({f.rule})</em></small></div></li>
{/each}</ul>
<style>
  .lf-sum{font-size:12px;color:var(--text-muted);line-height:1.55;margin:6px 0}.lf{list-style:none;margin:6px 0;padding:0;border:1px solid var(--border);border-radius:9px}
  .lf li{display:flex;gap:10px;align-items:flex-start;padding:9px 12px;border-bottom:1px solid var(--border);font-size:12.5px}.lf li:last-child{border:0}.lf small{display:block;color:var(--text-muted);font-size:11.5px;margin-top:2px}.lf em{font-style:normal;color:var(--text-muted)}
  .chip.bad{background:var(--danger-bg);color:var(--danger-text)}.chip.warn{background:var(--warn-bg);color:var(--warn-text)}
</style>
