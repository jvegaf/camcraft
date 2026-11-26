<script lang="ts">
import { css } from 'styled-system/css';
import { workbenchState } from '../workbench/workbench.svelte';

// Organism (095 T3, S4): the viewport HUD of
// `design/camcraft-workbench-v2/02-floating-studio.html` `.hud` — the viewport label card plus the
// selection card. "2D Board" and " · Grid 1 mm" are chrome strings of the artboard; the count is
// sample data there, so it is rendered from the real selection. With nothing selected the card
// stays and states `Sin selección` (`design/system.md` «Accesibilidad y comportamiento»: estados
// vacíos explícitos) instead of disappearing and leaving the operator to guess.
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
  borderRadius: 'panel',
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  textStyle: 'small',
  color: 'textDim',
  boxShadow: 'float',
  '& b': { color: 'text', fontWeight: '500' }
});
const selText = css({ color: 'sel' });

// The viewport selects one triangle at a time (`workbench.svelte.ts`: `ViewportSelection` is a
// single triangle or nothing), so the card states that single selection instead of counting a set
// the app cannot produce today. The plural branch this used to carry was unreachable and claimed a
// multi-select that does not exist.
let selectionLabel = $derived(
  workbenchState.selection ? '1 triángulo seleccionado' : 'Sin selección'
);
</script>

<div class={hud} aria-label="Estado de viewport">
  <div class={card}><b>2D Board</b> · Grid 1 mm</div>
  <div class={card}>
    {#if workbenchState.selection}
      <span class={selText}>{selectionLabel}</span>
    {:else}
      <span>{selectionLabel}</span>
    {/if}
  </div>
</div>
