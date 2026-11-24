<script lang="ts">
import { css } from 'styled-system/css';
import { workbenchState } from './workbench.svelte';

interface Props {
  message?: string;
}

let { message = 'mm · Grid 1 mm · Tauri' }: Props = $props();

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
  <span class={mono}>3D</span>
  <span class={sep} aria-hidden="true"></span>
  <span>{message}</span>
  <span class={spacer}></span>
  {#if workbenchState.selection}
    <span class={mono}>Tri {workbenchState.selection.index}</span>
    <span class={mono}>{workbenchState.selection.avgDepthMm.toFixed(2)} mm</span>
    <span class={sep} aria-hidden="true"></span>
  {/if}
  <span class={workbenchState.statusKind === 'error' ? err : workbenchState.statusKind === 'ok' ? ok : info}>
    {workbenchState.statusMessage}
  </span>
</footer>
