import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import ToolLibraryDetail from './ToolLibraryDetail.svelte';

// Organism contract (107 T6, S10 + S14): the `Detail` column of
// `design/camcraft-workbench-v2/05-settings-tool-library.html`, fed by the `tool_detail` command's
// payload (`src-tauri/src/lib.rs:886-923`). The payload below is that command's JSON, key for key:
// `schema`, `id`, `name`, `kind`, `kindLabel`, then the geometry (`number`, `diameterMm`, `flutes`,
// `cutLengthMm`, `shankMm`, `overallLengthMm`, `includedAngleDeg`, `flatDiameterMm`), the feeds
// (`spindleRpm`, `cutFeedMmPerMin`, `plungeFeedMmPerMin`, `stepoverMm`) and `stepoverPercent`, which
// the model does not hold and T1 publishes as `null`.
//
// What is editable is read off the artboard's own markup: its `<input>` rows are `Name`, `Number` and
// `Flutes`; every other row is a `.ro` text row. That selection is not a UI preference — the write
// command's payload (107 T2, `ToolSaveRequest`) accepts exactly `name`, `number` and `flutes` and
// denies unknown fields, so a wider form would offer edits the backend rejects. The component is
// prop-driven and reports edits through `onedit`; it never calls a command and writes nothing.
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

/** S10's labels, in the order the artboard lists them, split by section. */
const GEOMETRY_LABELS = [
  'Name',
  'Type',
  'Number',
  'Diameter',
  'Flutes',
  'Cut length',
  'Shank',
  'Overall length'
] as const;
const FEEDS_LABELS = [
  'Spindle',
  'Cut feed',
  'Feed plunge',
  'Stepover'
] as const;

/** The fields the artboard renders as `.ro` text rather than as an `<input>`. */
const READ_ONLY_FIELDS = [
  'type',
  'diameter',
  'cut-length',
  'shank',
  'overall-length',
  'spindle',
  'cut-feed',
  'feed-plunge',
  'stepover'
] as const;

const fieldOf = (container: HTMLElement, field: string): HTMLElement => {
  const element = container.querySelector(`[data-field="${field}"]`);
  if (!(element instanceof HTMLElement)) {
    throw new Error(`no field rendered for ${field}`);
  }
  return element;
};

describe('ToolLibraryDetail', () => {
  it('renders the tool title, its number badge and both S10 section headings', () => {
    render(ToolLibraryDetail, { detail });

    // S10's title is the tool's name, and the badge carries the program number in `T` notation.
    expect(
      screen.getByRole('heading', { level: 4, name: 'End mill 1.0 mm' })
    ).toBeTruthy();
    expect(screen.getByText('T22')).toBeTruthy();

    // Real headings, not styled spans: the two sections can be navigated by heading.
    expect(
      screen.getByRole('heading', { level: 5, name: 'Geometry' })
    ).toBeTruthy();
    expect(
      screen.getByRole('heading', { level: 5, name: 'Feeds & speeds' })
    ).toBeTruthy();
  });

  it('renders every S10 field label in both sections', () => {
    render(ToolLibraryDetail, { detail });

    for (const label of [...GEOMETRY_LABELS, ...FEEDS_LABELS]) {
      expect(screen.getByText(label)).toBeTruthy();
    }
  });

  it('renders only Name, Number and Flutes as inputs, with the values and units of S10', () => {
    const { container } = render(ToolLibraryDetail, { detail });

    // The artboard's three `<input>` rows, and only those, are editable.
    expect(container.querySelectorAll('input')).toHaveLength(3);
    const name = screen.getByRole('textbox', { name: 'Name' });
    expect((name as HTMLInputElement).value).toBe('End mill 1.0 mm');
    expect(
      (screen.getByRole('spinbutton', { name: 'Number' }) as HTMLInputElement)
        .value
    ).toBe('22');
    expect(
      (screen.getByRole('spinbutton', { name: 'Flutes' }) as HTMLInputElement)
        .value
    ).toBe('1');

    // Every `.ro` row is text, not a disabled input: the values are readable and copyable.
    for (const field of READ_ONLY_FIELDS) {
      expect(fieldOf(container, field).querySelector('input')).toBeNull();
    }

    // Lengths in `mm`, spindle in `rpm`, feeds in `mm/min`, stepover in `%`.
    expect(
      within(fieldOf(container, 'diameter')).getByText('1.0')
    ).toBeTruthy();
    expect(within(fieldOf(container, 'diameter')).getByText('mm')).toBeTruthy();
    expect(
      within(fieldOf(container, 'cut-length')).getByText('8.0')
    ).toBeTruthy();
    expect(within(fieldOf(container, 'shank')).getByText('3.175')).toBeTruthy();
    expect(
      within(fieldOf(container, 'overall-length')).getByText('38.0')
    ).toBeTruthy();
    expect(
      within(fieldOf(container, 'spindle')).getByText('18000')
    ).toBeTruthy();
    expect(within(fieldOf(container, 'spindle')).getByText('rpm')).toBeTruthy();
    expect(
      within(fieldOf(container, 'cut-feed')).getByText('800')
    ).toBeTruthy();
    expect(
      within(fieldOf(container, 'cut-feed')).getByText('mm/min')
    ).toBeTruthy();
    expect(
      within(fieldOf(container, 'feed-plunge')).getByText('200')
    ).toBeTruthy();
    expect(within(fieldOf(container, 'stepover')).getByText('%')).toBeTruthy();
    // S10's `Type` shows the kind's own word; the shape word is absent from the payload.
    expect(
      within(fieldOf(container, 'type')).getByText('Flat end mill')
    ).toBeTruthy();
  });

  it('reports edits of the three editable fields through onedit and writes nothing itself', async () => {
    const onedit = vi.fn();
    render(ToolLibraryDetail, { detail, onedit });

    // Rendering is not an edit: opening a tool never reports a change.
    expect(onedit).not.toHaveBeenCalled();

    const name = screen.getByRole('textbox', { name: 'Name' });
    await userEvent.clear(name);
    await userEvent.type(name, 'End mill 1.0 mm - 3.175 mm');
    expect(onedit).toHaveBeenLastCalledWith({
      name: 'End mill 1.0 mm - 3.175 mm'
    });

    const number = screen.getByRole('spinbutton', { name: 'Number' });
    await userEvent.clear(number);
    // A cleared box is an absence, which the save payload expresses as `null`, not as `0`.
    expect(onedit).toHaveBeenLastCalledWith({ number: null });
    await userEvent.type(number, '23');
    expect(onedit).toHaveBeenLastCalledWith({ number: 23 });

    const flutes = screen.getByRole('spinbutton', { name: 'Flutes' });
    await userEvent.clear(flutes);
    await userEvent.type(flutes, '2');
    expect(onedit).toHaveBeenLastCalledWith({ flutes: 2 });

    // The callback is the whole write path: no command, no payload with a read-only field.
    for (const call of onedit.mock.calls) {
      expect(Object.keys(call[0])).toHaveLength(1);
      expect(['name', 'number', 'flutes']).toContain(Object.keys(call[0])[0]);
    }
  });

  it('renders the two absent values as the app dash instead of a zero or an empty cell', () => {
    const { container } = render(ToolLibraryDetail, { detail });

    // `stepoverPercent` is absent: S10's `40 %` cannot be derived from the stored length, so the
    // cell is a dash with its unit — never `0 %` and never an empty box.
    const stepover = fieldOf(container, 'stepover');
    expect(within(stepover).getByText('—')).toBeTruthy();
    expect(within(stepover).queryByText('0')).toBeNull();
    expect(within(stepover).getByText('%')).toBeTruthy();

    // The shape word of the type is absent too: the kind's word is shown, the shape is the dash.
    const type = fieldOf(container, 'type');
    expect(within(type).getByText('—')).toBeTruthy();
    expect(within(type).queryByText('flat')).toBeNull();
  });

  it('renders a tool held without a number with the dash badge, never Tnull or an empty badge', () => {
    const { container } = render(ToolLibraryDetail, {
      detail: { ...detail, number: null }
    });

    const badge = container.querySelector('[data-tool-number]');
    expect(badge?.textContent?.trim()).toBe('—');
    expect(screen.queryByText(/Tnull/)).toBeNull();
  });
});
