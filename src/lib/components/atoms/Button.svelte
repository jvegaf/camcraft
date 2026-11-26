<script lang="ts">
import { cx } from 'styled-system/css';
import { atomButton as atomButtonRecipe } from 'styled-system/recipes';
import type { Snippet } from 'svelte';

// Atom: labelled button. `primary` is the `Cut` accent of the design system (system.md: primary
// fabrication action), which is what the artboard's «Generar plan» uses.
interface Props {
  variant?: 'primary' | 'secondary' | 'ghost';
  size?: 'sm' | 'md';
  type?: 'button' | 'submit' | 'reset';
  disabled?: boolean;
  onclick?: (event: MouseEvent) => void;
  class?: string;
  children: Snippet;
}

let {
  variant = 'secondary',
  size = 'md',
  type = 'button',
  disabled = false,
  onclick,
  class: className = '',
  children
}: Props = $props();
</script>

<button
  class={cx(atomButtonRecipe({ variant, size }), className)}
  {type}
  disabled={disabled}
  {onclick}
>
  {@render children()}
</button>
