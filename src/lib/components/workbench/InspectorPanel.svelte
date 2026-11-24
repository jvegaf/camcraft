<script lang="ts">
import FloatCard from './FloatCard.svelte';

interface Field {
  label: string;
  value: string;
  unit?: string;
}

interface Props {
  title?: string;
  fields?: Field[];
  stats?: { label: string; value: string }[];
  statusTitle?: string;
  statusText?: string;
}

let {
  title = 'Drill 0.8 / 1.0',
  fields = [
    { label: 'Tipo', value: 'Drill cycle', unit: 'G81' },
    { label: 'Herramienta', value: 'T1 Flat 0.8 mm', unit: 'cambiar' },
    { label: 'Z cut', value: '-1.8', unit: 'mm' },
    { label: 'Feed plunge', value: '120', unit: 'mm/min' }
  ],
  stats = [
    { label: 'Hits', value: '6' },
    { label: 'Herramientas', value: 'T1, T2' },
    { label: 'Tiempo', value: '0:42' }
  ],
  statusTitle = 'Listo para simular.',
  statusText = 'Toolpath válido, sin colisiones detectadas en preview.'
}: Props = $props();
</script>

<FloatCard title="Inspector" meta={title} class="inspector" ariaLabel="Inspector">
  <div class="section">
    <h3>Operación</h3>
    {#each fields as field}
      <div class="field">
        <span class="label">{field.label}</span>
        <div class="input">{field.value}{#if field.unit}<span class="u">{field.unit}</span>{/if}</div>
      </div>
    {/each}
    <dl class="kv">
      {#each stats as stat}
        <dt>{stat.label}</dt><dd>{stat.value}</dd>
      {/each}
    </dl>
  </div>
  <div class="section">
    <h3>Estado</h3>
    <div class="state-card"><strong>{statusTitle}</strong> {statusText}</div>
  </div>
</FloatCard>

<style>
  :global(.inspector) { top:64px; right:16px; width:324px; max-height:calc(100% - 114px); }
  .section { margin-bottom:12px; }
  .section:last-child { margin-bottom:0; }
  .section h3 { margin:0 0 6px; padding:0 4px; font-size:11px; color:var(--text-off); font-weight:500; text-transform:uppercase; letter-spacing:.04em; display:flex; align-items:center; gap:8px; }
  .section h3::after { content:""; flex:1; height:1px; background:var(--line-hair); opacity:.9; }
  .field { display:grid; gap:4px; margin-bottom:10px; }
  .field .label { font-size:11px; color:var(--text-dim); padding:0 1px; }
  .input { height:30px; padding:0 10px; border-radius:var(--radius-control); background:rgba(18,21,26,.72); border:1px solid var(--line-strong); display:flex; align-items:center; justify-content:space-between; font-size:12px; color:var(--text); }
  .input .u { color:var(--text-off); font:11px var(--mono); }
  .kv { display:grid; grid-template-columns:1fr auto; gap:6px 12px; font-size:12px; padding:10px 0 0; border-top:1px solid var(--line-hair); margin:2px 0 0; }
  .kv dt { color:var(--text-dim); } .kv dd { margin:0; font:11px var(--mono); color:var(--text); }
  .state-card { padding:10px; border-radius:6px; background:rgba(94,194,127,.08); border:1px solid rgba(94,194,127,.22); color:var(--text); font-size:12px; line-height:1.45; }
  .state-card strong { color:var(--ok); font-weight:500; }
</style>
