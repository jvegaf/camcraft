<script lang="ts">
import { cx } from 'styled-system/css';
import { panelSection as panelSectionRecipe } from 'styled-system/recipes';
import type { Snippet } from 'svelte';

// Molecule: the browser's Board / Setup / Operaciones / Salida block. Header with the section name,
// an optional count and an optional right slot; body is a slot. No Tauri, no API, no store.
interface Props {
  title: string;
  count?: string | number;
  right?: Snippet;
  class?: string;
  children: Snippet;
}

let { title, count, right, class: className = '', children }: Props = $props();

const styles = panelSectionRecipe();
</script>

<section class={cx(styles.root, className)}>
  <div class={styles.head}>
    <h3 class={styles.title}>{title}</h3>
    <span class={styles.rule} aria-hidden="true"></span>
    {#if count !== undefined}
      <span class={styles.count}>{count}</span>
    {/if}
    {#if right}{@render right()}{/if}
  </div>
  <div class={styles.body}>
    {@render children()}
  </div>
</section>
