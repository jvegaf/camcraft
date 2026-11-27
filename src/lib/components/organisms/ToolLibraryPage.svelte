<script lang="ts">
import { css, cx } from 'styled-system/css';
import { badge as badgeRecipe } from 'styled-system/recipes';
import type { Snippet } from 'svelte';
import { Button } from '../atoms';

// Organism (107 T4, S6 + S8) — the page shell of «Tool library» of
// `design/camcraft-workbench-v2/05-settings-tool-library.html` (`.lib`): one container with the
// artboard's three tracks, `260px minmax(0, 1fr) 220px`, and one headed column per pane — `Tools`
// with its `+ New` action, `Detail` with the `Valid` badge and `Preview`.
//
// The shell is presentational only: it takes no data, reads no store and calls no command. Its own
// contract is the two header affordances, declared as props so the page never claims what it cannot
// back: `oncreate` is the caller's create action (`+ New` calls it) and `valid` is the caller's
// verdict on the selected tool, so the `Valid` badge is a statement about a real selection instead of
// permanent decoration. The three bodies are empty placeholders that the later tasks fill through the
// named snippet props (`tools` = T5 tree, `detail` = T6 form, `preview` = T7 drawing), so filling them
// never reshapes the layout. Mounting the page in the settings navigation belongs to T3.
//
// Preview width: design comment `vjc4aj7t` reports the artboard's preview column being wider than
// the space it has («el problema le tienes en la columna entera de preview que es mas ancha que el
// espacio que tiene»). A CSS grid item's automatic minimum size is its content's minimum, so the
// fixed `220px` track was only a suggestion once something wider landed in it. The clamp is:
//   · the middle track is `minmax(0, 1fr)`, so it yields to its own content;
//   · every column is `minW: 0` and clips (`overflow: hidden`), and the container does the same;
//   · the preview body clips again and caps its own content (`& > *`, `& svg`) at `maxW: 100%`.
// The column can therefore never grow past `220px` of the container.
type Props = {
  /** Column 1 body — the tool tree of T5. */
  tools?: Snippet;
  /** Column 2 body — the detail form of T6. */
  detail?: Snippet;
  /** Column 3 body — the cutting-zone preview of T7. */
  preview?: Snippet;
  /**
   * The create action `+ New` runs. Without it the button offers nothing, so a caller that has no
   * create command passes none rather than leaving a button that does nothing.
   */
  oncreate?: () => void;
  /**
   * Whether the selected tool is valid right now. The badge says `Valid` only when this is true, so
   * an empty selection (or a detail form holding invalid data) never advertises validity.
   */
  valid?: boolean;
};

let { tools, detail, preview, oncreate, valid = false }: Props = $props();

const uid = $props.id();

const library = css({
  flex: '1',
  minH: '0',
  minW: '0',
  w: '100%',
  maxW: '100%',
  display: 'grid',
  gridTemplateColumns: '260px minmax(0, 1fr) 220px',
  overflow: 'hidden',
  bg: 'app',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: 'panel'
});
const column = css({
  minW: '0',
  minH: '0',
  display: 'flex',
  flexDirection: 'column',
  overflow: 'hidden'
});
// The artboard separates the columns with a hairline on every column but the first (`.col + .col`).
const dividedColumn = css({
  borderLeft: '1px solid',
  borderColor: 'hairline'
});
const columnHead = css({
  flex: 'none',
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  px: '12px',
  py: '8px',
  bg: 'panel/45',
  borderBottom: '1px solid',
  borderColor: 'hairline'
});
// `caption` already carries the artboard's uppercase caption shape (11px, 0.06em, uppercase).
const columnTitle = css({
  m: '0',
  flex: 'none',
  textStyle: 'caption',
  color: 'textDim'
});
const spacer = css({ flex: '1' });
const validBadge = css({
  flex: 'none',
  color: 'ok',
  borderColor: 'ok/25',
  bg: 'ok/8'
});
const columnBody = css({
  flex: '1',
  minW: '0',
  minH: '0',
  overflow: 'auto'
});
// Each pane of the artboard pads its body differently; the padding is the only per-column body rule.
const listBody = css({ p: '8px' });
const detailBody = css({ p: '16px' });
// The clamp of the design comment: clip, and cap whatever the preview task injects.
const previewBody = css({
  p: '16px',
  overflow: 'hidden',
  '& > *': { minW: '0', maxW: '100%' },
  '& svg, & img, & canvas': { display: 'block', maxW: '100%', h: 'auto' }
});
const headingId = (pane: string) => `${uid}-${pane}`;
</script>

<section class={library} aria-label="Tool library">
  <section class={column} aria-labelledby={headingId('tools')}>
    <div class={columnHead}>
      <h4 class={columnTitle} id={headingId('tools')}>Tools</h4>
      <span class={spacer}></span>
      <Button size="sm" variant="secondary" onclick={oncreate}>+ New</Button>
    </div>
    <div class={cx(columnBody, listBody)} data-column-body="tools">
      {#if tools}{@render tools()}{/if}
    </div>
  </section>

  <section class={cx(column, dividedColumn)} aria-labelledby={headingId('detail')}>
    <div class={columnHead}>
      <h4 class={columnTitle} id={headingId('detail')}>Detail</h4>
      <span class={spacer}></span>
      {#if valid}
        <span class={cx(badgeRecipe(), validBadge)}>Valid</span>
      {/if}
    </div>
    <div class={cx(columnBody, detailBody)} data-column-body="detail">
      {#if detail}{@render detail()}{/if}
    </div>
  </section>

  <section
    class={cx(column, dividedColumn)}
    aria-labelledby={headingId('preview')}
  >
    <div class={columnHead}>
      <h4 class={columnTitle} id={headingId('preview')}>Preview</h4>
    </div>
    <div class={cx(columnBody, previewBody)} data-column-body="preview">
      {#if preview}{@render preview()}{/if}
    </div>
  </section>
</section>
