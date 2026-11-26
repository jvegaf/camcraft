<script lang="ts">
import { cx } from 'styled-system/css';
import { statusDot as statusDotRecipe } from 'styled-system/recipes';
import type { StatusDotTone } from './tone';

// Atom: tonal dot. Decorative by default, so a caller that relies on the tone alone must pass a
// label (design/system.md: never signal with color only).
interface Props {
  tone?: StatusDotTone;
  /** Accessible name. Without it the dot is decoration (`aria-hidden`). */
  label?: string;
  class?: string;
}

let { tone = 'neutral', label, class: className = '' }: Props = $props();
</script>

<span
  class={cx(statusDotRecipe({ tone }), className)}
  data-tone={tone}
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
></span>
