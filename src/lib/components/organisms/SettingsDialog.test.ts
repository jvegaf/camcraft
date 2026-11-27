import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  closeSettings,
  openSettings,
  workbenchState
} from '../workbench/workbench.svelte';
import SettingsDialog, {
  DIALOG_SIZE_STANDARD,
  DIALOG_SIZE_WIDE
} from './SettingsDialog.svelte';

// Coverage for the shared settings surface (095 T5 follow-up, risk item 6). The dialog is shared by
// the start screen and the workbench shell, and it is the surface that stores the theme preference,
// so it is asserted through the DOM: it loads the stored settings when opened, applies them, saves
// through `update_settings` and closes. The Tauri bridge is mocked at
// `@tauri-apps/api/core` — the module both the dialog and `src/lib/ds/theme.ts` import.
//
// `design/camcraft-workbench-v2/04-settings.html` is the visual reference (page list, title search,
// two-column rows, footer) and 05 adds the `Tool library` page inside it (107 T3, S5 + S6). The
// binding constraint of tasks 090/091 is "solo settings reales": a page is rendered only when it has
// something real behind it, so the list holds `Appearance` (the theme `SettingsDto` persists) and
// `Tool library` (the CAM library) and nothing else — the artboard's Layout, Canvas, Manufacturing
// and Import entries stay unrendered. Everything is asserted through the public surface: rendered
// roles/text and the command + payload the bridge received.
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

/** The dialog footer of artboard 05 (107 S12): the five actions, in DOM order. */
function footerOf(dialog: HTMLElement): HTMLElement {
  return within(dialog).getByRole('button', { name: 'Restore defaults' })
    .parentElement as HTMLElement;
}

/** A footer action by its exact label. Scoped to the footer: the titlebar icon is also `Close`. */
function footerButton(dialog: HTMLElement, name: string): HTMLElement {
  return within(footerOf(dialog)).getByRole('button', { name });
}

const THEME_LABEL: Record<string, string> = {
  dark: 'Dark',
  light: 'Light',
  system: 'System'
};

/** The theme segmented control (artboard `.seg`, `aria-pressed` per option). */
function themeGroup(): HTMLElement {
  return screen.getByRole('group', { name: 'Interface appearance' });
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

    const dialog = await screen.findByRole('dialog', { name: 'Preferences' });
    await waitFor(() => expect(selectedTheme()).toBe('Light'));

    expect(invokeMock).toHaveBeenCalledWith('get_settings');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(dialog.textContent).toContain('Appearance');
  });

  it('persists the chosen theme through update_settings and closes', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();

    await waitFor(() => expect(selectedTheme()).toBe('Light'));

    await user.click(
      within(themeGroup()).getByRole('button', { name: 'Dark' })
    );
    const dialog = screen.getByRole('dialog', { name: 'Preferences' });
    await user.click(footerButton(dialog, 'Save changes'));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('update_settings', {
        settings: { theme: 'dark' }
      })
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(workbenchState.settingsOpen).toBe(false);
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('closes on Close without storing anything', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    const dialog = await screen.findByRole('dialog', {
      name: 'Preferences'
    });
    await waitFor(() => expect(selectedTheme()).toBe('Light'));

    await user.click(footerButton(dialog, 'Close'));

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
    expect(selectedTheme()).toBe('Dark');
  });

  // --- Real settings only (tasks 090/091) -------------------------------------------------------

  it('renders one page per real surface: the theme and the tool library', async () => {
    render(SettingsDialog);
    openSettings();

    const nav = await screen.findByRole('navigation', {
      name: 'Preferences pages'
    });
    const pages = within(nav).getAllByRole('button');

    // `SettingsDto` persists `theme` and the CAM library has its own commands, so those are the two
    // pages. The artboard's remaining placeholder pages (Layout, Canvas, Manufacturing, Import and
    // the `General` entry) must still not be rendered.
    expect(pages.map(page => page.textContent?.trim())).toEqual([
      'Appearance',
      'Tool library'
    ]);
    expect(pages[0].getAttribute('aria-current')).toBe('page');
    expect(pages[1].getAttribute('aria-current')).toBeNull();
    expect(nav.textContent).toContain('General');
    expect(nav.textContent).toContain('CAM');
    for (const absent of [
      'General',
      'Layout',
      'Canvas',
      'Manufacturing',
      'Import'
    ]) {
      expect(within(nav).queryByRole('button', { name: absent })).toBeNull();
    }
    // No fake toggle from the artboard survived either.
    expect(screen.queryByRole('checkbox')).toBeNull();
  });

  it('offers exactly the theme preferences the app stores', async () => {
    render(SettingsDialog);
    openSettings();
    await waitFor(() => expect(selectedTheme()).toBe('Light'));

    const pressed = THEME_LABEL[storedTheme()];
    const labels = within(themeGroup())
      .getAllByRole('button')
      .map(button => button.textContent?.trim());

    expect(pressed).toBe('Light');
    expect(labels).toEqual(['Dark', 'Light', 'System']);
    // A rejected/cancelled edit never writes: only the load command ran so far.
    expect(invokeMock).toHaveBeenCalledWith('get_settings');
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
  });

  // --- Tool library page (107 T3, S5 + S6) ------------------------------------------------------

  /**
   * The dialog's published size variant. jsdom computes no layout, so the width/height pair a variant
   * stands for is not observable here; what is observable — and what this file owns — is which
   * variant the active page applies. It is read from the dialog's own `data-dialog-size` and named by
   * the constants the dialog exports, so this test never restates the geometry and never depends on a
   * generated class name.
   */
  function dialogSize(dialog: HTMLElement): string | null {
    return dialog.getAttribute('data-dialog-size');
  }

  it('mounts the tool library page when its entry is selected', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();

    const nav = await screen.findByRole('navigation', {
      name: 'Preferences pages'
    });
    await user.click(within(nav).getByRole('button', { name: 'Tool library' }));

    // The body is the real organism (107 T4 shell): a page heading with the artboard's copy and
    // three headed columns, with no invented rows.
    expect(
      screen.getByRole('heading', { level: 3, name: 'Tool library' })
    ).toBeTruthy();
    expect(screen.getByText(/Tools available for CAM operations/)).toBeTruthy();
    for (const column of ['Tools', 'Detail', 'Preview']) {
      expect(screen.getByRole('region', { name: column })).toBeTruthy();
    }

    // The nav marks the selected page with `aria-current`, and only it.
    expect(
      within(nav)
        .getByRole('button', { name: 'Tool library' })
        .getAttribute('aria-current')
    ).toBe('page');
    expect(
      within(nav)
        .getByRole('button', { name: 'Appearance' })
        .getAttribute('aria-current')
    ).toBeNull();
    // Nothing from the theme page bleeds into the library page.
    expect(
      screen.queryByRole('group', { name: 'Interface appearance' })
    ).toBeNull();
  });

  it('sizes the dialog as each artboard draws it', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();

    const dialog = await screen.findByRole('dialog', { name: 'Preferences' });
    const nav = screen.getByRole('navigation', { name: 'Preferences pages' });

    // Artboard 04 keeps its standard geometry for the settings pages…
    expect(dialogSize(dialog)).toBe(DIALOG_SIZE_STANDARD);

    // …and artboard 05's library page applies the wide variant so the three columns fit (S6).
    await user.click(within(nav).getByRole('button', { name: 'Tool library' }));
    expect(dialogSize(dialog)).toBe(DIALOG_SIZE_WIDE);

    // The size follows the page, it is not a one-way switch.
    await user.click(within(nav).getByRole('button', { name: 'Appearance' }));
    expect(dialogSize(dialog)).toBe(DIALOG_SIZE_STANDARD);
  });

  it('keeps Appearance working after visiting the tool library', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await waitFor(() => expect(selectedTheme()).toBe('Light'));

    const nav = screen.getByRole('navigation', { name: 'Preferences pages' });
    await user.click(within(nav).getByRole('button', { name: 'Tool library' }));
    await user.click(within(nav).getByRole('button', { name: 'Appearance' }));

    expect(screen.getByText('Interface appearance')).toBeTruthy();
    expect(selectedTheme()).toBe('Light');

    await user.click(
      within(themeGroup()).getByRole('button', { name: 'Dark' })
    );
    await user.click(
      footerButton(
        screen.getByRole('dialog', { name: 'Preferences' }),
        'Save changes'
      )
    );

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('update_settings', {
        settings: { theme: 'dark' }
      })
    );
  });

  it('keeps the library page while the search has no matching row', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();

    const nav = await screen.findByRole('navigation', {
      name: 'Preferences pages'
    });
    await user.click(within(nav).getByRole('button', { name: 'Tool library' }));

    const search = screen.getByRole('searchbox', { name: 'Search settings' });
    await user.type(search, 'drill');

    // The search filters setting rows only; the library page has none, so it keeps rendering
    // instead of being replaced by the empty state or by invented rows.
    expect(screen.getByRole('region', { name: 'Tools' })).toBeTruthy();
    expect(screen.queryByText(/No results for/)).toBeNull();

    // The filter itself still works where the active page has rows.
    await user.click(within(nav).getByRole('button', { name: 'Appearance' }));
    expect(screen.queryByText('Interface appearance')).toBeNull();
    expect(screen.getByText(/No results for “drill”/)).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Clear search' }));
    expect(screen.getByText('Interface appearance')).toBeTruthy();
  });

  it('filters setting rows by label and hint, and shows a no-results state', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await screen.findByRole('dialog', { name: 'Preferences' });

    const search = screen.getByRole('searchbox', {
      name: 'Search settings'
    });
    expect(search.getAttribute('placeholder')).toBe('Search settings…');
    expect(screen.getByText('⌘F')).toBeTruthy();
    expect(screen.getByText('Interface appearance')).toBeTruthy();

    // Label match.
    await user.type(search, 'appearance');
    expect(screen.getByText('Interface appearance')).toBeTruthy();

    // Hint-only match: the label does not contain the query.
    await user.clear(search);
    await user.type(search, 'default');
    expect(screen.getByText('Interface appearance')).toBeTruthy();

    // Case-insensitive.
    await user.clear(search);
    await user.type(search, 'APPEARANCE');
    expect(screen.getByText('Interface appearance')).toBeTruthy();

    // Nothing matches: the row is gone and the empty state says so instead of a blank column.
    await user.clear(search);
    await user.type(search, 'drill');
    expect(screen.queryByText('Interface appearance')).toBeNull();
    expect(screen.getByText(/No results for “drill”/)).toBeTruthy();
  });

  it('restores every row when the search is cleared or emptied', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await screen.findByRole('dialog', { name: 'Preferences' });

    const search = screen.getByRole('searchbox', {
      name: 'Search settings'
    });

    await user.type(search, 'drill');
    await user.click(screen.getByRole('button', { name: 'Clear search' }));

    expect((search as HTMLInputElement).value).toBe('');
    expect(screen.queryByText(/No results/)).toBeNull();
    expect(screen.getByText('Interface appearance')).toBeTruthy();

    await user.type(search, 'drill');
    await user.clear(search);

    expect((search as HTMLInputElement).value).toBe('');
    expect(screen.getByText('Interface appearance')).toBeTruthy();
  });

  it('focuses the search field with ⌘F and Ctrl+F', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await screen.findByRole('dialog', { name: 'Preferences' });

    const search = screen.getByRole('searchbox', {
      name: 'Search settings'
    });
    expect(document.activeElement).not.toBe(search);

    await user.keyboard('[ControlLeft>]f[/ControlLeft]');
    expect(document.activeElement).toBe(search);

    // The accelerator is a focus shortcut, not a new global hotkey: typing keeps working.
    await user.keyboard('default');
    expect((search as HTMLInputElement).value).toBe('default');

    (search as HTMLInputElement).blur();
    await user.keyboard('{Meta>}f{/Meta}');
    expect(document.activeElement).toBe(search);
  });

  // --- Footer -----------------------------------------------------------------------------------

  it('renders the artboard 05 footer: Restore defaults / Import… / Export… / Close / Save changes', async () => {
    render(SettingsDialog);
    openSettings();
    const dialog = await screen.findByRole('dialog', {
      name: 'Preferences'
    });

    // Exact labels and order, as artboard 05 draws them (107 S12).
    expect(
      within(footerOf(dialog))
        .getAllByRole('button')
        .map(button => button.textContent?.trim())
    ).toEqual([
      'Restore defaults',
      'Import…',
      'Export…',
      'Close',
      'Save changes'
    ]);

    // Same design-system hierarchy as the artboard: ghost secondaries, plain Close, primary save.
    const variants = [
      'Restore defaults',
      'Import…',
      'Export…',
      'Close',
      'Save changes'
    ].map(label => footerButton(dialog, label).getAttribute('data-variant'));
    expect(variants).toEqual([
      'ghost',
      'ghost',
      'ghost',
      'secondary',
      'primary'
    ]);
  });

  it('renders Import… and Export… disabled because the library has no such commands', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    const dialog = await screen.findByRole('dialog', {
      name: 'Preferences'
    });

    for (const label of ['Import…', 'Export…']) {
      const action = footerButton(dialog, label) as HTMLButtonElement;
      expect(action.disabled).toBe(true);
      // A disabled action cannot be triggered, so it cannot claim to have done anything.
      await user.click(action);
    }

    // Only the load ran, and no status line reports an import or an export.
    expect(
      invokeMock.mock.calls.every(([command]) => command === 'get_settings')
    ).toBe(true);
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
    expect(screen.queryByText(/imported|exported/i)).toBeNull();
  });

  it('Restore defaults only resets the draft and still requires Save changes', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await waitFor(() => expect(selectedTheme()).toBe('Light'));

    await user.click(
      footerButton(
        screen.getByRole('dialog', { name: 'Preferences' }),
        'Restore defaults'
      )
    );

    expect(selectedTheme()).toBe('Dark');
    // Nothing reached the backend, and the window theme is untouched: restore is a draft edit.
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
    expect(document.documentElement.dataset.theme).toBe('light');
    // The operator is told the restore is not stored yet.
    expect(screen.getByText(/Unsaved changes/)).toBeTruthy();

    await user.click(
      footerButton(
        screen.getByRole('dialog', { name: 'Preferences' }),
        'Save changes'
      )
    );

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('update_settings', {
        settings: { theme: 'dark' }
      })
    );
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('does not preview or store a theme until Save changes is pressed', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();
    await waitFor(() => expect(selectedTheme()).toBe('Light'));

    await user.click(
      within(themeGroup()).getByRole('button', { name: 'System' })
    );

    expect(selectedTheme()).toBe('System');
    expect(screen.getByText(/Unsaved changes/)).toBeTruthy();
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
    await waitFor(() => expect(selectedTheme()).toBe('Light'));

    await user.click(
      within(themeGroup()).getByRole('button', { name: 'Dark' })
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
    await screen.findByRole('dialog', { name: 'Preferences' });

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
