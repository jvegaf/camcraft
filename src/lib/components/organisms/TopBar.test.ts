import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setStatus, workbenchState } from '../workbench/workbench.svelte';
import TopBar from './TopBar.svelte';

// Organism contract (095 T3, S4 + S8 + T5 follow-up). Reference:
// `design/camcraft-workbench-v2/02-floating-studio.html` `.topbar` — `CommandSearch` in the centre,
// quiet icon actions on the right. The file actions were text buttons before T3; they are icon
// actions now, so the assertions are the accessible names (an icon alone is never a name) plus the
// handler each one still calls. The theme action applies *and persists* the theme through the shared
// mechanism (`src/lib/ds/theme.ts`, `update_settings`) — the command `SettingsDialog` stores with —
// so opening the dialog after a toggle no longer reverts the visible theme.
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const resetTheme = () => document.documentElement.removeAttribute('data-theme');

describe('TopBar', () => {
  beforeEach(() => {
    resetTheme();
    setStatus('Ready', 'ok');
    invokeMock.mockReset();
    // The backend echoes what it stored, exactly like `update_settings_impl` does.
    invokeMock.mockImplementation(
      (command: string, args?: { settings?: { theme: string } }) =>
        command === 'update_settings'
          ? Promise.resolve({ theme: args?.settings?.theme })
          : Promise.reject(new Error(`no bridge for ${command}`))
    );
  });
  afterEach(resetTheme);

  it('renders the file actions as named icon actions that still call their handlers', async () => {
    const user = userEvent.setup();
    const onNew = vi.fn();
    const onOpen = vi.fn();
    const onSave = vi.fn();
    const onSaveAs = vi.fn();
    render(TopBar, { onNew, onOpen, onSave, onSaveAs });

    const actions = [
      ['New', onNew],
      ['Open', onOpen],
      ['Save', onSave],
      ['Save as', onSaveAs]
    ] as const;

    for (const [name, handler] of actions) {
      const button = screen.getByRole('button', { name });
      expect(button.querySelector('svg'), `${name} has no icon`).not.toBeNull();
      await user.click(button);
      expect(handler).toHaveBeenCalledTimes(1);
    }
  });

  it('gives Save and Save as their own glyphs', () => {
    render(TopBar, { onSave: vi.fn(), onSaveAs: vi.fn() });

    const glyphOf = (name: string) =>
      screen.getByRole('button', { name }).querySelector('svg');

    // 095 T5 (L8d): both actions import the same file, so they must not look identical.
    expect(glyphOf('Save')?.getAttribute('data-icon')).toBe('save');
    expect(glyphOf('Save as')?.getAttribute('data-icon')).toBe('save-as');
  });

  it('omits Save as on a page that has no such handler', () => {
    render(TopBar, { onNew: vi.fn(), onOpen: vi.fn(), onSave: vi.fn() });

    expect(screen.queryByRole('button', { name: 'Save as' })).toBeNull();
    // The actions the start screen does pass keep rendering.
    expect(screen.getByRole('button', { name: 'New' })).toBeTruthy();
  });

  // 095 T5 follow-up: the bar passes no query handler, so the search renders the artboard
  // placeholder in an honest disabled state instead of accepting text nobody consumes.
  it('renders the command search with the artboard placeholder', () => {
    render(TopBar, {});

    const landmark = screen.getByRole('search');
    expect(landmark.getAttribute('aria-label')).toBe('Search command');

    const field = screen.getByRole('searchbox') as HTMLInputElement;
    expect(field.getAttribute('placeholder')).toBe(
      'Search command, layer or operation…'
    );
    expect(field.disabled).toBe(true);
    expect(landmark.textContent).toMatch(/not available yet/);
  });

  it('flips data-theme on the document element from the theme action', async () => {
    const user = userEvent.setup();
    render(TopBar, {});

    const toggle = screen.getByRole('button', { name: 'Theme' });

    expect(document.documentElement.dataset.theme).toBeUndefined();
    await user.click(toggle);
    expect(document.documentElement.dataset.theme).toBe('light');
    await user.click(toggle);
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  // The regression the independent verification found: a toggle that only flips the attribute is
  // lost as soon as a surface reads the stored preference (the Settings dialog does).
  it('persists the theme it applies through the settings command', async () => {
    const user = userEvent.setup();
    render(TopBar, {});

    await user.click(screen.getByRole('button', { name: 'Theme' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('update_settings', {
        settings: { theme: 'light' }
      })
    );
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  // Two rapid clicks used to overlap two writes, so the display was decided by whichever promise
  // resolved last. They are serialized now: the second write does not even start until the first
  // settles, and the stored preference follows the clicks.
  it('stores two rapid toggles in click order', async () => {
    const user = userEvent.setup();
    const pending: Array<() => void> = [];
    invokeMock.mockImplementation(
      (_command: string, args?: { settings?: { theme: string } }) =>
        new Promise(resolve =>
          pending.push(() => resolve({ theme: args?.settings?.theme }))
        )
    );
    render(TopBar, {});

    const toggle = screen.getByRole('button', { name: 'Theme' });
    await user.click(toggle);
    await user.click(toggle);

    // Only the first write is in flight; a racing second one would already be here.
    await waitFor(() => expect(pending).toHaveLength(1));
    pending[0]();

    await waitFor(() => expect(pending).toHaveLength(2));
    pending[1]();

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
    expect(invokeMock).toHaveBeenNthCalledWith(1, 'update_settings', {
      settings: { theme: 'light' }
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'update_settings', {
      settings: { theme: 'dark' }
    });
    await waitFor(() =>
      expect(document.documentElement.dataset.theme).toBe('dark')
    );
  });

  // A write the backend refused leaves nothing to restore from the click itself, so the bar goes
  // back to the preference the backend confirmed and says so on the status line.
  it('reports a failed theme write and restores the stored theme', async () => {
    const user = userEvent.setup();
    // The failure depends on the value written, so the test does not depend on which write runs first;
    // `get_settings` reports what the backend really kept, which is where the bar rolls back to.
    const persisted = { theme: 'dark' };
    invokeMock.mockImplementation(
      (command: string, args?: { settings?: { theme: string } }) => {
        if (command === 'get_settings') return Promise.resolve(persisted);
        const theme = args?.settings?.theme ?? '';
        if (theme === 'dark') {
          return Promise.reject(new Error('permission denied'));
        }
        persisted.theme = theme;
        return Promise.resolve({ theme });
      }
    );
    render(TopBar, {});

    const toggle = screen.getByRole('button', { name: 'Theme' });
    await user.click(toggle);
    await waitFor(() =>
      expect(document.documentElement.dataset.theme).toBe('light')
    );

    await user.click(toggle);

    await waitFor(() =>
      expect(workbenchState.statusMessage).toBe(
        'Error: could not save the theme (Error: permission denied)'
      )
    );
    expect(workbenchState.statusKind).toBe('error');
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('keeps the artboard layout pill and settings action', async () => {
    const user = userEvent.setup();
    const onSettings = vi.fn();
    render(TopBar, { onSettings });

    expect(screen.getByLabelText('Floating layout').textContent).toBe(
      'Floating layout'
    );
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    expect(onSettings).toHaveBeenCalledTimes(1);
  });
});
