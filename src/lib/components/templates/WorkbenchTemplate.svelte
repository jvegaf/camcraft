<script lang="ts">
import { workbenchTemplate } from 'styled-system/recipes';
import type { Snippet } from 'svelte';

// Template (095 T2, S1 + S4) — the atomic tier that owns layout only.
// Reference: `design/camcraft-workbench-v2/02-floating-studio.html` (`.shell` frame + `.canvas-bg`
// wash). No Tauri calls, no document state, no data logic: the organisms that land in each region
// keep their own behaviour, and pages keep the wiring.
interface Props {
  /** Accessible name of the frame landmark. */
  ariaLabel?: string;
  topBar?: Snippet;
  toolRail?: Snippet;
  viewport?: Snippet;
  viewportHud?: Snippet;
  browser?: Snippet;
  inspector?: Snippet;
  statusBar?: Snippet;
}

let {
  ariaLabel = 'Camcraft Floating Studio',
  topBar,
  toolRail,
  viewport,
  viewportHud,
  browser,
  inspector,
  statusBar
}: Props = $props();

const styles = workbenchTemplate();
</script>

<div
  class={styles.root}
  data-region="frame"
  role="application"
  aria-label={ariaLabel}
>
  <div class={styles.backdrop} data-region="backdrop" aria-hidden="true"></div>
  <div class={styles.viewport} data-slot="viewport">{@render viewport?.()}</div>
  <div class={styles.topBar} data-slot="topBar">{@render topBar?.()}</div>
  <div class={styles.toolRail} data-slot="toolRail">{@render toolRail?.()}</div>
  <div class={styles.viewportHud} data-slot="viewportHud">
    {@render viewportHud?.()}
  </div>
  <div class={styles.browser} data-slot="browser">{@render browser?.()}</div>
  <div class={styles.inspector} data-slot="inspector">
    {@render inspector?.()}
  </div>
  <div class={styles.statusBar} data-slot="statusBar">
    {@render statusBar?.()}
  </div>
</div>
