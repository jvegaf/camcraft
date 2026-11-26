import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { atomButton } from 'styled-system/recipes';
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

  it('renders Generar plan with the primary Cut button atom', () => {
    render(ToolRail, { onOpen: vi.fn() });

    const plan = screen.getByRole('button', { name: /^Generar plan/ });
    const primary = atomButton({ variant: 'primary', size: 'sm' });

    expect(plan.getAttribute('class')).toContain(primary);
    expect(
      screen.getByRole('button', { name: 'Importar' }).getAttribute('class')
    ).toContain(atomButton({ variant: 'secondary', size: 'sm' }));
  });

  it('keeps the rail landmark of the artboard', () => {
    render(ToolRail, {});

    expect(screen.getByLabelText('Acciones rápidas')).toBeTruthy();
  });
});
