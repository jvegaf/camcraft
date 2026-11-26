<script lang="ts">
import { cx } from 'styled-system/css';
import { commandSearch as commandSearchRecipe } from 'styled-system/recipes';
import Icon from '../atoms/Icon.svelte';
import KeyHint from '../atoms/KeyHint.svelte';

// Molecule: the command search field of the app bar. Presentational: it renders the artboard
// placeholder and the keyboard hint, and reports what was typed through `onquery`. It never
// searches anything itself.
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
    oninput={handleInput}
  />
  <KeyHint keys={shortcut} />
</div>
