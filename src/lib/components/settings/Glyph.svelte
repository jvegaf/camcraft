<script lang="ts">
import { cx } from 'styled-system/css';
import { icon as iconRecipe } from 'styled-system/recipes';

// Local glyphs of the settings surface. `design/camcraft-workbench-v2/04-settings.html` draws a
// close cross (titlebar) that `$lib/components/atoms/icons.ts` does not have, and that file is a
// shared surface this task must not edit, so the few strokes the dialog needs live next to the
// dialog. The artboard's chevron and check marks are not drawn: they belong to the `select` and the
// checkbox rows, and neither control survived into the real settings contract (theme only), so
// drawing them would be decoration for options the app does not have.
//
// The drawing uses the `icon` recipe, so stroke width, caps and size come from the same token path
// as every atom glyph instead of a second copy of the artboard's SVG attributes.
type GlyphName = 'close';

const GLYPHS: Record<GlyphName, string[]> = {
  close: ['M6 6l12 12', 'M18 6L6 18']
};

interface Props {
  name?: GlyphName;
  size?: 'sm' | 'md' | 'lg';
  class?: string;
}

let { name = 'close', size = 'md', class: className = '' }: Props = $props();
</script>

<svg
  class={cx(iconRecipe({ size }), className)}
  viewBox="0 0 24 24"
  data-glyph={name}
  aria-hidden="true"
  stroke="currentColor"
  fill="none"
>
  {#each GLYPHS[name] as d}
    <path {d} />
  {/each}
</svg>
