<script lang="ts">
import { cx } from 'styled-system/css';
import { iconButton as iconButtonRecipe } from 'styled-system/recipes';
import Icon from './Icon.svelte';
import type { IconName } from './icons';

// Atom: square icon-only control. `ariaLabel` is required — an icon alone is never a name.
interface Props {
  ariaLabel: string;
  icon: IconName;
  size?: 'sm' | 'md';
  disabled?: boolean;
  onclick?: (event: MouseEvent) => void;
  class?: string;
}

let {
  ariaLabel,
  icon,
  size = 'md',
  disabled = false,
  onclick,
  class: className = ''
}: Props = $props();
</script>

<button
  type="button"
  class={cx(iconButtonRecipe({ size }), className)}
  aria-label={ariaLabel}
  disabled={disabled}
  {onclick}
>
  <Icon name={icon} />
</button>
