import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import TopBar from './TopBar.svelte';

// Organism contract (095 T3, S4 + S8). Reference:
// `design/camcraft-workbench-v2/02-floating-studio.html` `.topbar` — `CommandSearch` in the centre,
// quiet icon actions on the right. The file actions were text buttons before T3; they are icon
// actions now, so the assertions are the accessible names (an icon alone is never a name) plus the
// handler each one still calls. The theme action is the only new behaviour: it flips the shared
// `data-theme` attribute, the same mechanism `SettingsDialog` uses.
const resetTheme = () => document.documentElement.removeAttribute('data-theme');

describe('TopBar', () => {
  beforeEach(resetTheme);
  afterEach(resetTheme);

  it('renders the file actions as named icon actions that still call their handlers', async () => {
    const user = userEvent.setup();
    const onNew = vi.fn();
    const onOpen = vi.fn();
    const onSave = vi.fn();
    const onSaveAs = vi.fn();
    render(TopBar, { onNew, onOpen, onSave, onSaveAs });

    const actions = [
      ['Nuevo', onNew],
      ['Abrir', onOpen],
      ['Guardar', onSave],
      ['Guardar como', onSaveAs]
    ] as const;

    for (const [name, handler] of actions) {
      const button = screen.getByRole('button', { name });
      expect(button.querySelector('svg'), `${name} has no icon`).not.toBeNull();
      await user.click(button);
      expect(handler).toHaveBeenCalledTimes(1);
    }
  });

  it('omits Guardar como on a page that has no such handler', () => {
    render(TopBar, { onNew: vi.fn(), onOpen: vi.fn(), onSave: vi.fn() });

    expect(screen.queryByRole('button', { name: 'Guardar como' })).toBeNull();
    // The actions the start screen does pass keep rendering.
    expect(screen.getByRole('button', { name: 'Nuevo' })).toBeTruthy();
  });

  it('renders the command search with the artboard placeholder and shortcut', () => {
    render(TopBar, {});

    const landmark = screen.getByRole('search');
    expect(landmark.getAttribute('aria-label')).toBe('Buscar comando');
    expect(screen.getByRole('searchbox').getAttribute('placeholder')).toBe(
      'Buscar comando, capa u operación…'
    );
    expect(screen.getByText('⌘K').tagName).toBe('KBD');
  });

  it('flips data-theme on the document element from the theme action', async () => {
    const user = userEvent.setup();
    render(TopBar, {});

    const toggle = screen.getByRole('button', { name: 'Tema' });

    expect(document.documentElement.dataset.theme).toBeUndefined();
    await user.click(toggle);
    expect(document.documentElement.dataset.theme).toBe('light');
    await user.click(toggle);
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('keeps the artboard layout pill and settings action', async () => {
    const user = userEvent.setup();
    const onSettings = vi.fn();
    render(TopBar, { onSettings });

    expect(screen.getByLabelText('Layout flotante').textContent).toBe(
      'Floating layout'
    );
    await user.click(screen.getByRole('button', { name: 'Configuración' }));
    expect(onSettings).toHaveBeenCalledTimes(1);
  });
});
