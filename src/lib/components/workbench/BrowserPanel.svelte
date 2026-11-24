<script lang="ts">
import FloatCard from './FloatCard.svelte';

interface NavItem {
  label: string;
  meta?: string;
  dot?: 'copper' | 'drill' | 'cut' | 'ok' | 'default';
  active?: boolean;
}

interface Section {
  title: string;
  items: NavItem[];
}

interface Props {
  sections?: Section[];
}

let {
  sections = [
    {
      title: 'Board',
      items: [
        { label: 'Top Copper', meta: '.gbr', dot: 'copper' },
        { label: 'Bottom Copper', meta: '.gbr', dot: 'copper' },
        { label: 'Drill Hits', meta: '.drl', dot: 'drill', active: true },
        { label: 'Board Outline', meta: '.gbr', dot: 'default' }
      ]
    },
    {
      title: 'Setup',
      items: [
        { label: 'Stock FR-4 1.6mm', meta: '100×80', dot: 'ok' },
        { label: 'Origen bottom-left', meta: 'X0 Y0', dot: 'ok' }
      ]
    },
    {
      title: 'Operaciones',
      items: [
        { label: '1 · Profile outline', meta: 'OK', dot: 'cut' },
        {
          label: '2 · Drill 0.8 / 1.0',
          meta: 'seleccionada',
          dot: 'drill',
          active: true
        },
        { label: '3 · Isolation top', meta: 'draft', dot: 'copper' }
      ]
    },
    {
      title: 'Salida',
      items: [{ label: 'Post GRBL', meta: '.nc', dot: 'default' }]
    }
  ]
}: Props = $props();
</script>

<FloatCard title="Browser" meta="Proyecto" class="browser" ariaLabel="Browser">
  {#each sections as section}
    <div class="section">
      <h3>{section.title}</h3>
      {#each section.items as item}
        <button class="nav-item" aria-current={item.active ? 'true' : undefined}>
          <span class="dot {item.dot ?? 'default'}"></span>
          {item.label}
          {#if item.meta}<span class="count">{item.meta}</span>{/if}
        </button>
      {/each}
    </div>
  {/each}
</FloatCard>

<style>
  :global(.browser) { top:64px; left:16px; width:284px; max-height:calc(100% - 114px); }
  .section { margin-bottom:12px; }
  .section:last-child { margin-bottom:0; }
  .section h3 { margin:0 0 6px; padding:0 4px; font-size:11px; color:var(--text-off); font-weight:500; text-transform:uppercase; letter-spacing:.04em; display:flex; align-items:center; gap:8px; }
  .section h3::after { content:""; flex:1; height:1px; background:var(--line-hair); opacity:.9; }
  .nav-item { width:100%; display:flex; align-items:center; gap:8px; padding:6px 8px; border-radius:5px; text-align:left; font-size:12px; color:var(--text); cursor:pointer; font:inherit; background:none; border:none; }
  .nav-item:hover { background:var(--bg-raised); }
  .nav-item[aria-current="true"] { background:var(--sel-soft); color:var(--text-hi); box-shadow:inset 0 0 0 1px rgba(111,211,232,.12); }
  .nav-item:focus-visible { outline:2px solid var(--sel); outline-offset:-1px; }
  .dot { width:8px; height:8px; border-radius:50%; background:var(--line-strong); flex:none; }
  .dot.copper{background:#c47a2c}.dot.drill{background:#7cc7ff}.dot.cut{background:var(--cut)}.dot.ok{background:var(--ok)}
  .count { margin-left:auto; font:11px var(--mono); color:var(--text-off); white-space:nowrap; }
</style>
