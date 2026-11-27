import { atomButton, button } from 'styled-system/recipes';
import { describe, expect, it } from 'vitest';

// PRESERVE evidence for task 095 T1: registering the atomic primitive recipes in
// `camcraft.preset.ts` must not change the recipes that already shipped. The start screen renders
// the preset `button` with `variant: 'primary'`, which is the `Sel` accent; the artboard's primary
// CAM action («Generar plan») is the `Cut` accent, so it lives in its own `atomButton` recipe
// instead of rewriting that variant.
describe('atomButton does not replace the existing button recipe', () => {
  it('keeps the preset button primary variant', () => {
    const classes = button({ variant: 'primary', size: 'sm' });

    expect(classes).toContain('button');
    expect(classes).toContain('button--variant_primary');
    expect(classes).toContain('button--size_sm');
  });

  it('exposes the Cut accent as its own primary variant', () => {
    const classes = atomButton({ variant: 'primary', size: 'md' });

    expect(classes).toContain('atomButton');
    expect(classes).toContain('atomButton--variant_primary');
    expect(classes).toContain('atomButton--size_md');
    expect(classes).not.toContain('button--variant_primary');
  });
});
