<script lang="ts">
import { css } from 'styled-system/css';
import { workbenchState } from './workbench.svelte';

const hud = css({
  position: 'absolute', right: '356px', top: '64px',
  display: 'flex', gap: '8px', zIndex: '20'
});
const card = css({
  px: '10px', py: '7px', borderRadius: '6px', bg: 'panel',
  border: '1px solid', borderColor: 'border', fontSize: '11px', color: 'textDim',
  boxShadow: '0 4px 16px rgba(0,0,0,.18)',
  '& b': { color: 'text', fontWeight: '500' }
});
const selText = css({ color: 'sel' });
</script>

<div class={hud} aria-label="Estado de viewport">
  <div class={card}><b>3D Viewport</b> · Orbit · Grid 1 mm</div>
  <div class={card}>
    <span class={selText}>
      {#if workbenchState.selection}
        Tri {workbenchState.selection.index} seleccionado
      {:else}
        Sin selección
      {/if}
    </span>
  </div>
</div>
