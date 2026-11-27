import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import CommandSearch from './CommandSearch.svelte';

// Molecule contract: a presentational search field with the artboard's exact placeholder and the
// "⌘K" hint. It reports what was typed through `onquery`; it never queries anything itself.
//
// The app has no command palette wired yet and `TopBar` passes no handler today, so the honest
// state is a real `disabled` field that says why it is unavailable (ToolRail's policy for Capas /
// Origen / Generar plan). An enabled field that accepts and discards what the operator types is the
// regression this contract pins.
describe('CommandSearch', () => {
  it('renders the artboard placeholder and keyboard hint inside a search landmark', () => {
    render(CommandSearch, { onquery: vi.fn() });

    const landmark = screen.getByRole('search');
    expect(landmark.getAttribute('aria-label')).toBe('Search command');

    const field = screen.getByRole('searchbox');
    expect(field.getAttribute('placeholder')).toBe(
      'Search command, layer or operation…'
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

  // No handler: the field must not silently accept text. It is disabled, it keeps the artboard
  // placeholder, and the reason is rendered as text the operator (and a screen reader, through
  // `aria-describedby`) can read — not merely a dimmed colour, and not a shortcut hint for a command
  // that does not exist.
  it('renders a disabled field with the reason instead of an inert input', () => {
    render(CommandSearch, {});

    const field = screen.getByRole('searchbox') as HTMLInputElement;
    expect(field.disabled).toBe(true);
    expect(field.getAttribute('placeholder')).toBe(
      'Search command, layer or operation…'
    );

    const reason = screen.getByText(/not available yet/);
    expect(reason.textContent).toMatch(/search/i);
    expect(reason.textContent).not.toBe('');
    expect(field.getAttribute('aria-describedby')).toBe(reason.id);
    expect(reason.id).not.toBe('');

    // A disabled field keeps native focus semantics: it is not focusable and holds no shortcut.
    field.focus();
    expect(document.activeElement).not.toBe(field);
    expect(screen.queryByText('⌘K')).toBeNull();
  });

  it('lets the caller override the placeholder, shortcut and accessible name', () => {
    render(CommandSearch, {
      placeholder: 'Search in browser…',
      shortcut: '⌘F',
      ariaLabel: 'Search in browser',
      onquery: vi.fn()
    });

    expect(screen.getByRole('search').getAttribute('aria-label')).toBe(
      'Search in browser'
    );
    expect(screen.getByRole('searchbox').getAttribute('placeholder')).toBe(
      'Search in browser…'
    );
    expect(screen.getByText('⌘F')).toBeTruthy();
  });

  it('shows a caller-provided initial value', () => {
    render(CommandSearch, { value: 'drill', onquery: vi.fn() });

    expect((screen.getByRole('searchbox') as HTMLInputElement).value).toBe(
      'drill'
    );
  });
});
