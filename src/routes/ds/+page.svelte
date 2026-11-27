<script lang="ts">
import { css, cx } from 'styled-system/css';
import { buildBoard, CATALOG, CATALOG_PATHS } from '$lib/ds/catalog';
import type { ComponentEntry, Layer } from '$lib/ds/inventory';
import { inventory } from '$lib/ds/inventory.generated';

// Catalog index and progress board (097 T3, S1 + S2). Every number on this page is derived from the
// generated inventory and the curated manifest: nothing is declared by hand. Unclassified
// components are listed explicitly instead of disappearing from the count.

const board = buildBoard(inventory, CATALOG);

const SECTIONS: { layer: Layer; title: string }[] = [
  { layer: 'atom', title: 'Atoms' },
  { layer: 'molecule', title: 'Molecules' },
  { layer: 'organism', title: 'Organisms' },
  { layer: 'template', title: 'Templates' }
];

function componentsOf(layer: Layer): ComponentEntry[] {
  return inventory
    .filter(component => component.layer === layer)
    .sort((left, right) => left.name.localeCompare(right.name));
}

function hasSheet(component: ComponentEntry): boolean {
  return CATALOG_PATHS.has(component.path);
}

function slugOf(component: ComponentEntry): string | undefined {
  return CATALOG.find(entry => entry.path === component.path)?.slug;
}

const page = css({
  minH: '100vh',
  bg: 'canvas',
  color: 'text',
  fontFamily: 'sans',
  fontSize: '13px',
  lineHeight: '1.4',
  p: '24px'
});
const inner = css({ maxW: '1100px', mx: 'auto' });
const heading = css({
  textStyle: 'hero',
  color: 'textHi',
  m: '0',
  mb: '4px'
});
const lead = css({ color: 'textDim', m: '0', mb: '20px' });
const card = css({
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: 'panel',
  boxShadow: 'float',
  p: '14px',
  mb: '20px'
});
const cardTitle = css({
  textStyle: 'caption',
  color: 'textOff',
  m: '0',
  mb: '10px'
});
const boardTable = css({ w: '100%', borderCollapse: 'collapse' });
const boardCell = css({
  px: '6px',
  py: '5px',
  borderTop: '1px solid',
  borderColor: 'hairline',
  textAlign: 'right',
  textStyle: 'mono',
  color: 'text'
});
const boardHead = css({
  px: '6px',
  py: '4px',
  textStyle: 'caption',
  color: 'textOff',
  textAlign: 'right'
});
const boardLayer = css({
  px: '6px',
  py: '5px',
  borderTop: '1px solid',
  borderColor: 'hairline',
  textStyle: 'small',
  color: 'textDim',
  textAlign: 'left'
});
const section = css({ mb: '24px' });
const sectionTitle = css({
  textStyle: 'title',
  color: 'textHi',
  m: '0',
  mb: '8px'
});
const row = css({
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  px: '10px',
  py: '7px',
  borderTop: '1px solid',
  borderColor: 'hairline',
  _first: { borderTop: 'none' }
});
const name = css({ flex: '1', minW: '0', color: 'textHi', textStyle: 'small' });
const layerTag = css({ flex: 'none', color: 'textOff', textStyle: 'mono' });
const sheetLink = css({
  flex: 'none',
  color: 'sel',
  textStyle: 'small',
  textDecoration: 'none',
  _hover: { textDecoration: 'underline' }
});
const noSheet = css({ flex: 'none', color: 'textOff', textStyle: 'small' });
const badge = css({
  flex: 'none',
  px: '7px',
  py: '1px',
  borderRadius: '999px',
  border: '1px solid',
  borderColor: 'hairline',
  bg: 'raised',
  textStyle: 'caption'
});
const badgeYes = css({ color: 'ok', borderColor: 'ok/35' });
const badgeNo = css({ color: 'textOff' });
const unclassifiedRow = css({
  px: '10px',
  py: '5px',
  textStyle: 'mono',
  color: 'textDim',
  borderTop: '1px solid',
  borderColor: 'hairline'
});
const empty = css({ color: 'textOff', textStyle: 'small', m: '0' });

const sectionTotal = (layer: Layer) =>
  board.layers.find(summary => summary.layer === layer)?.total ?? 0;
</script>

<div class={page}>
  <div class={inner}>
    <h1 class={heading}>Design system</h1>
    <p class={lead}>
      Atomic Design catalog derived from the repository. Only exists under
      <code> pnpm dev</code>.
    </p>

    <section class={card} aria-label="Progress board">
      <h2 class={cardTitle}>Progress board</h2>
      <table class={boardTable}>
        <thead>
          <tr>
            <th class={boardLayer}>Layer</th>
            <th class={boardHead}>Total</th>
            <th class={boardHead}>Sheet</th>
            <th class={boardHead}>Test</th>
            <th class={boardHead}>Components in use</th>
            <th class={boardHead}>Renders in isolation</th>
            <th class={boardHead}>Does not render in isolation</th>
          </tr>
        </thead>
        <tbody>
          {#each board.layers as summary (summary.layer)}
            <tr data-board-layer={summary.layer}>
              <th class={boardLayer}>{summary.layer}</th>
              <td class={boardCell} data-board-total>{summary.total}</td>
              <td class={boardCell} data-board-sheet>{summary.withSheet}</td>
              <td class={boardCell} data-board-test>{summary.withTest}</td>
              <td class={boardCell} data-board-inuse>{summary.inUse}</td>
              <td class={boardCell} data-board-renderable
                >{summary.renderable}</td
              >
              <td class={boardCell} data-board-not-renderable
                >{summary.notRenderable}</td
              >
            </tr>
          {/each}
        </tbody>
      </table>
    </section>

    <section class={card} aria-label="Unclassified components">
      <h2 class={cardTitle}>Unclassified ({board.unclassified.length})</h2>
      {#if board.unclassified.length === 0}
        <p class={empty}>Every component lives in an atomic layer.</p>
      {:else}
        {#each board.unclassified as component (component.path)}
          <div class={unclassifiedRow} data-unclassified-item={component.path}>
            {component.path}
          </div>
        {/each}
      {/if}
    </section>

    {#each SECTIONS as entry (entry.layer)}
      <section class={section} data-layer-section={entry.layer}>
        <h2 class={sectionTitle}
          >{entry.title} <span class={layerTag}
            >{sectionTotal(entry.layer)}</span
          ></h2
        >
        {#each componentsOf(entry.layer) as component (component.path)}
          <div class={row} data-component-path={component.path}>
            <span class={name}>{component.name}</span>
            <span class={layerTag} data-layer={component.layer}
              >{component.layer}</span
            >
            <span
              class={cx(badge, hasSheet(component) ? badgeYes : badgeNo)}
              data-badge-sheet={hasSheet(component) ? 'yes' : 'no'}
              >sheet {hasSheet(component) ? 'yes' : 'no'}</span
            >
            <span
              class={cx(badge, component.test ? badgeYes : badgeNo)}
              data-badge-test={component.test ? 'yes' : 'no'}
              >test {component.test ? 'yes' : 'no'}</span
            >
            <span
              class={cx(
                badge,
                component.consumers.length > 0 ? badgeYes : badgeNo
              )}
              data-badge-inuse={component.consumers.length > 0 ? 'yes' : 'no'}
              >in use {component.consumers.length > 0 ? 'yes' : 'no'}</span
            >
            {#if hasSheet(component)}
              <a class={sheetLink} href={`/ds/${slugOf(component)}`}>Sheet →</a>
            {:else}
              <span class={noSheet}>No sheet</span>
            {/if}
          </div>
        {/each}
      </section>
    {/each}
  </div>
</div>
