<script lang="ts">
import { css } from 'styled-system/css';
import { workbenchState } from '../workbench/workbench.svelte';

// Organism (095 T3, S4): the viewport HUD of
// `design/camcraft-workbench-v2/02-floating-studio.html` `.hud` — the viewport label card plus the
// selection card. "2D Board" and " · Grid 1 mm" are chrome strings of the artboard; the count is
// sample data there, so it is rendered from the real selection and the card disappears when nothing
// is selected instead of showing a made-up number.
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

// The viewport selects one triangle at a time (`workbench.svelte.ts`), so a selection is always a
// count of one piece of geometry.
let selectedCount = $derived(workbenchState.selection ? 1 : 0);
let selectionLabel = $derived(
  selectedCount === 1
    ? `${selectedCount} triángulo seleccionado`
    : `${selectedCount} triángulos seleccionados`
);
</script>

<div class={hud} aria-label="Estado de viewport">
  <div class={card}><b>2D Board</b> · Grid 1 mm</div>
  {#if selectedCount > 0}
    <div class={card}>
      <span class={selText}>{selectionLabel}</span>
    </div>
  {/if}
</div>
