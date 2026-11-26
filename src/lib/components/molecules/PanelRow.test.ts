import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import PanelRow from './PanelRow.svelte';

// Molecule contract: the browser/operaciones row — leading dot or icon, label, trailing meta such
// as ".gbr" or "seleccionada". It is a button only when it can be activated.
describe('PanelRow', () => {
  it('renders the label and trailing meta of a static row', () => {
    render(PanelRow, { label: 'Top Copper', meta: '.gbr', dot: 'cut' });

    expect(screen.getByText('Top Copper')).toBeTruthy();
    expect(screen.getByText('.gbr')).toBeTruthy();
    expect(screen.queryByRole('button')).toBeNull();
    expect(screen.getByText('Top Copper').isConnected).toBe(true);
  });

  it('renders the leading dot with its tone', () => {
    render(PanelRow, { label: 'Stock FR-4 1.6mm', meta: '100×80', dot: 'ok' });

    const dot = document.querySelector('[data-tone]');
    expect(dot?.getAttribute('data-tone')).toBe('ok');
    expect(dot?.getAttribute('class')).toContain('statusDot--tone_ok');
  });

  it('renders a leading icon instead of a dot when asked', () => {
    render(PanelRow, { label: 'Importar', icon: 'import' });

    expect(document.querySelector('[data-icon="import"]')).not.toBeNull();
    expect(document.querySelector('[data-tone]')).toBeNull();
  });

  it('is an activatable button when it takes an onclick', async () => {
    const user = userEvent.setup();
    const onclick = vi.fn();
    render(PanelRow, {
      label: 'Drill Hits',
      meta: '.drl',
      dot: 'selected',
      selected: true,
      onclick
    });

    const row = screen.getByRole('button', { name: /Drill Hits/ });
    expect(row.getAttribute('type')).toBe('button');
    expect(row.getAttribute('aria-current')).toBe('true');

    await user.click(row);
    expect(onclick).toHaveBeenCalledTimes(1);
  });

  it('applies the selected variant class and leaves unselected rows unmarked', () => {
    const first = render(PanelRow, {
      label: '2 · Drill 0.8 / 1.0',
      meta: 'seleccionada',
      selected: true,
      onclick: () => {}
    });
    expect(
      screen.getByRole('button', { name: /Drill/ }).getAttribute('class')
    ).toContain('panelRow__root--selected_true');
    first.unmount();

    render(PanelRow, { label: '3 · Isolation top', meta: 'draft', dot: 'cut' });
    expect(screen.getByText('draft')).toBeTruthy();
    expect(document.querySelector('[aria-current]')).toBeNull();
  });
});
