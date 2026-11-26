import { describe, expect, it } from 'vitest';
import { camcraftPreset } from './camcraft.preset';

// Token convergence contract (095 T5, S6). `design/system.md` «Geometría» is the authority for the
// radii scale: radius-control 4, radius-panel 6, radius-overlay 8, radius-dialog 10, and nothing
// else in the token block may move.
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

const surfaceRadii = [
  panelRecipe.base.borderRadius,
  panelRecipe.variants?.tone.overlay.borderRadius,
  workbenchTemplateRecipe.base.root.borderRadius
];

describe('camcraft preset radii match design/system.md', () => {
  it('exposes the system radii scale', () => {
    expect(radii.control.value).toBe('4px');
    expect(radii.panel.value).toBe('6px');
    expect(radii.overlay.value).toBe('8px');
    expect(radii.dialog.value).toBe('10px');
  });

  // The guarantee S6 asks for: a surface corner is never a literal number, it is a named radius of
  // the system scale, so widening or tightening the scale moves every surface with it.
  it('resolves every surface corner to a radius of the system scale', () => {
    const scale = Object.keys(radii);

    for (const reference of surfaceRadii) {
      expect(typeof reference).toBe('string');
      expect(scale).toContain(reference);
    }
  });

  // PRESERVE: the surfaces keep the distinct corners design/system.md gives them — the docked panel
  // is not the floating card and neither is the shell frame.
  it('keeps the panel, floating card and shell frame on their own scale steps', () => {
    expect(panelRecipe.base.borderRadius).toBe('panel');
    expect(panelRecipe.variants?.tone.overlay.borderRadius).toBe('overlay');
    expect(workbenchTemplateRecipe.base.root.borderRadius).toBe('dialog');
  });
});
