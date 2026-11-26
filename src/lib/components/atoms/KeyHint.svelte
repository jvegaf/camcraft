<script lang="ts">
import { cx } from 'styled-system/css';
import { keyHint as keyHintRecipe } from 'styled-system/recipes';

// Atom: keyboard shortcut chip. The artboard chrome says exactly "⌘K".
interface Props {
  keys?: string;
  class?: string;
}

let { keys = '⌘K', class: className = '' }: Props = $props();
</script>

<kbd class={cx(keyHintRecipe(), className)}>{keys}</kbd>
