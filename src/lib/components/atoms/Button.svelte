<script lang="ts">
import { cx } from 'styled-system/css';
import { atomButton as atomButtonRecipe } from 'styled-system/recipes';
import type { Snippet } from 'svelte';

// Atom: labelled button. `primary` is the `Cut` accent of the design system (system.md: primary
// fabrication action), which is what the artboard's «Generar plan» uses.
//
// The variant and size are also exposed as `data-variant` / `data-size` so a control's role in the
// surface is a DOM fact instead of a hashed class name: the accents come from the design system and
// jsdom computes no styles, so this is what a contract test can honestly read.
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
  type={type}
  disabled={disabled}
  data-variant={variant}
  data-size={size}
  {onclick}
>
  {@render children()}
</button>
