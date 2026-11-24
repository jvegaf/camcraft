<script lang="ts">
import { css } from 'styled-system/css';
import FloatCard from './FloatCard.svelte';
import { workbenchState } from './workbench.svelte';

interface Props {
  onOpenRecent?: (path: string) => void;
}

let { onOpenRecent }: Props = $props();

const panelPos = css({
  top: '64px',
  left: '16px',
  w: '284px',
  maxH: 'calc(100% - 114px)'
});
const sectionCls = css({ mb: '12px', _last: { mb: '0' } });
const sectionTitle = css({
  m: '0 0 6px',
  px: '4px',
  fontSize: '11px',
  color: 'textOff',
  fontWeight: '500',
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  _after: { content: '""', flex: '1', h: '1px', bg: 'hairline', opacity: '0.9' }
});
const navItem = css({
  w: '100%',
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  px: '8px',
  py: '6px',
  borderRadius: '5px',
  textAlign: 'left',
  fontSize: '12px',
  color: 'text',
  bg: 'transparent',
  border: 'none'
});
const recentBtn = css({ cursor: 'pointer', _hover: { bg: 'raised' } });
const dotBase = css({ w: '8px', h: '8px', borderRadius: '50%', bg: 'border', flex: 'none' });
const count = css({ ml: 'auto', fontSize: '11px', fontFamily: 'mono', color: 'textOff', whiteSpace: 'nowrap' });
const headBtn = css({
  w: '24px', h: '24px', display: 'grid', placeItems: 'center',
  borderRadius: '4px', color: 'textOff', cursor: 'pointer',
  _hover: { bg: 'raised', color: 'text' }
});
const icon = css({ w: '14px', h: '14px', stroke: 'currentColor', fill: 'none', strokeWidth: '1.6', strokeLinecap: 'round', strokeLinejoin: 'round' });
</script>

<FloatCard title="Browser" meta={workbenchState.document?.path?.split('/').pop() ?? 'Proyecto'} class={panelPos} ariaLabel="Browser">
  {#snippet headAction()}
    <button class={headBtn} aria-label="Buscar en browser">
      <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6"/><path d="M20 20l-4.2-4.2"/></svg>
    </button>
  {/snippet}

  <div class={sectionCls}>
    <h3 class={sectionTitle}>Documento</h3>
    <div class={navItem}><span class={dotBase} style="background:#5ec27f"></span>Schema <span class={count}>{workbenchState.document?.documentSchema ?? '—'}</span></div>
    <div class={navItem}><span class={dotBase} style="background:#7cc7ff"></span>Dirty <span class={count}>{workbenchState.document ? (workbenchState.document.dirty ? 'yes' : 'no') : '—'}</span></div>
  </div>
  <div class={sectionCls}>
    <h3 class={sectionTitle}>Board</h3>
    <div class={navItem}><span class={dotBase} style="background:#c47a2c"></span>Tamaño <span class={count}>{workbenchState.document ? `${workbenchState.document.boardWidthMm}×${workbenchState.document.boardHeightMm}` : '—'}</span></div>
    <div class={navItem}><span class={dotBase} style="background:#c47a2c"></span>Espesor <span class={count}>{workbenchState.document ? `${workbenchState.document.boardThicknessMm} mm` : '—'}</span></div>
  </div>
  <div class={sectionCls}>
    <h3 class={sectionTitle}>Contenido</h3>
    <div class={navItem}><span class={dotBase} style="background:#6fd3e8"></span>Capas <span class={count}>{workbenchState.document?.layerCount ?? '—'}</span></div>
    <div class={navItem}><span class={dotBase} style="background:#f5a623"></span>Toolpaths <span class={count}>{workbenchState.document?.toolpathCount ?? '—'}</span></div>
  </div>
  <div class={sectionCls}>
    <h3 class={sectionTitle}>Recientes</h3>
    {#if workbenchState.recents.length === 0}
      <div class={navItem}><span class={dotBase} style="background:#35404c"></span>Sin recientes</div>
    {:else}
      {#each workbenchState.recents.slice(0, 5) as recent}
        <button class={`${navItem} ${recentBtn}`} onclick={() => onOpenRecent?.(recent.path)}>
          <span class={dotBase} style="background:#8aa0b6"></span>
          {recent.name}
        </button>
      {/each}
    {/if}
  </div>
</FloatCard>
