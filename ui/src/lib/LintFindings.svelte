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
  .lf-sum{font-size:12px;color:#95a5a4;line-height:1.55;margin:6px 0}.lf{list-style:none;margin:6px 0;padding:0;border:1px solid #2c393a;border-radius:9px}
  .lf li{display:flex;gap:10px;align-items:flex-start;padding:9px 12px;border-bottom:1px solid #2c393a;font-size:12.5px}.lf li:last-child{border:0}.lf small{display:block;color:#95a5a4;font-size:11.5px;margin-top:2px}.lf em{font-style:normal;color:#7f8e8d}
  .chip.bad{background:#3a1c1c;color:#ffb4b4}.chip.warn{background:#3a2e14;color:#e5d9a8}
</style>
