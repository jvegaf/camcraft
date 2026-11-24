<script lang="ts">
interface Props {
  x?: string;
  y?: string;
  z?: string;
  message?: string;
  status?: string;
}

let {
  x = 'X 42.80',
  y = 'Y 31.20',
  z = 'Z 2.00',
  message = 'mm · Grid 1 mm · GRBL',
  status = '● Sin errores'
}: Props = $props();
</script>

<footer class="status" aria-label="Barra de estado">
  <span class="mono">{x}</span>
  <span class="mono">{y}</span>
  <span class="mono">{z}</span>
  <span class="sep" aria-hidden="true"></span>
  <span>{message}</span>
  <span class="spacer"></span>
  <span class="ok">{status}</span>
</footer>

<style>
  .status { position:absolute; left:0; right:0; bottom:0; height:34px; display:flex; align-items:center; gap:12px; padding:0 14px; background:rgba(23,27,33,.92); backdrop-filter:blur(8px); border-top:1px solid var(--line-hair); font-size:11px; color:var(--text-dim); z-index:30; }
  .mono { font:11px var(--mono); color:var(--text); }
  .ok { color:var(--ok); }
  .spacer { flex:1; }
  .sep { width:1px; height:14px; background:var(--line-hair); }
</style>
