<script lang="ts">
import { css } from 'styled-system/css';
import { workbenchState } from '../workbench/workbench.svelte';

// Organism (095 T3, S4): the status bar of
// `design/camcraft-workbench-v2/02-floating-studio.html` `.status` — X/Y/Z in mono, the units block
// and the health indicator. The artboard coordinates are sample data; the real ones come from the
// point `BoardCanvas` publishes on every pointer move, and when the pointer is not over the canvas
// the cells show an em dash instead of a number nobody measured.
//
// The bar also carries the viewport selection readout the 3D workbench had before the chrome
// rewrite (triangle index + average depth), so the information is not lost next to the artboard
// labels.
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
  textStyle: 'small',
  color: 'textDim',
  zIndex: '30'
});
const mono = css({ textStyle: 'mono', color: 'text' });
const ok = css({ color: 'ok' });
const err = css({ color: 'danger' });
const info = css({ color: 'textDim' });
const spacer = css({ flex: '1' });
const sep = css({ w: '1px', h: '14px', bg: 'hairline' });

// `design/system.md` «Mono» is 12/400; the artboard prints two decimals, so the cells are formatted
// once here and the em dash keeps the cell width when there is no point.
function coordinate(value: number | undefined): string {
  return value === undefined ? '—' : value.toFixed(2);
}
</script>

<footer class={statusBar} aria-label="Status bar">
  <span class={mono}>X {coordinate(workbenchState.pointer?.x)}</span>
  <span class={mono}>Y {coordinate(workbenchState.pointer?.y)}</span>
  <span class={mono}>Z {coordinate(workbenchState.pointer?.z)}</span>
  <span class={sep} aria-hidden="true"></span>
  <span>mm · Grid 1 mm · GRBL</span>
  {#if workbenchState.selection}
    <span class={sep} aria-hidden="true"></span>
    <span class={mono}>Tri {workbenchState.selection.index}</span>
    <span class={mono}>{workbenchState.selection.avgDepthMm.toFixed(2)} mm</span>
  {/if}
  <span class={spacer}></span>
  <span
    class={workbenchState.statusKind === 'error'
      ? err
      : workbenchState.statusKind === 'ok'
        ? ok
        : info}>{workbenchState.statusMessage}</span
  >
  {#if workbenchState.statusKind !== 'error'}
    <span class={ok}>● No errors</span>
  {/if}
</footer>
