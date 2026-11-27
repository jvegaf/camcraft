import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsDialog } from '$lib/components/organisms';
import {
  closeSettings,
  workbenchState
} from '$lib/components/workbench/workbench.svelte';
import Page from './+page.svelte';

// Route test for the start screen (`src/routes/+page.svelte`, 095 T5 follow-up, S8 + risk item 6).
// The page owns the file actions the shared `TopBar` and `StartScreen` render, so the assertions go
// through the DOM: the actions are there and each one reaches the command the page wires to it. The
// Tauri bridge and the navigation are mocked at their module boundaries.
const { invokeMock, gotoMock, openMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  gotoMock: vi.fn(),
  openMock: vi.fn()
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));
vi.mock('$app/navigation', () => ({ goto: gotoMock }));

const recent = {
  name: 'controller-board.cmc',
  path: '/tmp/controller-board.cmc',
  lastOpened: '2026-11-26T00:00:00Z'
};

describe('start screen route', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    gotoMock.mockReset();
    openMock.mockReset();
    closeSettings();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'list_recents') return Promise.resolve([recent]);
      if (command === 'open_document' || command === 'new_document') {
        return Promise.resolve({ dirty: false, path: '/tmp/x.cmc' });
      }
      if (command === 'get_settings') return Promise.resolve({ theme: 'dark' });
      if (command === 'update_settings')
        return Promise.resolve({ theme: 'dark' });
      return Promise.reject(new Error(`no bridge for ${command}`));
    });
  });

  afterEach(() => closeSettings());

  it('renders the start screen actions inside the shared chrome', async () => {
    render(Page);

    expect(
      screen.getByRole('button', { name: /New PCB project/ })
    ).toBeTruthy();
    expect(screen.getByRole('button', { name: /Open project/ })).toBeTruthy();
    expect(screen.getByLabelText('Status bar')).toBeTruthy();
    const banner = screen.getByRole('banner');
    expect(
      within(banner).getByRole('button', { name: 'Current project' })
        .textContent
    ).toContain('Camcraft');

    // The recents the page loads reach the Recientes panel.
    expect(await screen.findByText(recent.name)).toBeTruthy();
  });

  it('reopens a recent through open_document and leaves for the workspace', async () => {
    const user = userEvent.setup();
    render(Page);

    await user.click(await screen.findByText(recent.name));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('open_document', {
        path: recent.path
      })
    );
    await waitFor(() => expect(gotoMock).toHaveBeenCalledWith('/workspace'));
  });

  it('creates a document through the New action', async () => {
    const user = userEvent.setup();
    render(Page);

    await user.click(screen.getByRole('button', { name: /New PCB project/ }));
    await user.click(screen.getByRole('button', { name: 'Create' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('new_document', {
        request: { widthMm: 1219, heightMm: 2438, thicknessMm: 18 }
      })
    );
    await waitFor(() => expect(gotoMock).toHaveBeenCalledWith('/workspace'));
  });

  it('opens a document through the Open action', async () => {
    openMock.mockResolvedValue('/tmp/otro.cmc');
    const user = userEvent.setup();
    render(Page);

    await user.click(screen.getByRole('button', { name: /Open project/ }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('open_document', {
        path: '/tmp/otro.cmc'
      })
    );
    await waitFor(() => expect(gotoMock).toHaveBeenCalledWith('/workspace'));
  });

  // `+layout.svelte` mounts the settings dialog once for every route, so the route test mounts the
  // same pair the layout does.
  it('opens the settings surface from the shared chrome', async () => {
    const user = userEvent.setup();
    render(Page);
    render(SettingsDialog);

    await user.click(
      within(screen.getByRole('banner')).getByRole('button', {
        name: 'Settings'
      })
    );

    await waitFor(() => expect(workbenchState.settingsOpen).toBe(true));
    const dialog = await screen.findByRole('dialog', { name: 'Preferences' });
    expect(
      within(dialog).getByRole('heading', { name: 'Appearance' })
    ).toBeTruthy();
  });
});
