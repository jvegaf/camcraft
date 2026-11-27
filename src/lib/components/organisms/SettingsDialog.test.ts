import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  closeSettings,
  openSettings,
  workbenchState
} from '../workbench/workbench.svelte';
import SettingsDialog from './SettingsDialog.svelte';

// Coverage for the shared settings surface (095 T5 follow-up, risk item 6). The dialog is shared by
// the start screen and the workbench shell, and it is the surface that stores the theme preference,
// so it is asserted through the DOM: it loads the stored settings when opened, applies them, saves
// through `update_settings` and closes. The Tauri bridge is mocked at
// `@tauri-apps/api/core` — the module both the dialog and `src/lib/ds/theme.ts` import.
//
// `design/camcraft-workbench-v2/04-settings.html` is the visual reference (page list, title search,
// two-column rows, footer). Its option list is a placeholder and the binding constraint of tasks 090
// / 091 is "solo settings reales (theme)": `SettingsDto` in `src-tauri/src/lib.rs` persists exactly
// one field, so the page list has exactly one page and the tests pin that instead of inventing
// pages. Everything is asserted through the public surface: rendered roles/text and the command +
// payload the bridge received.
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const resetTheme = () => document.documentElement.removeAttribute('data-theme');

type BridgeHandler = (command: string, args?: unknown) => Promise<unknown>;

function bridge(handler: BridgeHandler) {
  invokeMock.mockImplementation((command: string, args?: unknown) =>
    handler(command, args)
  );
}

// `update_settings_impl` echoes what it stored; `get_settings` returns the stored file content.
const echoStored: BridgeHandler = async (command, args) => {
  if (command === 'get_settings') return { theme: 'light' };
  if (command === 'update_settings') {
    const settings = (args as { settings?: { theme: string } }).settings;
    return { theme: settings?.theme ?? 'dark' };
  }
  throw new Error(`no bridge for ${command}`);
};

const THEME_LABEL: Record<string, string> = {
  dark: 'Oscuro',
  light: 'Claro',
  system: 'Sistema'
};

/** The theme segmented control (artboard `.seg`, `aria-pressed` per option). */
function themeGroup(): HTMLElement {
  return screen.getByRole('group', { name: 'Apariencia de la interfaz' });
}

/** The theme the dialog would store: the single pressed option of the segmented control. */
function selectedTheme(): string {
  const pressed = within(themeGroup())
    .getAllByRole('button')
    .filter(button => button.getAttribute('aria-pressed') === 'true');
  expect(pressed).toHaveLength(1);
  return pressed[0].textContent?.trim() ?? '';
}

function storedTheme(): string {
  return (
    [...Object.entries(THEME_LABEL)].find(
      ([, label]) => label === selectedTheme()
    )?.[0] ?? ''
  );
}

describe('SettingsDialog', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    resetTheme();
    closeSettings();
    bridge(echoStored);
  });

  it('renders nothing until the settings surface is opened', () => {
    render(SettingsDialog);

    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('loads the stored settings and applies the stored theme when it opens', async () => {
    render(SettingsDialog);

    openSettings();

    const dialog = await screen.findByRole('dialog', { name: 'Preferencias' });
    await waitFor(() => expect(selectedTheme()).toBe('Claro'));

    expect(invokeMock).toHaveBeenCalledWith('get_settings');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(dialog.textContent).toContain('Apariencia');
  });

  it('persists the chosen theme through update_settings and closes', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();

    await waitFor(() => expect(selectedTheme()).toBe('Claro'));

    await user.click(
      within(themeGroup()).getByRole('button', { name: 'Oscuro' })
    );
    await user.click(screen.getByRole('button', { name: 'Aplicar' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('update_settings', {
        settings: { theme: 'dark' }
      })
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(workbenchState.settingsOpen).toBe(false);
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('closes on Cancelar without storing anything', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await screen.findByRole('dialog', { name: 'Preferencias' });
    await waitFor(() => expect(selectedTheme()).toBe('Claro'));

    await user.click(screen.getByRole('button', { name: 'Cancelar' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('shows the backend error instead of pretending the settings were read', async () => {
    bridge(async command => {
      if (command === 'get_settings') throw 'state poisoned';
      throw new Error(`no bridge for ${command}`);
    });
    render(SettingsDialog);

    openSettings();

    const error = await screen.findByText('state poisoned');
    // The failure is announced, not only painted (design/system.md, a11y rules).
    expect(error.getAttribute('aria-live')).toBe('assertive');
    expect(selectedTheme()).toBe('Oscuro');
  });

  // --- Real settings only (tasks 090/091) -------------------------------------------------------

  it('renders one page per persisted setting: theme only', async () => {
    render(SettingsDialog);
    openSettings();

    const nav = await screen.findByRole('navigation', {
      name: 'Páginas de preferencias'
    });
    const pages = within(nav).getAllByRole('button');

    // `SettingsDto` persists `theme` and nothing else, so the artboard's placeholder pages
    // (Layout, Canvas, Manufactura, Librería de herramientas, Importar) must not be rendered.
    expect(pages).toHaveLength(1);
    expect(pages[0].textContent?.trim()).toBe('Apariencia');
    expect(pages[0].getAttribute('aria-current')).toBe('page');
    expect(nav.textContent).toContain('General');
    // No fake toggle from the artboard survived either.
    expect(screen.queryByRole('checkbox')).toBeNull();
  });

  it('offers exactly the theme preferences the app stores', async () => {
    render(SettingsDialog);
    openSettings();
    await waitFor(() => expect(selectedTheme()).toBe('Claro'));

    const pressed = THEME_LABEL[storedTheme()];
    const labels = within(themeGroup())
      .getAllByRole('button')
      .map(button => button.textContent?.trim());

    expect(pressed).toBe('Claro');
    expect(labels).toEqual(['Oscuro', 'Claro', 'Sistema']);
    // A rejected/cancelled edit never writes: only the load command ran so far.
    expect(invokeMock).toHaveBeenCalledWith('get_settings');
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
  });

  // --- Titlebar search --------------------------------------------------------------------------

  it('filters setting rows by label and hint, and shows a no-results state', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await screen.findByRole('dialog', { name: 'Preferencias' });

    const search = screen.getByRole('searchbox', {
      name: 'Buscar configuración'
    });
    expect(search.getAttribute('placeholder')).toBe('Buscar configuración…');
    expect(screen.getByText('⌘F')).toBeTruthy();
    expect(screen.getByText('Apariencia de la interfaz')).toBeTruthy();

    // Label match.
    await user.type(search, 'interfaz');
    expect(screen.getByText('Apariencia de la interfaz')).toBeTruthy();

    // Hint-only match: the label does not contain the query.
    await user.clear(search);
    await user.type(search, 'defecto');
    expect(screen.getByText('Apariencia de la interfaz')).toBeTruthy();

    // Case-insensitive.
    await user.clear(search);
    await user.type(search, 'APARIENCIA');
    expect(screen.getByText('Apariencia de la interfaz')).toBeTruthy();

    // Nothing matches: the row is gone and the empty state says so instead of a blank column.
    await user.clear(search);
    await user.type(search, 'drill');
    expect(screen.queryByText('Apariencia de la interfaz')).toBeNull();
    expect(screen.getByText(/Sin resultados para «drill»/)).toBeTruthy();
  });

  it('restores every row when the search is cleared or emptied', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await screen.findByRole('dialog', { name: 'Preferencias' });

    const search = screen.getByRole('searchbox', {
      name: 'Buscar configuración'
    });

    await user.type(search, 'drill');
    await user.click(screen.getByRole('button', { name: 'Limpiar búsqueda' }));

    expect((search as HTMLInputElement).value).toBe('');
    expect(screen.queryByText(/Sin resultados/)).toBeNull();
    expect(screen.getByText('Apariencia de la interfaz')).toBeTruthy();

    await user.type(search, 'drill');
    await user.clear(search);

    expect((search as HTMLInputElement).value).toBe('');
    expect(screen.getByText('Apariencia de la interfaz')).toBeTruthy();
  });

  it('focuses the search field with ⌘F and Ctrl+F', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await screen.findByRole('dialog', { name: 'Preferencias' });

    const search = screen.getByRole('searchbox', {
      name: 'Buscar configuración'
    });
    expect(document.activeElement).not.toBe(search);

    await user.keyboard('{Control>}f{/Control}');
    expect(document.activeElement).toBe(search);

    // The accelerator is a focus shortcut, not a new global hotkey: typing keeps working.
    await user.keyboard('defecto');
    expect((search as HTMLInputElement).value).toBe('defecto');

    (search as HTMLInputElement).blur();
    await user.keyboard('{Meta>}f{/Meta}');
    expect(document.activeElement).toBe(search);
  });

  // --- Footer -----------------------------------------------------------------------------------

  it('renders Restaurar valores / Cancelar / Aplicar as design-system buttons', async () => {
    render(SettingsDialog);
    openSettings();
    await screen.findByRole('dialog', { name: 'Preferencias' });

    expect(
      screen
        .getByRole('button', { name: 'Restaurar valores' })
        .getAttribute('data-variant')
    ).toBe('ghost');
    expect(
      screen
        .getByRole('button', { name: 'Cancelar' })
        .getAttribute('data-variant')
    ).toBe('secondary');
    expect(
      screen
        .getByRole('button', { name: 'Aplicar' })
        .getAttribute('data-variant')
    ).toBe('primary');
  });

  it('Restaurar valores only resets the draft and still requires Aplicar', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await waitFor(() => expect(selectedTheme()).toBe('Claro'));

    await user.click(screen.getByRole('button', { name: 'Restaurar valores' }));

    expect(selectedTheme()).toBe('Oscuro');
    // Nothing reached the backend, and the window theme is untouched: restore is a draft edit.
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
    expect(document.documentElement.dataset.theme).toBe('light');
    // The operator is told the restore is not stored yet.
    expect(screen.getByText(/Cambios sin aplicar/)).toBeTruthy();

    await user.click(screen.getByRole('button', { name: 'Aplicar' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('update_settings', {
        settings: { theme: 'dark' }
      })
    );
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('does not preview or store a theme until Aplicar is pressed', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await waitFor(() => expect(selectedTheme()).toBe('Claro'));

    await user.click(
      within(themeGroup()).getByRole('button', { name: 'Sistema' })
    );

    expect(selectedTheme()).toBe('Sistema');
    expect(screen.getByText(/Cambios sin aplicar/)).toBeTruthy();
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
  });

  // --- Dismissal --------------------------------------------------------------------------------

  it('closes on Escape without writing', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await waitFor(() => expect(selectedTheme()).toBe('Claro'));

    await user.click(
      within(themeGroup()).getByRole('button', { name: 'Oscuro' })
    );
    await user.keyboard('{Escape}');

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(workbenchState.settingsOpen).toBe(false);
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  // --- Preserved behaviour ----------------------------------------------------------------------

  it('keeps closing on a backdrop click', async () => {
    const user = userEvent.setup();
    const { container } = render(SettingsDialog);
    openSettings();
    await screen.findByRole('dialog', { name: 'Preferencias' });

    const backdrop = container.firstElementChild as HTMLElement;
    expect(backdrop).toBeTruthy();
    await user.click(backdrop);

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
  });
});
