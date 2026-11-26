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

  it('applies the preference the backend has stored', async () => {
    invokeMock.mockResolvedValue({ theme: 'light' });

    await expect(loadThemePreference()).resolves.toBe('light');

    expect(invokeMock).toHaveBeenCalledWith('get_settings');
    expect(document.documentElement.dataset.theme).toBe('light');
  });
});
