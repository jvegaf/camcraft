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
 * The preference the backend last confirmed: the value `update_settings` echoed or `get_settings`
 * returned. Before either happens it is the palette the app renders with no attribute, which is the
 * documented dark default, so a first write that fails still has something honest to go back to.
 */
let persistedTheme: string = readTheme();

/** Tail of the write chain: every `update_settings` call starts after the previous one settled. */
let writeChain: Promise<unknown> = Promise.resolve();

/** Sequence of the latest requested write, used to keep a stale echo from repainting the app. */
let latestWrite = 0;

/**
 * Store the preference through `update_settings` and apply what the backend stored, so the running
 * theme is always the stored one. `SettingsDialog` and the TopBar theme action both call this.
 *
 * Writes are serialized: two rapid toggles used to overlap two `update_settings` calls and the last
 * resolved promise decided the display, so the window could show a theme the backend did not store.
 * Now the second write starts only after the first settles, and the echo of a write that is no
 * longer the latest intent never repaints the window. A failed write rolls the display back to the
 * preference the backend has stored (the caller reports the failure) instead of leaving the app on
 * a theme that was never saved.
 */
export async function saveThemePreference(theme: string): Promise<string> {
  const request = ++latestWrite;
  const write = writeChain.then(async () => {
    try {
      const saved = await invoke<ThemeSettings>('update_settings', {
        settings: { theme }
      });
      persistedTheme = saved.theme;
      if (request === latestWrite) applyTheme(saved.theme);
      return saved.theme;
    } catch (error) {
      if (request === latestWrite) await restorePersistedTheme();
      throw error;
    }
  });
  // The chain must survive a rejected write, otherwise every later write would be rejected too.
  writeChain = write.catch(() => undefined);
  return write;
}

/**
 * Put the window back on the preference the backend has stored. `get_settings` is the authority
 * because the app also applies the stored theme outside this module (`+layout.svelte` reads it and
 * calls `applyTheme` itself), so the value tracked here may never have seen it. If even that read
 * fails, the last value this module confirmed is the best knowledge available.
 */
async function restorePersistedTheme(): Promise<void> {
  try {
    const stored = await invoke<ThemeSettings>('get_settings');
    persistedTheme = stored.theme;
  } catch {
    // Keep the last confirmed preference.
  }
  applyTheme(persistedTheme);
}

/** Read the stored preference and apply it; returns the theme the app now renders. */
export async function loadThemePreference(): Promise<string> {
  const settings = await invoke<ThemeSettings>('get_settings');
  persistedTheme = settings.theme;
  applyTheme(settings.theme);
  return settings.theme;
}
