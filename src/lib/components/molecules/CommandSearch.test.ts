import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import CommandSearch from './CommandSearch.svelte';

// Molecule contract: a presentational search field with the artboard's exact placeholder and the
// "⌘K" hint. It reports what was typed through onquery; it never queries anything itself.
describe('CommandSearch', () => {
  it('renders the artboard placeholder and keyboard hint inside a search landmark', () => {
    render(CommandSearch, {});

    const landmark = screen.getByRole('search');
    expect(landmark.getAttribute('aria-label')).toBe('Buscar comando');

    const field = screen.getByRole('searchbox');
    expect(field.getAttribute('placeholder')).toBe(
      'Buscar comando, capa u operación…'
    );
    expect(screen.getByText('⌘K').tagName).toBe('KBD');
  });

  it('reports the typed query through onquery', async () => {
    const user = userEvent.setup();
    const onquery = vi.fn();
    render(CommandSearch, { onquery });

    await user.type(screen.getByRole('searchbox'), 'drill');

    expect(onquery).toHaveBeenLastCalledWith('drill');
  });

  it('lets the caller override the placeholder, shortcut and accessible name', () => {
    render(CommandSearch, {
      placeholder: 'Buscar en browser…',
      shortcut: '⌘F',
      ariaLabel: 'Buscar en browser'
    });

    expect(screen.getByRole('search').getAttribute('aria-label')).toBe(
      'Buscar en browser'
    );
    expect(screen.getByRole('searchbox').getAttribute('placeholder')).toBe(
      'Buscar en browser…'
    );
    expect(screen.getByText('⌘F')).toBeTruthy();
  });

  it('shows a caller-provided initial value', () => {
    render(CommandSearch, { value: 'drill' });

    expect((screen.getByRole('searchbox') as HTMLInputElement).value).toBe(
      'drill'
    );
  });
});
