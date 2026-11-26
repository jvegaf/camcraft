import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  applyTheme,
  loadThemePreference,
  readTheme,
  saveThemePreference,
  toggleTheme
} from './theme';

// Shared mechanism contract (095 T3 + T5 follow-up): the theme lives on
// `document.documentElement[data-theme]` and the preference is persisted through the settings
// commands `SettingsDialog` already used (`get_settings` / `update_settings`). The dialog and the
// TopBar theme action both go through this module, so the two of them can never disagree about
// which theme was stored; before the persistence moved here the TopBar only flipped the attribute
// and opening Settings silently reverted the visible theme to the stored one.
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const reset = () => document.documentElement.removeAttribute('data-theme');

describe('shared theme mechanism', () => {
  afterEach(() => {
    reset();
    invokeMock.mockReset();
  });

  it('writes the explicit themes SettingsDialog writes', () => {
    applyTheme('light');
    expect(document.documentElement.dataset.theme).toBe('light');

    applyTheme('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('removes the attribute for the non-explicit "system" preference', () => {
    applyTheme('light');
    applyTheme('system');

    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('reads dark as the default and flips between dark and light', () => {
    expect(readTheme()).toBe('dark');

    expect(toggleTheme()).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');

    expect(toggleTheme()).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('persists the chosen theme through update_settings and applies what was stored', async () => {
    invokeMock.mockResolvedValue({ theme: 'light' });

    await expect(saveThemePreference('light')).resolves.toBe('light');

    expect(invokeMock).toHaveBeenCalledWith('update_settings', {
      settings: { theme: 'light' }
    });
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  // Two rapid clicks must not race: the second `update_settings` call waits for the first to settle,
  // so the stored preference is the last one the operator chose and no earlier echo can overwrite
  // the theme displayed afterwards.
  it('serializes two rapid writes so the last click decides what is stored', async () => {
    let open!: (value: { theme: string }) => void;
    let ready!: (value: { theme: string }) => void;
    const written: string[] = [];
    const gates = [
      new Promise<{ theme: string }>(resolve => (open = resolve)),
      new Promise<{ theme: string }>(resolve => (ready = resolve))
    ];
    invokeMock.mockImplementation((_command: string, args) => {
      written.push(args.settings.theme);
      return gates[written.length - 1];
    });

    const first = saveThemePreference('light');
    const second = saveThemePreference('dark');

    // Only the first write is in flight; the second is queued behind it, not racing it.
    await vi.waitFor(() => expect(written).toEqual(['light']));

    open({ theme: 'light' });
    await vi.waitFor(() => expect(written).toEqual(['light', 'dark']));

    ready({ theme: 'dark' });

    await expect(first).resolves.toBe('light');
    await expect(second).resolves.toBe('dark');
    expect(invokeMock).toHaveBeenNthCalledWith(1, 'update_settings', {
      settings: { theme: 'light' }
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'update_settings', {
      settings: { theme: 'dark' }
    });
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  // A failed write must not leave the app showing a theme the backend never stored: the displayed
  // theme goes back to the last preference the backend confirmed, and the caller learns about it.
  it('restores the last persisted theme when the write fails', async () => {
    invokeMock.mockImplementation(
      (command: string, args?: { settings?: { theme: string } }) =>
        command === 'get_settings'
          ? Promise.resolve({ theme: 'light' })
          : Promise.resolve({ theme: args?.settings?.theme })
    );
    await saveThemePreference('light');
    expect(document.documentElement.dataset.theme).toBe('light');

    // The operator's next intent is applied right away, as the bar does before it stores it.
    expect(toggleTheme()).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');

    invokeMock.mockImplementation(
      (command: string, args?: { settings?: { theme: string } }) =>
        command === 'get_settings'
          ? Promise.resolve({ theme: 'light' })
          : args?.settings?.theme === 'dark'
            ? Promise.reject(new Error('disk full'))
            : Promise.resolve({ theme: args?.settings?.theme })
    );
    await expect(saveThemePreference('dark')).rejects.toThrow('disk full');

    expect(document.documentElement.dataset.theme).toBe('light');
  });

  // The window applies the stored preference outside this module (`+layout.svelte` calls
  // `get_settings` and `applyTheme` itself), so the value tracked here can be older than what the
  // backend has. The rollback asks the backend instead of trusting the stale value; here the stored
  // preference is the non-explicit `system`, which is only right if the backend was really asked.
  it('rolls back to the stored preference it never read itself', async () => {
    invokeMock.mockImplementation(
      (command: string, args?: { settings?: { theme: string } }) =>
        command === 'get_settings'
          ? Promise.resolve({ theme: 'system' })
          : args?.settings?.theme === 'light'
            ? Promise.reject(new Error('disk full'))
            : Promise.resolve({ theme: args?.settings?.theme })
    );
    // What the layout did on startup: applied the stored preference without telling this module.
    applyTheme('system');

    expect(toggleTheme()).toBe('light');
    await expect(saveThemePreference('light')).rejects.toThrow('disk full');

    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('applies the preference the backend has stored', async () => {
    invokeMock.mockResolvedValue({ theme: 'light' });

    await expect(loadThemePreference()).resolves.toBe('light');

    expect(invokeMock).toHaveBeenCalledWith('get_settings');
    expect(document.documentElement.dataset.theme).toBe('light');
  });
});
