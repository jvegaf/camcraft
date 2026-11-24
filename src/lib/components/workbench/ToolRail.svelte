<script lang="ts">
interface Tool {
  label: string;
  primary?: boolean;
}

interface Props {
  tools?: Tool[];
}

let {
  tools = [
    { label: 'Importar' },
    { label: 'Capas' },
    { label: 'Origen' },
    { label: 'Generar plan', primary: true }
  ]
}: Props = $props();
</script>

<div class="tool-rail" aria-label="Acciones rápidas">
  {#each tools as tool}
    <button class="tool" class:primary={tool.primary}>{tool.label}</button>
  {/each}
</div>

<style>
  .tool-rail { position:absolute; top:64px; left:316px; display:flex; gap:6px; z-index:20; }
  .tool { height:32px; padding:0 10px; border-radius:6px; background:rgba(23,27,33,.92); border:1px solid var(--line-strong); font-size:12px; color:var(--text); display:inline-flex; align-items:center; gap:6px; box-shadow:0 4px 16px rgba(0,0,0,.18); cursor:pointer; font:inherit; }
  .tool:hover { background:var(--bg-raised); color:var(--text-hi); }
  .tool.primary { color:var(--cut); border-color:rgba(245,166,35,.28); background:rgba(35,30,20,.92); }
  .tool:focus-visible { outline:2px solid var(--sel); outline-offset:-1px; }
</style>
