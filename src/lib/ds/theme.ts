import { invoke } from '@tauri-apps/api/core';

// Shared theme mechanism (095 T3 + T5 follow-up). The theme is one attribute on the document
// element: `design/system.md` defines the dark palette as the default and the light palette under
// `[data-theme=light]` (see the `_light` condition in `panda.config.ts`), and the preference is
// stored by the settings commands the app already exposes (`get_settings` / `update_settings`,
// `src-tauri/src/lib.rs:265-333`).
//
// `SettingsDialog` used to own all of this; the TopBar theme action needs exactly the same rule, so
// it lives here once and both call it instead of keeping two copies that can drift. Owning the
// persistence here is what keeps them in agreement: a toggle that only flips the attribute is lost
// the next time any surface reads the stored preference.
export type ThemePreference = 'dark' | 'light' | 'system';

export type Theme = 'dark' | 'light';

type ThemeSettings = { theme: string };

/**
 * Write the preference to the document element. An explicit `dark`/`light` becomes `data-theme`;
 * anything else (`system`) removes the attribute so the default dark palette applies.
 */
export function applyTheme(
  theme: string,
  root: HTMLElement = document.documentElement
): void {
  if (theme === 'light' || theme === 'dark') root.dataset.theme = theme;
  else root.removeAttribute('data-theme');
}

/** The theme currently applied. Without the attribute the app renders the default dark palette. */
export function readTheme(root: HTMLElement = document.documentElement): Theme {
  return root.dataset.theme === 'light' ? 'light' : 'dark';
}

/** Flip between the two explicit themes and return the one now applied. */
export function toggleTheme(
  root: HTMLElement = document.documentElement
): Theme {
  const next: Theme = readTheme(root) === 'light' ? 'dark' : 'light';
  applyTheme(next, root);
  return next;
}

/**
 * Store the preference through `update_settings` and apply what the backend stored, so the running
 * theme is always the stored one. `SettingsDialog` and the TopBar theme action both call this.
 */
export async function saveThemePreference(theme: string): Promise<string> {
  const saved = await invoke<ThemeSettings>('update_settings', {
    settings: { theme }
  });
  applyTheme(saved.theme);
  return saved.theme;
}

/** Read the stored preference and apply it; returns the theme the app now renders. */
export async function loadThemePreference(): Promise<string> {
  const settings = await invoke<ThemeSettings>('get_settings');
  applyTheme(settings.theme);
  return settings.theme;
}
