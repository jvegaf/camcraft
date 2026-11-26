import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import ToolRail from './ToolRail.svelte';

// Organism contract (095 T3, S4). Reference:
// `design/camcraft-workbench-v2/02-floating-studio.html` `.tool-rail` — Importar, Capas, Origen and
// the primary Cut action Generar plan. Only Importar has a real command behind it (the file dialog
// plus `open_document`, which moved here from the old Abrir tool); Capas, Origen and Generar plan
// have no command in the app yet, so they must render as visibly unavailable controls instead of
// buttons that swallow a click and do nothing.
describe('ToolRail', () => {
  it('opens a document from Importar', async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    render(ToolRail, { onOpen });

    await user.click(screen.getByRole('button', { name: 'Importar' }));

    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it.each(['Capas', 'Origen', 'Generar plan'])(
    'renders %s as an unavailable control that says why',
    name => {
      render(ToolRail, { onOpen: vi.fn() });

      const button = screen.getByRole('button', {
        name: new RegExp(`^${name}`)
      }) as HTMLButtonElement;

      expect(button.disabled).toBe(true);
      // The reason is part of the control's own text, so it is not a hover-only affordance.
      expect(button.textContent).toContain('todavía no');
    }
  );

  // The rail's shape as the operator reads it: the four artboard actions in order, and exactly one
  // of them is the primary CAM (Cut) action — «Generar plan», the last one.
  it('renders the four artboard actions in order with a single primary action', () => {
    render(ToolRail, { onOpen: vi.fn() });

    const rail = screen.getByLabelText('Acciones rápidas');
    const buttons = within(rail).getAllByRole('button');

    expect(
      buttons.map(button => button.textContent?.split('—')[0].trim())
    ).toEqual(['Importar', 'Capas', 'Origen', 'Generar plan']);
    expect(
      buttons.filter(
        button => button.getAttribute('data-variant') === 'primary'
      ).length
    ).toBe(1);
    expect(
      screen
        .getByRole('button', { name: /^Generar plan/ })
        .getAttribute('data-variant')
    ).toBe('primary');
  });

  it('keeps the rail landmark of the artboard', () => {
    render(ToolRail, {});

    expect(screen.getByLabelText('Acciones rápidas')).toBeTruthy();
  });
});
