<script lang="ts">
import { css } from 'styled-system/css';

interface Props {
  projectName?: string;
  searchPlaceholder?: string;
  layoutLabel?: string;
  onOpen?: () => void;
  onSave?: () => void;
  onSaveAs?: () => void;
  onSettings?: () => void;
  onBack?: () => void;
}

let {
  projectName = 'controller-board.cam',
  searchPlaceholder = 'Buscar comando, capa u operación…',
  layoutLabel = 'Floating',
  onOpen,
  onSave,
  onSaveAs,
  onSettings,
  onBack
}: Props = $props();

const topbar = css({
  position: 'absolute',
  top: '0',
  left: '0',
  right: '0',
  h: '48px',
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  px: '12px',
  background: 'panel',
  backdropFilter: 'blur(8px)',
  borderBottom: '1px solid',
  borderColor: 'hairline',
  zIndex: '30'
});
const mark = css({
  w: '26px',
  h: '26px',
  borderRadius: '7px',
  display: 'grid',
  placeItems: 'center',
  background: 'raised',
  border: '1px solid',
  borderColor: 'border',
  color: 'sel',
  fontSize: '12px',
  fontWeight: '700',
  flex: 'none',
  cursor: 'pointer'
});
const product = css({ fontSize: '15px', fontWeight: '600', color: 'textHi', letterSpacing: '-0.01em' });
const sep = css({ w: '1px', h: '18px', bg: 'hairline', flex: 'none' });
const workspace = css({
  display: 'flex', alignItems: 'center', gap: '6px', px: '8px', py: '5px', borderRadius: '6px',
  color: 'text', fontSize: '12px', border: '1px solid transparent', cursor: 'pointer',
  _hover: { bg: 'raised', borderColor: 'hairline' }
});
const muted = css({ color: 'textOff' });
const search = css({
  mx: 'auto', w: '380px', maxW: '40vw', h: '30px', borderRadius: '6px', bg: 'app',
  border: '1px solid', borderColor: 'border', display: 'flex', alignItems: 'center', gap: '8px',
  px: '10px', color: 'textDim', fontSize: '12px'
});
const topActions = css({ display: 'flex', alignItems: 'center', gap: '4px' });
const actionBtn = css({
  h: '30px', px: '10px', borderRadius: '6px', fontSize: '12px', color: 'textDim',
  cursor: 'pointer', _hover: { bg: 'raised', color: 'textHi' }
});
const iconBtn = css({
  w: '30px', h: '30px', display: 'grid', placeItems: 'center', borderRadius: '6px',
  color: 'textDim', cursor: 'pointer', _hover: { bg: 'raised', color: 'textHi' }
});
const layoutPill = css({
  display: 'flex', alignItems: 'center', gap: '6px', px: '8px', py: '4px', borderRadius: '6px',
  border: '1px solid', borderColor: 'hairline', bg: 'raised', fontSize: '11px', color: 'textDim',
  '& b': { color: 'text', fontWeight: '500' }
});
const icon = css({
  w: '16px', h: '16px', stroke: 'currentColor', fill: 'none', strokeWidth: '1.6',
  strokeLinecap: 'round', strokeLinejoin: 'round', flex: 'none'
});
</script>

<header class={topbar}>
  <button class={mark} aria-label="Inicio" onclick={onBack}>C</button>
  <div class={product}>Camcraft</div>
  <div class={sep} aria-hidden="true"></div>
  <button class={workspace} aria-label="Proyecto actual" onclick={onOpen}>
    <span>{projectName}</span>
    <span class={muted}>▾</span>
  </button>
  <div class={search} role="search" aria-label="Buscar comando">
    <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6"/><path d="M20 20l-4.2-4.2"/></svg>
    <span>{searchPlaceholder}</span>
  </div>
  <div class={topActions}>
    <button class={actionBtn} onclick={onOpen}>Abrir</button>
    <button class={actionBtn} onclick={onSave}>Guardar</button>
    <button class={actionBtn} onclick={onSaveAs}>Guardar como</button>
    <div class={layoutPill} aria-label="Layout flotante"><b>{layoutLabel}</b> layout</div>
    <button class={iconBtn} aria-label="Configuración" onclick={onSettings}>
      <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1l2-1.5-1.8-3.1-2.3.8a7 7 0 0 0-1.7-1l-.3-2.4h-3.6l-.3 2.4a7 7 0 0 0-1.7 1l-2.3-.8L4.1 9.5 6.1 11a7 7 0 0 0 0 2l-2 1.5 1.8 3.1 2.3-.8a7 7 0 0 0 1.7 1l.3 2.4h3.6l.3-2.4a7 7 0 0 0 1.7-1l2.3.8 1.8-3.1-2-1.5c.1-.3.1-.7.1-1z"/></svg>
    </button>
  </div>
</header>
