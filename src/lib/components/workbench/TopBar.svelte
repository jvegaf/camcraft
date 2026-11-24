<script lang="ts">
import { css, cx } from 'styled-system/css';

interface Props {
  projectName?: string;
  searchPlaceholder?: string;
  layoutLabel?: string;
}

let {
  projectName = 'controller-board.cam',
  searchPlaceholder = 'Buscar comando, capa u operación…',
  layoutLabel = 'Floating'
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
  flex: 'none'
});
const product = css({
  fontSize: '15px',
  fontWeight: '600',
  color: 'textHi',
  letterSpacing: '-0.01em'
});
const sep = css({ w: '1px', h: '18px', bg: 'hairline', flex: 'none' });
const workspace = css({
  display: 'flex',
  alignItems: 'center',
  gap: '6px',
  px: '8px',
  py: '5px',
  borderRadius: '6px',
  color: 'text',
  fontSize: '12px',
  border: '1px solid transparent',
  cursor: 'pointer',
  _hover: { bg: 'raised', borderColor: 'hairline' },
  _focusVisible: {
    outline: '2px solid',
    outlineColor: 'sel',
    outlineOffset: '-1px'
  }
});
const muted = css({ color: 'textOff' });
const search = css({
  mx: 'auto',
  w: '380px',
  maxW: '40vw',
  h: '30px',
  borderRadius: '6px',
  bg: 'app',
  border: '1px solid',
  borderColor: 'border',
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  px: '10px',
  color: 'textDim',
  fontSize: '12px',
  _focusWithin: {
    borderColor: 'sel',
    boxShadow: '0 0 0 2px token(colors.selSoft)'
  },
  '& kbd': {
    ml: 'auto',
    fontSize: '11px',
    fontFamily: 'mono',
    color: 'textOff',
    px: '5px',
    py: '1px',
    borderRadius: '3px',
    bg: 'raised',
    border: '1px solid',
    borderColor: 'hairline'
  }
});
const topActions = css({ display: 'flex', alignItems: 'center', gap: '4px' });
const iconBtn = css({
  w: '30px',
  h: '30px',
  display: 'grid',
  placeItems: 'center',
  borderRadius: '6px',
  color: 'textDim',
  cursor: 'pointer',
  _hover: { bg: 'raised', color: 'textHi' },
  _focusVisible: {
    outline: '2px solid',
    outlineColor: 'sel',
    outlineOffset: '-1px'
  }
});
const layoutPill = css({
  display: 'flex',
  alignItems: 'center',
  gap: '6px',
  px: '8px',
  py: '4px',
  borderRadius: '6px',
  border: '1px solid',
  borderColor: 'hairline',
  bg: 'raised',
  fontSize: '11px',
  color: 'textDim',
  '& b': { color: 'text', fontWeight: '500' }
});
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
</script>

<header class={topbar}>
  <div class={mark} aria-hidden="true">C</div>
  <div class={product}>Camcraft</div>
  <div class={sep} aria-hidden="true"></div>
  <button class={workspace} aria-label="Proyecto actual">
    <span>{projectName}</span>
    <span class={muted}>▾</span>
  </button>
  <div class={search} role="search" aria-label="Buscar comando">
    <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6"/><path d="M20 20l-4.2-4.2"/></svg>
    <span>{searchPlaceholder}</span>
    <kbd>⌘K</kbd>
  </div>
  <div class={topActions}>
    <div class={layoutPill} aria-label="Layout flotante"><b>{layoutLabel}</b> layout</div>
    <button class={iconBtn} aria-label="Tema">
      <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/></svg>
    </button>
    <button class={iconBtn} aria-label="Configuración">
      <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1l2-1.5-1.8-3.1-2.3.8a7 7 0 0 0-1.7-1l-.3-2.4h-3.6l-.3 2.4a7 7 0 0 0-1.7 1l-2.3-.8L4.1 9.5 6.1 11a7 7 0 0 0 0 2l-2 1.5 1.8 3.1 2.3-.8a7 7 0 0 0 1.7 1l.3 2.4h3.6l.3-2.4a7 7 0 0 0 1.7-1l2.3.8 1.8-3.1-2-1.5c.1-.3.1-.7.1-1z"/></svg>
    </button>
  </div>
</header>
