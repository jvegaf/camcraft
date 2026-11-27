<script lang="ts">
import { cx } from 'styled-system/css';
import { panelRow as panelRowRecipe } from 'styled-system/recipes';
import Icon from '../atoms/Icon.svelte';
import type { IconName } from '../atoms/icons';
import StatusDot from '../atoms/StatusDot.svelte';
import type { StatusDotTone } from '../atoms/tone';

// Molecule: a panel row — leading dot or icon, label, and trailing meta such as ".gbr" or
// "seleccionada". It is a real button only when it can be activated; a selected row is marked with
// `aria-current`, not with color alone. A dot only carries meaning when the caller says what it
// means (`dotLabel`), so a tonal row is announced instead of relying on color (design/system.md
// «Browser» + «Accesibilidad y comportamiento»).
interface Props {
  label: string;
  meta?: string;
  dot?: StatusDotTone;
  /**
   * Accessible name of the leading dot, e.g. `Estado: válido`. Without it the dot stays decorative
   * (`aria-hidden`) and only the row's own text is announced.
   */
  dotLabel?: string;
  icon?: IconName;
  selected?: boolean;
  onclick?: (event: MouseEvent) => void;
  class?: string;
}

let {
  label,
  meta,
  dot,
  dotLabel,
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
    <StatusDot tone={dot} label={dotLabel} />
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
