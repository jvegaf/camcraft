import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import { setSelection } from '../workbench/workbench.svelte';
import ViewportHud from './ViewportHud.svelte';

// Organism contract (095 T3, S4 + S6 follow-up). Reference:
// `design/camcraft-workbench-v2/02-floating-studio.html` `.hud` — the viewport label card and the
// selection card. "2D Board" and " · Grid 1 mm" are chrome strings (verbatim); the selection count
// is sample data in the artboard, so it is rendered from the real selection. With nothing selected
// the selection card must still be there and say so (`design/system.md` «Accesibilidad y
// comportamiento»: estados vacíos explícitos) instead of disappearing.
describe('ViewportHud', () => {
  beforeEach(() => setSelection(null));

  it('labels the viewport with the artboard 2D Board and grid strings', () => {
    render(ViewportHud, {});

    const hud = screen.getByLabelText('Estado de viewport');
    expect(hud.textContent).toContain('2D Board');
    expect(hud.textContent).toContain(' · Grid 1 mm');
  });

  it('states Sin selección instead of dropping the selection card', () => {
    render(ViewportHud, {});

    const hud = screen.getByLabelText('Estado de viewport');
    expect(hud.textContent).toContain('Sin selección');
    // The empty state is text, never a made-up count.
    expect(hud.textContent).not.toMatch(/triángulo/);
  });

  // The viewport selects one triangle per click (`workbench.svelte.ts`), so the card states that
  // single selection. It never guesses a set the app cannot have: no plural wording, no count.
  it('states the single selection the app can make, without a count it cannot have', () => {
    setSelection({ kind: 'triangle', index: 3, avgDepthMm: 0.4 });
    render(ViewportHud, {});

    const hud = screen.getByLabelText('Estado de viewport');
    expect(hud.textContent).toContain('2D Board');
    // Exactly the single-selection wording: no count, no plural.
    expect(screen.getByText('1 triángulo seleccionado').textContent).toBe(
      '1 triángulo seleccionado'
    );
    expect(hud.textContent).not.toMatch(/triángulos/);
    expect(screen.queryByText('Sin selección')).toBeNull();
  });
});
