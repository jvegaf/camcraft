<script lang="ts">
interface Props {
  mode?: string;
  selection?: string;
}

let {
  mode = '2D Board · Grid 1 mm',
  selection = '6 taladros seleccionados'
}: Props = $props();
</script>

<div class="hud" aria-label="Estado de viewport">
  <div class="hud-card"><b>{mode.split(' · ')[0]}</b>{#if mode.includes(' · ')} · {mode.split(' · ').slice(1).join(' · ')}{/if}</div>
  <div class="hud-card"><span class="sel">{selection}</span></div>
</div>

<style>
  .hud { position:absolute; right:356px; top:64px; display:flex; gap:8px; z-index:20; }
  .hud-card { padding:7px 10px; border-radius:6px; background:rgba(23,27,33,.9); border:1px solid var(--line-strong); font-size:11px; color:var(--text-dim); box-shadow:0 4px 16px rgba(0,0,0,.18); }
  .hud-card b { color:var(--text); font-weight:500; }
  .sel { color:var(--sel); }
</style>
