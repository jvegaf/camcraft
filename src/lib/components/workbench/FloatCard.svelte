<script lang="ts">
import type { Snippet } from 'svelte';

interface Props {
  title: string;
  meta?: string;
  class?: string;
  ariaLabel?: string;
  children: Snippet;
}

let {
  title,
  meta,
  class: className = '',
  ariaLabel = title,
  children
}: Props = $props();
</script>

<aside class={`float ${className}`} aria-label={ariaLabel}>
  <div class="head">
    <h2>{title}</h2>
    {#if meta}<span class="meta">{meta}</span>{/if}
  </div>
  <div class="body">
    {@render children()}
  </div>
</aside>

<style>
  .float { position:absolute; background:rgba(23,27,33,.94); backdrop-filter:blur(10px); border:1px solid var(--line-strong); border-radius:var(--radius-overlay); box-shadow:var(--shadow-float); z-index:20; display:flex; flex-direction:column; min-height:0; overflow:hidden; }
  .head { height:38px; padding:0 10px 0 12px; display:flex; align-items:center; gap:8px; border-bottom:1px solid var(--line-hair); background:rgba(30,36,43,.35); flex:none; }
  .head h2 { margin:0; font-size:11px; font-weight:600; letter-spacing:.06em; text-transform:uppercase; color:var(--text-dim); }
  .meta { margin-left:auto; font:11px var(--mono); color:var(--text-off); }
  .body { padding:10px; overflow:auto; min-height:0; }
</style>
