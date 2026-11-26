import { render, screen, waitFor } from '@testing-library/svelte';
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
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const resetTheme = () => document.documentElement.removeAttribute('data-theme');

type BridgeHandler = (command: string, args?: unknown) => Promise<unknown>;

function bridge(handler: BridgeHandler) {
  invokeMock.mockImplementation((command: string, args?: unknown) =>
    handler(command, args)
  );
}

const echoStored: BridgeHandler = async (command, args) => {
  if (command === 'get_settings') return { theme: 'light' };
  if (command === 'update_settings') {
    const settings = (args as { settings?: { theme: string } }).settings;
    return { theme: settings?.theme ?? 'dark' };
  }
  throw new Error(`no bridge for ${command}`);
};

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
    const theme = (await screen.findByLabelText('Theme')) as HTMLSelectElement;

    await waitFor(() => expect(theme.value).toBe('light'));
    expect(invokeMock).toHaveBeenCalledWith('get_settings');
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(dialog.textContent).toContain('Apariencia');
  });

  it('persists the chosen theme through update_settings and closes', async () => {
    const user = userEvent.setup();
    render(SettingsDialog);
    openSettings();

    const theme = (await screen.findByLabelText('Theme')) as HTMLSelectElement;
    await waitFor(() => expect(theme.value).toBe('light'));

    await user.selectOptions(theme, 'dark');
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

    await user.click(screen.getByRole('button', { name: 'Cancelar' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
  });

  it('shows the backend error instead of pretending the settings were read', async () => {
    bridge(async command => {
      if (command === 'get_settings') throw 'state poisoned';
      throw new Error(`no bridge for ${command}`);
    });
    render(SettingsDialog);

    openSettings();

    expect(await screen.findByText('state poisoned')).toBeTruthy();
    expect((screen.getByLabelText('Theme') as HTMLSelectElement).value).toBe(
      'dark'
    );
  });
});
