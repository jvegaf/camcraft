<script lang="ts">
import { css, cx } from 'styled-system/css';
import { panelSection as panelSectionRecipe } from 'styled-system/recipes';
import type { Snippet } from 'svelte';

// Molecule: the browser's Board / Setup / Operaciones / Salida block. Header with the section name,
// an optional count and an optional right slot; body is a slot. No Tauri, no API, no store.
//
// 095 T4 adds the disclosure variant: the Browser keeps which sections are collapsed in the
// workbench store, and this molecule only renders the header toggle it is given (`aria-expanded`
// plus the expand/contract name). Collapsed hides the body entirely, so a collapsed section has no
// phantom padding and no rows to announce.
interface Props {
  title: string;
  count?: string | number;
  right?: Snippet;
  /** Renders the header disclosure control. Needs `ontoggle`. */
  collapsible?: boolean;
  collapsed?: boolean;
  ontoggle?: () => void;
  class?: string;
  children: Snippet;
}

let {
  title,
  count,
  right,
  collapsible = false,
  collapsed = false,
  ontoggle,
  class: className = '',
  children
}: Props = $props();

const styles = panelSectionRecipe();
const toggle = css({
  flex: 'none',
  w: '18px',
  h: '18px',
  display: 'grid',
  placeItems: 'center',
  border: 'none',
  borderRadius: 'control',
  background: 'transparent',
  color: 'textOff',
  fontSize: '10px',
  lineHeight: '1',
  cursor: 'pointer',
  _hover: { background: 'raised', color: 'text' },
  _focusVisible: {
    outline: '2px solid',
    outlineColor: 'sel',
    outlineOffset: '-1px'
  }
});
</script>

<section class={cx(styles.root, className)}>
  <div class={styles.head}>
    <h3 class={styles.title}>{title}</h3>
    <span class={styles.rule} aria-hidden="true"></span>
    {#if count !== undefined}
      <span class={styles.count}>{count}</span>
    {/if}
    {#if right}{@render right()}{/if}
    {#if collapsible}
      <button
        type="button"
        class={toggle}
        aria-expanded={!collapsed}
        aria-label={`${collapsed ? 'Expand' : 'Collapse'} ${title}`}
        onclick={ontoggle}
      >
        <span aria-hidden="true">{collapsed ? '▸' : '▾'}</span>
      </button>
    {/if}
  </div>
  {#if !collapsed}
    <div class={styles.body}>
      {@render children()}
    </div>
  {/if}
</section>
