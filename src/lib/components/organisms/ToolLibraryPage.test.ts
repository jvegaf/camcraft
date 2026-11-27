import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import ToolLibraryPage from './ToolLibraryPage.svelte';

// Organism contract (107 T4, S8): the three-column page shell of
// `design/camcraft-workbench-v2/05-settings-tool-library.html` — the `Tool library` container with
// one column per pane, each with its own real heading and its own body. The bodies are empty
// placeholders here (T5/T6/T7 fill them through the named snippet props), so the tests assert the
// structure by role and accessible name instead of the CSS text: the grid may not be read in jsdom,
// and the meaningful fact is that every heading sits inside the column that also holds that
// column's body.
//
// The two header affordances are props, not decorations: `+ New` runs the create callback the caller
// composes, and the `Valid` badge is only claimed when the caller says the selected tool is valid.
// Both are asserted here because a header that advertises an inert action or a validity nobody
// checked is a promise this shell cannot keep.
const COLUMNS = ['Tools', 'Detail', 'Preview'] as const;

/** The column whose accessible name is `name` (each column is a labelled `<section>`). */
const columnOf = (name: string): HTMLElement =>
  screen.getByRole('region', { name });

/** The body of a column, identified by its own marker rather than by its position. */
const bodyOf = (name: string): HTMLElement | null =>
  columnOf(name).querySelector(`[data-column-body="${name.toLowerCase()}"]`);

/** A one-off snippet prop, the way a later task injects its column content. */
const snippet = (html: string) =>
  createRawSnippet(() => ({ render: () => html }));

describe('ToolLibraryPage', () => {
  it('renders the Tool library region with three headed columns and their empty bodies', () => {
    render(ToolLibraryPage);

    expect(screen.getByRole('region', { name: 'Tool library' })).toBeTruthy();
    expect(screen.getAllByRole('region')).toHaveLength(4);

    for (const name of COLUMNS) {
      const column = columnOf(name);
      // The column header is a real heading, and it belongs to the column it names.
      expect(
        within(column).getByRole('heading', { level: 4, name })
      ).toBeTruthy();
      // The column also holds its body; the body is the placeholder the later tasks fill and it
      // starts empty, so no column renders content that is not there yet.
      const body = bodyOf(name);
      expect(body).not.toBeNull();
      expect(body?.textContent?.trim()).toBe('');
    }
  });

  it('renders the + New button in the Tools column and the Valid badge in the Detail column', () => {
    render(ToolLibraryPage, { valid: true });

    expect(
      within(columnOf('Tools')).getByRole('button', { name: '+ New' })
    ).toBeTruthy();
    expect(within(columnOf('Detail')).getByText('Valid')).toBeTruthy();

    // No column borrows another column's header control.
    expect(within(columnOf('Detail')).queryByRole('button')).toBeNull();
    expect(within(columnOf('Preview')).queryByRole('button')).toBeNull();
    expect(within(columnOf('Tools')).queryByText('Valid')).toBeNull();
  });

  it('claims Valid only when the caller says the selected tool is valid', () => {
    // Nothing is selected: the header must not assert a validity it was not told about.
    render(ToolLibraryPage);

    expect(within(columnOf('Detail')).queryByText('Valid')).toBeNull();
    expect(screen.queryByText('Valid')).toBeNull();
    // The column header itself is still there; only the claim is absent.
    expect(
      within(columnOf('Detail')).getByRole('heading', {
        level: 4,
        name: 'Detail'
      })
    ).toBeTruthy();
  });

  it('runs the create action the + New button is wired to', async () => {
    const user = userEvent.setup();
    const create = vi.fn();
    render(ToolLibraryPage, { oncreate: create });

    await user.click(
      within(columnOf('Tools')).getByRole('button', { name: '+ New' })
    );

    expect(create).toHaveBeenCalledTimes(1);
    // The action stays in the Tools header; no other column grew a button.
    expect(within(columnOf('Detail')).queryByRole('button')).toBeNull();
  });

  it('injects one snippet per column body without reshaping the layout', () => {
    render(ToolLibraryPage, {
      tools: snippet('<p>Tool rows</p>'),
      detail: snippet('<p>Detail fields</p>'),
      preview: snippet('<p>Preview drawing</p>')
    });

    // Each snippet lands in its own column body, exactly once, and the headers stay in place.
    for (const [name, text] of [
      ['Tools', 'Tool rows'],
      ['Detail', 'Detail fields'],
      ['Preview', 'Preview drawing']
    ] as const) {
      expect(within(bodyOf(name) as HTMLElement).getByText(text)).toBeTruthy();
      expect(screen.getAllByText(text)).toHaveLength(1);
      expect(
        within(columnOf(name)).getByRole('heading', { level: 4, name })
      ).toBeTruthy();
    }
  });
});
