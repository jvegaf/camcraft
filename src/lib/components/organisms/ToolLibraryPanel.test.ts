import { act, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { closeSettings, openSettings } from '../workbench/workbench.svelte';
import SettingsDialog from './SettingsDialog.svelte';
import ToolLibraryPanel from './ToolLibraryPanel.svelte';

// Wiring contract (107 S9, S10, S11, S13 + 096 draft semantics): the panel is the only place that
// calls the CAM library commands, and it composes the existing tree, detail and preview organisms
// into `ToolLibraryPage`'s three snippet props.
//
// The Tauri bridge is mocked at `@tauri-apps/api/core` — the module both this panel and the settings
// dialog import — with the same mechanism `SettingsDialog.test.ts` uses. The payloads below are the
// commands' JSON, key for key.
const tree = {
  schema: 'tool-tree@1',
  groups: [
    {
      kind: 'flat-end-mill',
      label: 'Flat end mill',
      count: 2,
      tools: [
        { id: 'flat-1', number: 1, name: 'Flat 1.0 mm', meta: '1F' },
        { id: 'flat-22', number: 22, name: 'End mill 1.0 mm', meta: '1F' }
      ]
    },
    {
      kind: 'drill',
      label: 'Drill',
      count: 1,
      tools: [{ id: 'drill-4', number: 4, name: 'Drill 0.8 mm', meta: '118°' }]
    }
  ]
};

const drillDetail = {
  schema: 'tool-detail@1',
  id: 'drill-4',
  name: 'Drill 0.8 mm',
  kind: 'drill',
  kindLabel: 'Drill',
  number: 4,
  diameterMm: 0.8,
  flutes: 2,
  cutLengthMm: 8.0,
  shankMm: 3.175,
  overallLengthMm: 38.0,
  includedAngleDeg: 118,
  flatDiameterMm: null,
  spindleRpm: 12000,
  cutFeedMmPerMin: 400,
  plungeFeedMmPerMin: 100,
  stepoverMm: null,
  stepoverPercent: null
};

const detail = {
  schema: 'tool-detail@1',
  id: 'flat-22',
  name: 'End mill 1.0 mm',
  kind: 'flat-end-mill',
  kindLabel: 'Flat end mill',
  number: 22,
  diameterMm: 1.0,
  flutes: 1,
  cutLengthMm: 8.0,
  shankMm: 3.175,
  overallLengthMm: 38.0,
  includedAngleDeg: null,
  flatDiameterMm: null,
  spindleRpm: 18000,
  cutFeedMmPerMin: 800,
  plungeFeedMmPerMin: 200,
  stepoverMm: 0.4,
  stepoverPercent: null
};

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

type BridgeHandler = (command: string, args?: unknown) => Promise<unknown>;

function bridge(handler: BridgeHandler) {
  invokeMock.mockImplementation((command: string, args?: unknown) =>
    handler(command, args)
  );
}

type SavedTool = {
  id: string;
  name: string;
  number: number | null;
  flutes: number | null;
};

/**
 * A mutable stand-in for the store: `tool_library_save` writes into `stored`, and `tool_detail`
 * reads it back, so a re-read after a save observes what the store holds rather than what the panel
 * typed.
 */
function libraryBridge() {
  let stored: Record<string, unknown> = { ...detail };
  const handler: BridgeHandler = async (command, args) => {
    if (command === 'tool_library_tree') return tree;
    if (command === 'tool_detail') {
      const { id } = args as { id: string };
      return stored.id === id ? { ...stored } : null;
    }
    if (command === 'tool_library_save') {
      const { request } = args as { request: SavedTool };
      // What the store would hold after its own write: deliberately not the typed string, so a
      // panel that skips the re-read keeps showing the draft instead of the stored record.
      stored = {
        ...stored,
        ...request,
        name: `${request.name} (stored)`
      };
      return { ...stored };
    }
    if (command === 'get_settings') return { theme: 'dark' };
    if (command === 'update_settings')
      return (args as { settings: unknown }).settings;
    throw new Error(`no bridge for ${command}`);
  };
  return { handler, stored: () => stored };
}

const rowOf = (text: RegExp) => screen.getByRole('button', { name: text });

async function openToolLibrary() {
  const user = userEvent.setup();
  render(SettingsDialog);
  openSettings();
  const nav = await screen.findByRole('navigation', {
    name: 'Preferences pages'
  });
  await user.click(within(nav).getByRole('button', { name: 'Tool library' }));
  return user;
}

async function selectFlat22(user: ReturnType<typeof userEvent.setup>) {
  await screen.findByRole('heading', { level: 5, name: /^End mills\b/ });
  await user.click(rowOf(/T22\b/));
  await screen.findByRole('heading', {
    level: 4,
    name: 'End mill 1.0 mm'
  });
}

describe('ToolLibraryPanel', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    closeSettings();
    document.documentElement.removeAttribute('data-theme');
    bridge(libraryBridge().handler);
  });

  it('loads the tree on mount and renders the groups the command returned', async () => {
    render(ToolLibraryPanel);

    await screen.findByRole('heading', { level: 5, name: /^End mills\b/ });

    expect(invokeMock).toHaveBeenCalledWith('tool_library_tree');
    expect(
      screen.getByRole('heading', { level: 5, name: /^Drills\b/ })
    ).toBeTruthy();
    expect(rowOf(/T1\b/)).toBeTruthy();
    expect(rowOf(/T4\b/)).toBeTruthy();
  });

  it('loads the selected tool detail through tool_detail', async () => {
    const user = userEvent.setup();
    render(ToolLibraryPanel);
    await screen.findByRole('heading', { level: 5, name: /^End mills\b/ });

    await user.click(rowOf(/T22\b/));

    expect(
      await screen.findByRole('heading', {
        level: 4,
        name: 'End mill 1.0 mm'
      })
    ).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith('tool_detail', { id: 'flat-22' });
    expect(rowOf(/T22\b/).getAttribute('aria-current')).toBe('true');
  });

  it('renders the empty state instead of sample rows when the library holds nothing', async () => {
    bridge(async command => {
      if (command === 'tool_library_tree')
        return { schema: 'tool-tree@1', groups: [] };
      throw new Error(`no bridge for ${command}`);
    });

    render(ToolLibraryPanel);

    expect(
      await screen.findByText('No tools in the library yet.')
    ).toBeTruthy();
    expect(screen.queryByText('Flat 1.0 mm')).toBeNull();
    expect(screen.queryByText('Drill 0.8 mm')).toBeNull();
  });

  it('creates a new tool from + New, selects it and refreshes the tree', async () => {
    // The command's own contract (107 T2): the store decides the id, number, shape and flutes, so the
    // only thing the page sends is a name.
    const created = {
      ...detail,
      id: 'tool-1',
      name: 'New tool',
      number: 23,
      flutes: 2
    };
    const grownTree = {
      schema: 'tool-tree@1',
      groups: [
        {
          kind: 'flat-end-mill',
          label: 'Flat end mill',
          count: 3,
          tools: [
            ...tree.groups[0].tools,
            { id: 'tool-1', number: 23, name: 'New tool', meta: '2F' }
          ]
        },
        ...tree.groups.slice(1)
      ]
    };
    let treeReads = 0;
    bridge(async (command, args) => {
      if (command === 'tool_library_tree') {
        treeReads += 1;
        return treeReads === 1 ? tree : grownTree;
      }
      if (command === 'tool_detail') {
        const { id } = args as { id: string };
        return id === created.id ? { ...created } : null;
      }
      if (command === 'tool_library_create') return { ...created };
      throw new Error(`no bridge for ${command}`);
    });

    const user = userEvent.setup();
    render(ToolLibraryPanel);
    await screen.findByRole('heading', { level: 5, name: /^End mills\b/ });

    await user.click(screen.getByRole('button', { name: '+ New' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('tool_library_create', {
        request: { name: 'New tool' }
      })
    );

    // The new tool becomes the selection, so its detail form is where the operator names it…
    expect(
      await screen.findByRole('heading', { level: 4, name: 'New tool' })
    ).toBeTruthy();
    // …and the left column shows the row the store created, not a stale tree.
    expect(rowOf(/T23\b/).getAttribute('aria-current')).toBe('true');
  });

  it('claims Valid only while a selected tool has a non-empty name', async () => {
    const user = userEvent.setup();
    render(ToolLibraryPanel);
    await screen.findByRole('heading', { level: 5, name: /^End mills\b/ });

    // Nothing is selected yet: the header must not advertise a validity nobody checked.
    expect(screen.queryByText('Valid')).toBeNull();

    await user.click(rowOf(/T22\b/));
    await screen.findByRole('heading', {
      level: 4,
      name: 'End mill 1.0 mm'
    });
    expect(screen.getByText('Valid')).toBeTruthy();

    // A blank drafted name is not a valid tool, so the badge stops claiming it.
    const name = screen.getByRole('textbox', { name: 'Name' });
    await user.clear(name);
    expect(screen.queryByText('Valid')).toBeNull();

    await user.type(name, 'End mill 1.0 mm');
    expect(screen.getByText('Valid')).toBeTruthy();
  });

  it('keeps an edit in the draft until the dialog confirms, then saves exactly the allowed keys', async () => {
    const user = await openToolLibrary();
    await selectFlat22(user);

    const name = screen.getByRole('textbox', { name: 'Name' });
    const number = screen.getByRole('spinbutton', { name: 'Number' });
    const flutes = screen.getByRole('spinbutton', { name: 'Flutes' });

    await user.clear(name);
    await user.type(name, 'End mill 2.0 mm');
    await user.clear(number);
    await user.type(number, '23');
    await user.clear(flutes);
    await user.type(flutes, '2');

    // Editing only touches the dialog's draft: no save command has run.
    expect(invokeMock).not.toHaveBeenCalledWith(
      'tool_library_save',
      expect.anything()
    );

    await user.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('tool_library_save', {
        request: {
          id: 'flat-22',
          name: 'End mill 2.0 mm',
          number: 23,
          flutes: 2
        }
      })
    );
  });

  it('re-reads the detail after a successful save so it shows what the store holds', async () => {
    const user = userEvent.setup();
    const result = render(ToolLibraryPanel);
    await screen.findByRole('heading', { level: 5, name: /^End mills\b/ });
    await user.click(rowOf(/T22\b/));
    await screen.findByRole('heading', {
      level: 4,
      name: 'End mill 1.0 mm'
    });

    const name = screen.getByRole('textbox', { name: 'Name' });
    await user.clear(name);
    await user.type(name, 'Typed by the operator');

    await act(async () => {
      await result.component.save();
    });

    // The store wrote `Typed by the operator (stored)`; only a detail re-read can display that.
    expect((name as HTMLInputElement).value).toBe(
      'Typed by the operator (stored)'
    );
    const detailCalls = invokeMock.mock.calls.filter(
      call => call[0] === 'tool_detail'
    );
    expect(detailCalls.length).toBeGreaterThanOrEqual(2);
  });

  it('keeps an unconfirmed library edit when the operator navigates to another preferences page', async () => {
    const user = await openToolLibrary();
    await selectFlat22(user);

    const name = screen.getByRole('textbox', { name: 'Name' });
    await user.clear(name);
    await user.type(name, 'End mill kept across pages');

    const nav = screen.getByRole('navigation', {
      name: 'Preferences pages'
    });
    await user.click(within(nav).getByRole('button', { name: 'Appearance' }));

    // Coming back shows the same unconfirmed draft: navigation did not destroy it.
    await user.click(within(nav).getByRole('button', { name: 'Tool library' }));
    expect(
      (screen.getByRole('textbox', { name: 'Name' }) as HTMLInputElement).value
    ).toBe('End mill kept across pages');

    // Confirming from another page still reaches the panel's draft and saves it.
    await user.click(within(nav).getByRole('button', { name: 'Appearance' }));
    await user.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('tool_library_save', {
        request: {
          id: 'flat-22',
          name: 'End mill kept across pages',
          number: 22,
          flutes: 1
        }
      })
    );
  });

  it('discards a post-save re-read that arrives after another tool was selected', async () => {
    let flatDetailCalls = 0;
    let resolveReread!: (value: unknown) => void;
    const reread = new Promise(resolve => {
      resolveReread = resolve;
    });

    bridge(async (command, args) => {
      if (command === 'tool_library_tree') return tree;
      if (command === 'tool_detail') {
        const { id } = args as { id: string };
        if (id === 'flat-22') {
          flatDetailCalls += 1;
          // The first read is the selection; the next one is the save's re-read, held open so the
          // operator can select another tool while it is in flight.
          return flatDetailCalls > 1 ? reread : { ...detail };
        }
        if (id === 'drill-4') return { ...drillDetail };
        return null;
      }
      if (command === 'tool_library_save') return { ...detail };
      throw new Error(`no bridge for ${command}`);
    });

    const user = userEvent.setup();
    const result = render(ToolLibraryPanel);
    await screen.findByRole('heading', { level: 5, name: /^End mills\b/ });
    await user.click(rowOf(/T22\b/));
    await screen.findByRole('heading', {
      level: 4,
      name: 'End mill 1.0 mm'
    });

    const flatName = screen.getByRole('textbox', { name: 'Name' });
    await user.clear(flatName);
    await user.type(flatName, 'Draft on the end mill');

    const saving = result.component.save();
    await waitFor(() => expect(flatDetailCalls).toBe(2));

    // A newer selection lands while the save's re-read is still pending.
    await user.click(rowOf(/T4\b/));
    await screen.findByRole('heading', { level: 4, name: 'Drill 0.8 mm' });
    const drillName = screen.getByRole('textbox', { name: 'Name' });
    await user.clear(drillName);
    await user.type(drillName, 'Draft on the drill');

    resolveReread({ ...detail, name: 'End mill 1.0 mm (stored)' });
    await act(async () => {
      await saving;
    });

    // The late response belongs to the previous tool, so it cannot paint over the drill.
    expect(
      screen.getByRole('heading', { level: 4, name: 'Draft on the drill' })
    ).toBeTruthy();
    expect(
      (screen.getByRole('textbox', { name: 'Name' }) as HTMLInputElement).value
    ).toBe('Draft on the drill');
  });

  it('surfaces a structured save error and leaves the dialog open without a false success', async () => {
    bridge(async (command, args) => {
      if (command === 'tool_library_tree') return tree;
      if (command === 'tool_detail') return { ...detail };
      if (command === 'tool_library_save') {
        throw {
          code: 'tool_library_unwritable',
          message: 'the library file is read-only'
        };
      }
      if (command === 'get_settings') return { theme: 'dark' };
      if (command === 'update_settings') {
        return (args as { settings: unknown }).settings;
      }
      throw new Error(`no bridge for ${command}`);
    });

    const user = await openToolLibrary();
    await selectFlat22(user);

    const name = screen.getByRole('textbox', { name: 'Name' });
    await user.clear(name);
    await user.type(name, 'End mill 2.0 mm');
    await user.click(screen.getByRole('button', { name: 'Save changes' }));

    const error = await screen.findByText(
      'tool_library_unwritable: the library file is read-only'
    );
    expect(error.getAttribute('aria-live')).toBe('assertive');
    expect(screen.getByRole('dialog', { name: 'Preferences' })).toBeTruthy();
    // The library write failed, so the dialog must not go on to confirm a theme save or close.
    expect(invokeMock).not.toHaveBeenCalledWith(
      'update_settings',
      expect.anything()
    );
  });
});

describe('ToolLibraryPanel tree refresh after save', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    closeSettings();
    document.documentElement.removeAttribute('data-theme');
  });

  it('reloads the tree after a successful save so the edited row matches the store', async () => {
    let stored: Record<string, unknown> = { ...detail };
    const currentTree = () => ({
      schema: 'tool-tree@1',
      groups: [
        {
          kind: 'flat-end-mill',
          label: 'Flat end mill',
          count: 1,
          tools: [
            {
              id: 'flat-22',
              number: stored.number,
              name: stored.name,
              meta: '1F'
            }
          ]
        }
      ]
    });
    bridge(async (command, args) => {
      if (command === 'tool_library_tree') return currentTree();
      if (command === 'tool_detail') {
        const { id } = args as { id: string };
        return stored.id === id ? { ...stored } : null;
      }
      if (command === 'tool_library_save') {
        const { request } = args as { request: SavedTool };
        stored = { ...stored, ...request };
        return { ...stored };
      }
      throw new Error(`no bridge for ${command}`);
    });

    const user = userEvent.setup();
    const result = render(ToolLibraryPanel);
    await screen.findByRole('heading', { level: 5, name: /^End mills\b/ });
    await user.click(rowOf(/T22\b/));
    await screen.findByRole('heading', {
      level: 4,
      name: 'End mill 1.0 mm'
    });

    const name = screen.getByRole('textbox', { name: 'Name' });
    await user.clear(name);
    await user.type(name, 'Renamed by the operator');

    await act(async () => {
      await result.component.save();
    });

    expect(
      await screen.findByRole('button', { name: /Renamed by the operator/ })
    ).toBeTruthy();
    expect(
      screen.queryByRole('button', { name: /End mill 1.0 mm/ })
    ).toBeNull();
  });
});
