<script lang="ts">
import { css, cx } from 'styled-system/css';
import { panel } from 'styled-system/recipes';
import type { Snippet } from 'svelte';

// Molecule (095 T1, moved here in T5): the floating card surface of
// `design/camcraft-workbench-v2/02-floating-studio.html` `.float` — header 38px with the caption
// title, optional meta and quiet actions, then a scrolling body slot. It composes no organism logic:
// the Browser and Inspector organisms decide its content, placement and handlers, and the panel
// recipe supplies the `Overlay` surface with `radius-overlay` (S6).
interface Props {
  title: string;
  meta?: string;
  class?: string;
  ariaLabel?: string;
  headAction?: Snippet;
  children: Snippet;
}

let {
  title,
  meta,
  class: className = '',
  ariaLabel = title,
  headAction,
  children
}: Props = $props();

const surface = panel({ tone: 'overlay' });
const head = css({
  h: '38px',
  px: '10px',
  pl: '12px',
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  borderBottom: '1px solid',
  borderColor: 'hairline',
  background: 'raised',
  flex: 'none'
});
const heading = css({
  m: '0',
  fontSize: '11px',
  fontWeight: '600',
  letterSpacing: '0.06em',
  textTransform: 'uppercase',
  color: 'textDim'
});
const metaClass = css({
  ml: 'auto',
  fontSize: '11px',
  fontFamily: 'mono',
  color: 'textOff'
});
const body = css({ p: '10px', overflow: 'auto', minH: '0' });
</script>

<aside
  class={cx(surface, className)}
  style="position:absolute; backdrop-filter:blur(10px); z-index:20; display:flex; flex-direction:column; min-height:0; overflow:hidden;"
  aria-label={ariaLabel}
>
  <div class={head}>
    <h2 class={heading}>{title}</h2>
    {#if meta}<span class={metaClass}>{meta}</span>{/if}
    {#if headAction}{@render headAction()}{/if}
  </div>
  <div class={body}>
    {@render children()}
  </div>
</aside>
