// Shared theme mechanism (095 T3). The theme is one attribute on the document element:
// `design/system.md` defines the dark palette as the default and the light palette under
// `[data-theme=light]` (see the `_light` condition in `panda.config.ts`).
//
// `SettingsDialog` used to own this rule; the TopBar theme action needs exactly the same rule, so it
// lives here once and both call it instead of keeping two copies that can drift.
export type ThemePreference = 'dark' | 'light' | 'system';

export type Theme = 'dark' | 'light';

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
