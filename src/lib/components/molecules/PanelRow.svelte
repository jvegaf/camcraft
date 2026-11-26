<script lang="ts">
import { cx } from 'styled-system/css';
import { panelRow as panelRowRecipe } from 'styled-system/recipes';
import Icon from '../atoms/Icon.svelte';
import type { IconName } from '../atoms/icons';
import StatusDot from '../atoms/StatusDot.svelte';
import type { StatusDotTone } from '../atoms/tone';

// Molecule: a panel row — leading dot or icon, label, and trailing meta such as ".gbr" or
// "seleccionada". It is a real button only when it can be activated; a selected row is marked with
// `aria-current`, not with color alone.
interface Props {
  label: string;
  meta?: string;
  dot?: StatusDotTone;
  icon?: IconName;
  selected?: boolean;
  onclick?: (event: MouseEvent) => void;
  class?: string;
}

let {
  label,
  meta,
  dot,
  icon,
  selected = false,
  onclick,
  class: className = ''
}: Props = $props();

let styles = $derived(
  panelRowRecipe({ interactive: Boolean(onclick), selected })
);
</script>

{#snippet content()}
  {#if dot}
    <StatusDot tone={dot} />
  {:else if icon}
    <Icon name={icon} />
  {/if}
  <span class={styles.label}>{label}</span>
  {#if meta}
    <span class={styles.meta}>{meta}</span>
  {/if}
{/snippet}

{#if onclick}
  <button
    type="button"
    class={cx(styles.root, className)}
    aria-current={selected ? 'true' : undefined}
    {onclick}
  >
    {@render content()}
  </button>
{:else}
  <div
    class={cx(styles.root, className)}
    aria-current={selected ? 'true' : undefined}
  >
    {@render content()}
  </div>
{/if}
