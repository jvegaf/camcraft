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
const toolBtn = css({
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
</script>

<div class={rail} aria-label="Acciones rápidas">
  <button class={toolBtn} onclick={onOpen}>Abrir</button>
  <button class={toolBtn} onclick={onSave}>Guardar</button>
</div>
