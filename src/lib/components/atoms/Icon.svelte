<script lang="ts">
import { cx } from 'styled-system/css';
import { icon as iconRecipe } from 'styled-system/recipes';
import { ICONS, type IconName } from './icons';

// Atom: a single visual primitive. It draws, it does not decide: no store, no Tauri, no events.
interface Props {
  name: IconName;
  size?: 'sm' | 'md' | 'lg';
  /** Accessible name. Without it the icon is decoration (`aria-hidden`). */
  label?: string;
  class?: string;
}

let { name, size = 'md', label, class: className = '' }: Props = $props();

let shapes = $derived(ICONS[name]);
</script>

<svg
  class={cx(iconRecipe({ size }), className)}
  viewBox="0 0 24 24"
  data-icon={name}
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
  stroke="currentColor"
  fill="none"
>
  {#each shapes as shape}
    {#if shape.tag === 'path'}
      <path d={shape.d} />
    {:else if shape.tag === 'circle'}
      <circle cx={shape.cx} cy={shape.cy} r={shape.r} />
    {:else}
      <rect
        x={shape.x}
        y={shape.y}
        width={shape.w}
        height={shape.h}
        rx={shape.rx}
      />
    {/if}
  {/each}
</svg>
