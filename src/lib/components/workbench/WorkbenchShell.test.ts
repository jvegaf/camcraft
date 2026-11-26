import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import WorkbenchShell from './WorkbenchShell.svelte';
import { setDocument, setStatus, workbenchState } from './workbench.svelte';

// Shell contract (095 T2, S8): the organism keeps every Tauri handler it has today and mounts the
// regions through `WorkbenchTemplate`. The bridge is mocked at `@tauri-apps/api/core` — the module
// the shell and `workbench.svelte.ts` both import — so the assertions are about the rendered DOM
// and the commands that reach the bridge, not internal component state.
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

const documentStatus = {
  schema: 'camcraft.document',
  documentSchema: 'camcraft.document/1',
  dirty: false,
  path: '/tmp/controller-board.cmc',
  layerCount: 4,
  toolpathCount: 3,
  boardWidthMm: 100,
  boardHeightMm: 80,
  boardThicknessMm: 1.6
};

const recents = [
  {
    name: 'controller-board.cmc',
    path: '/tmp/controller-board.cmc',
    lastOpened: '2026-11-26T00:00:00Z'
  }
];

describe('WorkbenchShell', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((command: string) => {
      switch (command) {
        case 'document_status':
          return Promise.resolve(documentStatus);
        case 'list_recents':
          return Promise.resolve(recents);
        case 'open_document':
          return Promise.resolve(documentStatus);
        case 'new_document':
          return Promise.resolve(documentStatus);
        case 'save_document':
          return Promise.resolve(documentStatus);
        default:
          // `scene_snapshot` lands here: jsdom has no WebGL, so `BoardCanvas` stops at its
          // snapshot guard instead of creating a renderer. The shell must render without it.
          return Promise.reject(new Error(`no bridge for ${command}`));
      }
    });
    setDocument(null);
    setStatus('Listo', 'info');
    workbenchState.recents = [];
  });

  it('renders the frame and every region the artboard names', () => {
    const { container } = render(WorkbenchShell, {});

    expect(container.querySelector('[data-region="frame"]')).not.toBeNull();
    expect(container.querySelector('[data-region="backdrop"]')).not.toBeNull();
    expect(screen.getByLabelText('Acciones rápidas')).toBeTruthy();
    expect(screen.getByLabelText('Estado de viewport')).toBeTruthy();
    expect(screen.getByLabelText('Browser')).toBeTruthy();
    expect(screen.getByLabelText('Inspector')).toBeTruthy();
    expect(screen.getByLabelText('Barra de estado')).toBeTruthy();
    expect(screen.getByText('Camcraft')).toBeTruthy();
  });

  it('asks the bridge for document status and recents on mount', async () => {
    const { container } = render(WorkbenchShell, {});

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('document_status'));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('list_recents'));
    // The mocked status reaches the top bar through the existing store wiring.
    const topBar = container.querySelector('header') as HTMLElement;
    await waitFor(() =>
      expect(within(topBar).getByText(/controller-board\.cmc/)).toBeTruthy()
    );
  });

  it('still opens a document through the top bar action', async () => {
    const dialog = await import('@tauri-apps/plugin-dialog');
    vi.mocked(dialog.open).mockResolvedValue('/tmp/otro.cmc');
    const user = userEvent.setup();
    render(WorkbenchShell, {});

    const [openButton] = screen.getAllByRole('button', { name: 'Abrir' });
    await user.click(openButton);

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('open_document', {
        path: '/tmp/otro.cmc'
      })
    );
    await waitFor(() =>
      expect(
        within(screen.getByLabelText('Barra de estado')).getByText(
          /Documento abierto/
        )
      ).toBeTruthy()
    );
  });

  // 095 T3: the file actions became icon actions. Their accessible names are the contract, and each
  // one must still reach the same Tauri command it reached as a text button (S8).
  it('wires every named file action to its Tauri command', async () => {
    const dialog = await import('@tauri-apps/plugin-dialog');
    vi.mocked(dialog.open).mockResolvedValue('/tmp/otro.cmc');
    vi.mocked(dialog.save).mockResolvedValue('/tmp/copiado.cmc');
    const user = userEvent.setup();
    render(WorkbenchShell, {});

    for (const name of ['Nuevo', 'Abrir', 'Guardar', 'Guardar como']) {
      expect(screen.getByRole('button', { name })).toBeTruthy();
    }

    await user.click(screen.getByRole('button', { name: 'Nuevo' }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('new_document', {
        request: { width: 100, height: 100, thickness: 1.6 }
      })
    );

    await user.click(screen.getByRole('button', { name: 'Guardar' }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('save_document', {
        path: documentStatus.path
      })
    );

    await user.click(screen.getByRole('button', { name: 'Guardar como' }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('save_document', {
        path: '/tmp/copiado.cmc'
      })
    );
  });

  // 095 T5 (L8e): the recents the shell already loads through `list_recents` must reach the
  // Browser again, and activating one reopens it through the same `open_document` command.
  it('lists the recents it loaded and reopens one through the bridge', async () => {
    const user = userEvent.setup();
    render(WorkbenchShell, {});

    const browser = screen.getByLabelText('Browser');
    const row = await within(browser).findByRole('button', {
      name: recents[0].name
    });

    await user.click(row);

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('open_document', {
        path: recents[0].path
      })
    );
    await waitFor(() =>
      expect(
        within(screen.getByLabelText('Barra de estado')).getByText(
          /Documento abierto/
        )
      ).toBeTruthy()
    );
  });

  // 095 T3 item 4/5: the HUD and status bar read real state only.
  it('renders the artboard chrome labels without inventing coordinates', () => {
    render(WorkbenchShell, {});

    const hud = screen.getByLabelText('Estado de viewport');
    expect(hud.textContent).toContain('2D Board');
    expect(hud.textContent).toContain(' · Grid 1 mm');
    expect(screen.getByText('X —')).toBeTruthy();
    expect(screen.getByText('mm · Grid 1 mm · GRBL')).toBeTruthy();
  });
});
