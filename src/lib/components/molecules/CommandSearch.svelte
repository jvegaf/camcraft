<script lang="ts">
import { cx } from 'styled-system/css';
import { commandSearch as commandSearchRecipe } from 'styled-system/recipes';
import Icon from '../atoms/Icon.svelte';
import KeyHint from '../atoms/KeyHint.svelte';

// Molecule: the command search field of the app bar. Presentational: it renders the artboard
// placeholder and the keyboard hint, and reports what was typed through `onquery`. It never
// searches anything itself.
//
// Without `onquery` there is nothing that could consume the text, so the field renders in the same
// honest state `ToolRail` uses for the actions the app cannot run yet: a real `disabled` input plus
// its reason as text (`aria-describedby`) instead of an enabled field that swallows what the
// operator types. The reason takes the place of the shortcut hint, because a `⌘K` hint would
// advertise an accelerator for a command that does not exist.
interface Props {
  placeholder?: string;
  shortcut?: string;
  ariaLabel?: string;
  value?: string;
  onquery?: (query: string) => void;
  class?: string;
}

let {
  placeholder = 'Buscar comando, capa u operación…',
  shortcut = '⌘K',
  ariaLabel = 'Buscar comando',
  value = '',
  onquery,
  class: className = ''
}: Props = $props();

const styles = commandSearchRecipe();
const reasonId = $props.id();
const enabled = $derived(typeof onquery === 'function');

function handleInput(event: Event) {
  onquery?.((event.currentTarget as HTMLInputElement).value);
}
</script>

<div class={cx(styles.root, className)} role="search" aria-label={ariaLabel}>
  <Icon name="search" size="sm" />
  <input
    class={styles.field}
    type="search"
    {placeholder}
    aria-label={ariaLabel}
    {value}
    disabled={!enabled}
    aria-describedby={enabled ? undefined : reasonId}
    oninput={handleInput}
  />
  {#if enabled}
    <KeyHint keys={shortcut} />
  {:else}
    <span id={reasonId} data-reason="no-query-handler" class={styles.reason}
      >Búsqueda todavía no disponible</span
    >
  {/if}
</div>
