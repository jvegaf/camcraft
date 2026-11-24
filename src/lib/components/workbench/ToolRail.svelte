<script lang="ts">
import { css } from 'styled-system/css';

interface Tool {
  label: string;
  primary?: boolean;
}

interface Props {
  tools?: Tool[];
}

let {
  tools = [
    { label: 'Importar' },
    { label: 'Capas' },
    { label: 'Origen' },
    { label: 'Generar plan', primary: true }
  ]
}: Props = $props();

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
const primaryBtn = css({
  color: 'cut',
  borderColor: 'cut',
  bg: 'cutSoft'
});
</script>

<div class={rail} aria-label="Acciones rápidas">
  {#each tools as tool}
    <button class={tool.primary ? `${toolBtn} ${primaryBtn}` : toolBtn}>{tool.label}</button>
  {/each}
</div>
