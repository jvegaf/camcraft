<script lang="ts">
import { css } from 'styled-system/css';
import { workbenchState } from './workbench.svelte';

const statusBar = css({
  position: 'absolute',
  left: '0',
  right: '0',
  bottom: '0',
  h: '34px',
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
  px: '14px',
  bg: 'panel',
  backdropFilter: 'blur(8px)',
  borderTop: '1px solid',
  borderColor: 'hairline',
  fontSize: '11px',
  color: 'textDim',
  zIndex: '30'
});
const mono = css({ fontSize: '11px', fontFamily: 'mono', color: 'text' });
const ok = css({ color: 'ok' });
const err = css({ color: 'danger' });
const info = css({ color: 'textDim' });
const spacer = css({ flex: '1' });
const sep = css({ w: '1px', h: '14px', bg: 'hairline' });
</script>

<footer class={statusBar} aria-label="Barra de estado">
  <span class={mono}>3D Viewport</span>
  <span class={sep} aria-hidden="true"></span>
  <span>mm · Orbit · Grid 1 mm</span>
  {#if workbenchState.selection}
    <span class={sep} aria-hidden="true"></span>
    <span class={mono}>Tri {workbenchState.selection.index}</span>
    <span class={mono}>{workbenchState.selection.avgDepthMm.toFixed(2)} mm</span>
  {/if}
  <span class={spacer}></span>
  <span class={workbenchState.statusKind === 'error' ? err : workbenchState.statusKind === 'ok' ? ok : info}>
    {workbenchState.statusKind === 'ok' ? '● ' : workbenchState.statusKind === 'error' ? '● ' : ''}{workbenchState.statusMessage}
  </span>
</footer>
