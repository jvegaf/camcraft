import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import ToolLibraryTree from './ToolLibraryTree.svelte';

// Organism contract (107 T5, S9 + S13): the tools column of
// `design/camcraft-workbench-v2/05-settings-tool-library.html`, fed by the `tool_library_tree`
// command's payload (`src-tauri/src/lib.rs:744-861`). The payload below is that command's JSON,
// key for key: `schema`, then `groups` of `kind`/`label`/`count`/`tools`, and each row of
// `id`/`number`/`name`/`meta`. The group `label` is the backend's **singular** word (`Flat end
// mill`), so the plural headings S9 shows (`End mills`) are this organism's copy — and the component
// takes props instead of calling the command, so the tests hand it exactly that JSON.
const tree = {
  schema: 'tool-tree@1',
  groups: [
    {
      kind: 'flat-end-mill',
      label: 'Flat end mill',
      count: 4,
      tools: [
        { id: 'flat-1', number: 1, name: 'Flat 1.0 mm', meta: '1F' },
        { id: 'flat-2', number: 2, name: 'Flat 2.0 mm', meta: '2F' },
        { id: 'flat-3', number: 3, name: 'Flat 3.175 mm', meta: '2F' },
        { id: 'flat-22', number: 22, name: 'End mill 1.0 mm', meta: '1F' }
      ]
    },
    {
      kind: 'drill',
      label: 'Drill',
      count: 2,
      tools: [
        { id: 'drill-4', number: 4, name: 'Drill 0.8 mm', meta: '118°' },
        { id: 'drill-5', number: 5, name: 'Drill 1.0 mm', meta: '118°' }
      ]
    },
    {
      kind: 'v-bit',
      label: 'V-bit',
      count: 1,
      tools: [{ id: 'v-bit-6', number: 6, name: 'V-bit 30°', meta: '0.1' }]
    }
  ]
};

/** The row text with the artboard's spacing flattened, so a whole row can be asserted at once. */
const textOf = (element: HTMLElement) =>
  element.textContent?.replace(/\s+/g, ' ').trim() ?? '';

/** The group `<section>` whose heading starts with `heading` (the heading also carries the count). */
const groupOf = (heading: RegExp): HTMLElement =>
  screen
    .getByRole('heading', { level: 5, name: heading })
    .closest('section') as HTMLElement;

describe('ToolLibraryTree', () => {
  it('renders the three groups with their counts and English plural headings', () => {
    render(ToolLibraryTree, { groups: tree.groups });

    // S9: `End mills` 4, `Drills` 2, `V-bits` 1. The backend publishes the singular labels
    // (`Flat end mill`, `Drill`, `V-bit`), so the plurals are the component's own copy.
    for (const [heading, count] of [
      [/^End mills\b/, '4'],
      [/^Drills\b/, '2'],
      [/^V-bits\b/, '1']
    ] as const) {
      const group = groupOf(heading);
      expect(within(group).getByText(count)).toBeTruthy();
    }

    expect(screen.getAllByRole('heading', { level: 5 })).toHaveLength(3);
  });

  it('renders a row with its number, name and kind-dependent meta', () => {
    render(ToolLibraryTree, { groups: tree.groups });

    const endMills = groupOf(/^End mills\b/);
    // T1 `Flat 1.0 mm` `1F` — the whole row, in order, is the contract.
    expect(textOf(within(endMills).getByRole('button', { name: /T1\b/ }))).toBe(
      'T1 Flat 1.0 mm 1F'
    );
    // The meta differs by kind: an included angle for a drill, the tip flat for a V-bit.
    expect(
      textOf(within(groupOf(/^Drills\b/)).getByRole('button', { name: /T4\b/ }))
    ).toBe('T4 Drill 0.8 mm 118°');
    expect(
      textOf(within(groupOf(/^V-bits\b/)).getByRole('button', { name: /T6\b/ }))
    ).toBe('T6 V-bit 30° 0.1');
  });

  it('renders the dash for a tool the library holds without a number', () => {
    // The command publishes `number: null` and `meta: "—"` for such a tool
    // (`src-tauri/src/lib.rs:744-757`). The row shows the dash the payload carries: never `T0`,
    // never `Tnull`, and never an empty number cell.
    render(ToolLibraryTree, {
      groups: [
        {
          kind: 'flat-end-mill',
          label: 'Flat end mill',
          count: 1,
          tools: [
            { id: 'flat-none', number: null, name: 'Flat 1.0 mm', meta: '—' }
          ]
        }
      ]
    });

    const row = screen.getByRole('button');
    expect(textOf(row)).toBe('— Flat 1.0 mm —');
    expect(screen.queryByText(/T0\b/)).toBeNull();
    expect(screen.queryByText(/Tnull/)).toBeNull();
  });

  it('falls back to the backend label for a kind it has no heading for', () => {
    // Pluralising is a closed table of the kinds the store publishes, not `label + 's'`: an unknown
    // slug keeps the backend's own singular word instead of inventing a plural.
    render(ToolLibraryTree, {
      groups: [{ kind: 'laser', label: 'Laser', count: 1, tools: [] }]
    });

    expect(
      screen.getByRole('heading', { level: 5, name: /^Laser\b/ })
    ).toBeTruthy();
    expect(
      screen.queryByRole('heading', { level: 5, name: /^Lasers\b/ })
    ).toBeNull();
  });

  it('marks exactly the selected row with aria-current="true"', () => {
    render(ToolLibraryTree, { groups: tree.groups, selectedId: 'flat-22' });

    const current = screen
      .getAllByRole('button')
      .filter(row => row.getAttribute('aria-current') === 'true');

    expect(current).toHaveLength(1);
    expect(textOf(current[0])).toContain('T22');
    // The other rows keep no `aria-current` at all, so selection is never colour-only.
    expect(
      within(groupOf(/^Drills\b/))
        .getByRole('button', { name: /T4\b/ })
        .getAttribute('aria-current')
    ).toBeNull();
  });

  it('emits the clicked row id through its callback', async () => {
    const onselect = vi.fn();
    const user = userEvent.setup();
    render(ToolLibraryTree, {
      groups: tree.groups,
      selectedId: 'flat-22',
      onselect
    });

    await user.click(
      within(groupOf(/^Drills\b/)).getByRole('button', { name: /T5\b/ })
    );

    expect(onselect).toHaveBeenCalledTimes(1);
    expect(onselect).toHaveBeenCalledWith('drill-5');
  });

  it('reaches the rows by keyboard and activates them with Enter', async () => {
    const onselect = vi.fn();
    const user = userEvent.setup();
    render(ToolLibraryTree, { groups: tree.groups, onselect });

    // Every row is a real button, so the row itself is the hit target and the tab stop.
    await user.tab();
    const first = screen.getAllByRole('button')[0];
    expect(first.textContent).toContain('Flat 1.0 mm');
    expect(first.contains(document.activeElement)).toBe(true);

    await user.keyboard('{Enter}');
    expect(onselect).toHaveBeenCalledWith('flat-1');
  });
});
