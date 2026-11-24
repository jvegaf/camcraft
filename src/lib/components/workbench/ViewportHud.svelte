<script lang="ts">
import { css } from 'styled-system/css';

interface Props {
  mode?: string;
  selection?: string;
}

let {
  mode = '2D Board · Grid 1 mm',
  selection = '6 taladros seleccionados'
}: Props = $props();

const hud = css({
  position: 'absolute',
  right: '356px',
  top: '64px',
  display: 'flex',
  gap: '8px',
  zIndex: '20'
});
const card = css({
  px: '10px',
  py: '7px',
  borderRadius: '6px',
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  fontSize: '11px',
  color: 'textDim',
  boxShadow: '0 4px 16px rgba(0,0,0,.18)',
  '& b': { color: 'text', fontWeight: '500' }
});
const selText = css({ color: 'sel' });
</script>

<div class={hud} aria-label="Estado de viewport">
  <div class={card}><b>{mode.split(' · ')[0]}</b>{#if mode.includes(' · ')} · {mode.split(' · ').slice(1).join(' · ')}{/if}</div>
  <div class={card}><span class={selText}>{selection}</span></div>
</div>
