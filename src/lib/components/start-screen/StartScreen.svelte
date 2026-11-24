<script lang="ts">
import { css } from 'styled-system/css';
import { button, input } from 'styled-system/recipes';

interface RecentItem {
  name: string;
  path: string;
  when?: string;
  lastOpened?: string;
}

interface Props {
  recents?: RecentItem[];
  error?: string;
  showNew?: boolean;
  onCreate?: (request: {
    widthMm: number;
    heightMm: number;
    thicknessMm: number;
  }) => void;
  onOpen?: () => void;
  onOpenRecent?: (path: string) => void;
  onSettings?: () => void;
}

let {
  recents = [],
  error = '',
  showNew = $bindable(false),
  onCreate,
  onOpen,
  onOpenRecent,
  onSettings
}: Props = $props();
let widthMm = $state(1219);
let heightMm = $state(2438);
let thicknessMm = $state(18);

function recentWhen(item: RecentItem): string {
  return item.when || item.lastOpened || '';
}

function submitNew(event: Event) {
  event.preventDefault();
  onCreate?.({
    widthMm: Number(widthMm),
    heightMm: Number(heightMm),
    thicknessMm: Number(thicknessMm)
  });
}

const startShell = css({
  h: '100%',
  minH: '100%',
  overflow: 'auto',
  bg: 'canvas',
  position: 'relative',
  display: 'grid',
  placeItems: 'center',
  color: 'text',
  fontSize: '13px',
  lineHeight: '1.4',
  fontFamily: 'sans',
  _before: {
    content: '""',
    position: 'absolute',
    inset: '0',
    background:
      'linear-gradient(90deg, rgba(38,45,54,.22) 1px, transparent 1px) 0 0/80px 80px, linear-gradient(180deg, rgba(38,45,54,.22) 1px, transparent 1px) 0 0/80px 80px'
  }
});
const startCard = css({
  position: 'relative',
  w: '940px',
  maxW: 'calc(100% - 48px)',
  display: 'grid',
  gridTemplateColumns: { base: '1fr', md: '1fr 1fr' },
  gap: '16px'
});
const panel = css({
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: '12px',
  boxShadow: '0 1px 0 rgba(255,255,255,.03), 0 12px 32px rgba(0,0,0,.42)',
  p: '24px'
});
const eyebrow = css({
  fontSize: '11px',
  letterSpacing: '0.08em',
  textTransform: 'uppercase',
  color: 'textOff',
  m: '0 0 6px'
});
const welcome = css({
  m: '0 0 6px',
  fontSize: '28px',
  fontWeight: '600',
  lineHeight: '1.15',
  color: 'textHi'
});
const heading = css({
  m: '0',
  fontSize: '20px',
  fontWeight: '600',
  color: 'textHi'
});
const lead = css({
  mt: '8px',
  mb: '0',
  color: 'textDim',
  fontSize: '13px',
  lineHeight: '1.5'
});
const action = css({
  w: '100%',
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
  p: '14px',
  mt: '12px',
  borderRadius: '8px',
  bg: 'raised',
  border: '1px solid',
  borderColor: 'hairline',
  textAlign: 'left',
  cursor: 'pointer',
  color: 'inherit',
  _hover: { borderColor: 'sel', bg: 'overlay' }
});
const actionIcon = css({
  w: '34px',
  h: '34px',
  borderRadius: '8px',
  display: 'grid',
  placeItems: 'center',
  bg: 'selSoft',
  color: 'sel',
  flex: 'none'
});
const actionIconPrimary = css({ bg: 'cutSoft', color: 'cut' });
const actionTitle = css({
  display: 'block',
  fontSize: '14px',
  color: 'textHi',
  fontWeight: '500'
});
const actionDesc = css({
  display: 'block',
  fontSize: '12px',
  color: 'textDim',
  mt: '2px'
});
const recentBtn = css({
  w: '100%',
  display: 'flex',
  gap: '12px',
  alignItems: 'center',
  py: '10px',
  borderBottom: '1px solid',
  borderColor: 'hairline',
  textAlign: 'left',
  bg: 'transparent',
  borderX: 'none',
  borderTop: 'none',
  cursor: 'pointer',
  color: 'inherit',
  _last: { borderBottom: '0', pb: '0' },
  '&:hover .recent-name': { color: 'textHi' }
});
const recentName = css({ fontSize: '13px', color: 'text', fontWeight: '500' });
const recentPath = css({
  fontSize: '11px',
  color: 'textOff',
  fontFamily: 'mono',
  mt: '2px'
});
const recentWhenStyle = css({ ml: 'auto', fontSize: '11px', color: 'textOff' });
const icon = css({
  w: '16px',
  h: '16px',
  stroke: 'currentColor',
  fill: 'none',
  strokeWidth: '1.6',
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
  flex: 'none'
});
const errorText = css({
  color: 'danger',
  fontSize: '12px',
  mt: '12px',
  mb: '0'
});
const emptyText = css({
  color: 'textDim',
  fontSize: '13px',
  mt: '16px',
  mb: '0'
});
const topRow = css({
  display: 'flex',
  alignItems: 'flex-start',
  justifyContent: 'space-between',
  gap: '12px'
});
const settingsBtn = css({
  w: '32px',
  h: '32px',
  display: 'grid',
  placeItems: 'center',
  borderRadius: '6px',
  border: '1px solid',
  borderColor: 'hairline',
  bg: 'transparent',
  color: 'textDim',
  cursor: 'pointer',
  flex: 'none',
  _hover: { bg: 'raised', color: 'textHi', borderColor: 'border' }
});
const newDocForm = css({
  display: 'grid',
  gap: '10px',
  mt: '14px',
  p: '14px',
  bg: 'raised',
  border: '1px solid',
  borderColor: 'hairline',
  borderRadius: '8px'
});
const fieldLabel = css({
  display: 'grid',
  gap: '4px',
  fontSize: '12px',
  color: 'textDim'
});
const formActions = css({ display: 'flex', gap: '8px', mt: '4px' });
const btn = button({ variant: 'solid', size: 'sm' });
const btnPrimary = button({ variant: 'primary', size: 'sm' });
const inputCls = input();
</script>

<div class={startShell}>
  <div class={startCard}>
    <div class={panel}>
      <div class={topRow}>
        <div>
          <p class={eyebrow}>Camcraft</p>
          <h2 class={welcome}>Bienvenido al estudio CAM</h2>
        </div>
        {#if onSettings}
          <button class={settingsBtn} type="button" onclick={onSettings} aria-label="Settings">
            <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1l2-1.5-1.8-3.1-2.3.8a7 7 0 0 0-1.7-1l-.3-2.4h-3.6l-.3 2.4a7 7 0 0 0-1.7 1l-2.3-.8L4.1 9.5 6.1 11a7 7 0 0 0 0 2l-2 1.5 1.8 3.1 2.3-.8a7 7 0 0 0 1.7 1l.3 2.4h3.6l.3-2.4a7 7 0 0 0 1.7-1l2.3.8 1.8-3.1-2-1.5z"/></svg>
          </button>
        {/if}
      </div>
      <p class={lead}>Abrí una placa o retomá un proyecto. El shell usa superficies flotantes, jerarquía de bordes clara y command search central.</p>
      <button class={action} type="button" onclick={() => (showNew = !showNew)}>
        <span class={`${actionIcon} ${actionIconPrimary}`}>
          <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
        </span>
        <div><strong class={actionTitle}>Nuevo proyecto PCB</strong><span class={actionDesc}>Configurar stock, origen y board</span></div>
      </button>
      {#if showNew}
        <form class={newDocForm} onsubmit={submitNew}>
          <label class={fieldLabel}>Width (mm)<input class={inputCls} type="number" step="any" bind:value={widthMm} /></label>
          <label class={fieldLabel}>Height (mm)<input class={inputCls} type="number" step="any" bind:value={heightMm} /></label>
          <label class={fieldLabel}>Thickness (mm)<input class={inputCls} type="number" step="any" bind:value={thicknessMm} /></label>
          <div class={formActions}>
            <button type="submit" class={`${btn} ${btnPrimary}`}>Create</button>
            <button type="button" class={btn} onclick={() => (showNew = false)}>Cancel</button>
          </div>
        </form>
      {/if}
      <button class={action} type="button" onclick={onOpen}>
        <span class={actionIcon}>
          <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h6l2 2h8v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z"/></svg>
        </span>
        <div><strong class={actionTitle}>Abrir proyecto</strong><span class={actionDesc}>.cam o fuentes de placa</span></div>
      </button>
      {#if error}
        <p class={errorText}>{error}</p>
      {/if}
    </div>
    <div class={panel}>
      <h2 class={heading}>Recientes</h2>
      <p class={lead}>Retomá con contexto visible antes de abrir.</p>
      {#if recents.length === 0}
        <p class={emptyText}>No recent documents yet.</p>
      {:else}
        {#each recents as recent (recent.path)}
          <button class={recentBtn} type="button" onclick={() => onOpenRecent?.(recent.path)}>
            <div><div class={`${recentName} recent-name`}>{recent.name}</div><div class={recentPath}>{recent.path}</div></div>
            <div class={recentWhenStyle}>{recentWhen(recent)}</div>
          </button>
        {/each}
      {/if}
    </div>
  </div>
</div>
