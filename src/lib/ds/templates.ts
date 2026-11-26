import { defineSlotRecipe } from '@pandacss/dev';

// Recipes for the layout templates of the floating studio (task 095 T2).
//
// Source of truth: `design/system.md` for every token, `design/camcraft-workbench-v2/02-floating-studio.html`
// for the frame geometry. Two values have no design-system token and are reported, not invented:
//
// - The shell radius. `system.md` «Geometría» tops out at `radius-dialog` (10px, the largest
//   container radius); the artboard `.shell` draws 12px. The template consumes the semantic token
//   (`radii.dialog`), so today it resolves to the preset's 12px and converges with `system.md` in T5.
// - The shell shadow. `system.md` only tokenizes the floating-card shadow
//   (`--shadow-float`); the artboard `.shell` is the single source for the outer frame shadow.
//
// The `.canvas-bg` wash is expressed with the semantic colors behind `color-mix` so it flips with
// the theme (`hairline` grid at 32%, `sel` radial at 3.5%) instead of pinning the dark literals.
export const workbenchTemplateRecipe = defineSlotRecipe({
  className: 'workbenchTemplate',
  description: 'Camcraft floating studio frame (layout and slots only)',
  slots: [
    'root',
    'backdrop',
    'topBar',
    'toolRail',
    'viewport',
    'viewportHud',
    'browser',
    'inspector',
    'statusBar'
  ],
  base: {
    root: {
      position: 'relative',
      w: '100%',
      h: '100%',
      minH: '600px',
      overflow: 'hidden',
      // `design/system.md` «Geometría»: radius-dialog is the largest container radius.
      borderRadius: 'dialog',
      // `design/system.md` «Jerarquía de bordes»: Primary (`border`) is the outer card/shell edge.
      border: '1px solid',
      borderColor: 'border',
      boxShadow: '0 24px 80px rgba(0,0,0,.45)',
      bg: 'canvas',
      color: 'text',
      fontSize: '13px',
      lineHeight: '1.4',
      fontFamily: 'sans',
      '& button, & input': {
        font: 'inherit',
        color: 'inherit',
        bg: 'none',
        border: 'none'
      },
      '& button': { cursor: 'pointer' }
    },
    backdrop: {
      position: 'absolute',
      inset: '0',
      pointerEvents: 'none',
      bg: 'canvas',
      backgroundImage: [
        'linear-gradient(90deg, color-mix(in srgb, token(colors.hairline) 32%, transparent) 1px, transparent 1px)',
        'linear-gradient(180deg, color-mix(in srgb, token(colors.hairline) 32%, transparent) 1px, transparent 1px)',
        'radial-gradient(ellipse 70% 50% at 50% 40%, color-mix(in srgb, token(colors.sel) 3.5%, transparent), transparent 70%)'
      ].join(', '),
      backgroundSize: '80px 80px, 80px 80px, auto',
      backgroundRepeat: 'repeat, repeat, no-repeat'
    },
    // The regions are pass-through until T3/T4 move each panel's placement into the template:
    // every child still positions itself absolutely against `root`, which is the same containing
    // block the hand-rolled shell stack used to provide.
    topBar: { display: 'contents' },
    toolRail: { display: 'contents' },
    // 095 T3 item 6: the viewport is inset like the artboard's `.board-svg` (`.board-svg {
    // inset:52px 0 34px 0 }` inside the `.shell` frame) plus the 34px side gutter, so the frame's
    // gridded backdrop reads in the gutter instead of being covered by an opaque canvas. The canvas
    // keeps its own opaque surface — only the region it fills shrinks, and `BoardCanvas` already
    // follows its container with a `ResizeObserver`.
    viewport: {
      position: 'absolute',
      top: '52px',
      right: '34px',
      bottom: '34px',
      left: '34px'
    },
    viewportHud: { display: 'contents' },
    browser: { display: 'contents' },
    inspector: { display: 'contents' },
    statusBar: { display: 'contents' }
  }
});

export const templateSlotRecipes = {
  workbenchTemplate: workbenchTemplateRecipe
};
