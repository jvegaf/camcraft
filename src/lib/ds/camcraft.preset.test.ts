import { panel } from 'styled-system/recipes';
import { describe, expect, it } from 'vitest';
import { camcraftPreset } from './camcraft.preset';

// Token convergence contract (095 T5, S6). `design/system.md` «Geometría» is the authority for the
// radii scale: radius-control 4, radius-panel 6, radius-overlay 8, radius-dialog 10. The preset
// shipped 4/8/10/12 for T1–T4; T5 aligns it, and nothing else in the token block may move.
type RadiusConfig = { value: string };
type RecipeConfig = {
  base: Record<string, Record<string, unknown>>;
  variants?: Record<string, Record<string, Record<string, unknown>>>;
};

const extend = (
  camcraftPreset as unknown as {
    theme: {
      extend: {
        tokens: { radii: Record<string, RadiusConfig> };
        recipes: { panel: RecipeConfig };
        slotRecipes: { workbenchTemplate: RecipeConfig };
      };
    };
  }
).theme.extend;

const radii = extend.tokens.radii;
const panelRecipe = extend.recipes.panel;
const workbenchTemplateRecipe = extend.slotRecipes.workbenchTemplate;

describe('camcraft preset radii match design/system.md', () => {
  it('exposes the system radii scale', () => {
    expect(radii.control.value).toBe('4px');
    expect(radii.panel.value).toBe('6px');
    expect(radii.overlay.value).toBe('8px');
    expect(radii.dialog.value).toBe('10px');
  });

  // The values only reach the UI when the surfaces keep consuming the semantic tokens instead of
  // literal pixels: the floating cards use the overlay radius and the shell frame the dialog one.
  it('keeps the surfaces wired to the semantic radii tokens', () => {
    expect(panelRecipe.base.borderRadius).toBe('panel');
    expect(panelRecipe.variants?.tone.overlay.borderRadius).toBe('overlay');
    expect(workbenchTemplateRecipe.base.root).toMatchObject({
      borderRadius: 'dialog'
    });
    // `panel({ tone: 'overlay' })` is what `FloatCard` renders for the two floating panels.
    expect(panel({ tone: 'overlay' })).toContain('panel--tone_overlay');
  });
});
