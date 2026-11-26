<script lang="ts">
import { css } from 'styled-system/css';

interface Props {
  onOpen?: () => void;
  onSave?: () => void;
}

let { onOpen, onSave }: Props = $props();

const rail = css({
  position: 'absolute',
  top: '64px',
  left: '316px',
  display: 'flex',
  gap: '6px',
  zIndex: '20'
});
const tool = css({
  h: '32px',
  px: '10px',
  borderRadius: '6px',
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  fontSize: '12px',
  color: 'text',
  display: 'inline-flex',
  alignItems: 'center',
  gap: '6px',
  boxShadow: '0 4px 16px rgba(0,0,0,.18)',
  cursor: 'pointer',
  _hover: { bg: 'raised', color: 'textHi' },
  _focusVisible: {
    outline: '2px solid',
    outlineColor: 'sel',
    outlineOffset: '-1px'
  }
});
const primary = css({ color: 'cut', borderColor: 'cut', bg: 'cutSoft' });
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

<div class={rail} aria-label="Acciones rápidas">
  <button class={tool} onclick={onOpen}>
    <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h6l2 2h8v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z"/></svg>
    Abrir
  </button>
  <button class={`${tool} ${primary}`} onclick={onSave}>
    <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><path d="M17 21v-8H7v8M7 3v5h8"/></svg>
    Guardar
  </button>
</div>
