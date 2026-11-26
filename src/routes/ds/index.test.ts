import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { buildBoard, CATALOG } from '$lib/ds/catalog';
import { inventory } from '$lib/ds/inventory.generated';
import Page from './+page.svelte';

// Index and progress board (097 T3, S1 + S2): the board is derived, never declared. These
// assertions compare the rendered numbers with the same pure function the page calls, and check
// that the honest badges and the unclassified list reach the DOM.
describe('ds index route', () => {
  it('lists the atomic layers in order', () => {
    const { container } = render(Page);

    const sections = Array.from(
      container.querySelectorAll('[data-layer-section]')
    ).map(section => section.getAttribute('data-layer-section'));

    expect(sections).toEqual(['atom', 'molecule', 'organism', 'template']);
  });

  it('labels the third coverage fact as componentes en uso', () => {
    const { container } = render(Page);
    const headers = Array.from(container.querySelectorAll('thead th')).map(
      header => header.textContent?.trim()
    );

    expect(headers).toContain('Componentes en uso');
    expect(headers.join(' ')).not.toContain('template');
  });

  it('renders the derived totals and coverage counts per layer', () => {
    const { container } = render(Page);
    const board = buildBoard(inventory, CATALOG);

    for (const summary of board.layers) {
      const row = container.querySelector(
        `[data-board-layer="${summary.layer}"]`
      );
      expect(row, summary.layer).not.toBeNull();
      expect(row?.querySelector('[data-board-total]')?.textContent).toBe(
        String(summary.total)
      );
      expect(row?.querySelector('[data-board-sheet]')?.textContent).toBe(
        String(summary.withSheet)
      );
      expect(row?.querySelector('[data-board-test]')?.textContent).toBe(
        String(summary.withTest)
      );
      expect(row?.querySelector('[data-board-inuse]')?.textContent).toBe(
        String(summary.inUse)
      );
      expect(row?.querySelector('[data-board-renderable]')?.textContent).toBe(
        String(summary.renderable)
      );
      expect(
        row?.querySelector('[data-board-not-renderable]')?.textContent
      ).toBe(String(summary.notRenderable));
    }
  });

  it('lists the unclassified components instead of hiding them', () => {
    const { container } = render(Page);
    const listed = Array.from(
      container.querySelectorAll('[data-unclassified-item]')
    ).map(item => item.getAttribute('data-unclassified-item'));
    const expected = inventory
      .filter(component => component.layer === 'unclassified')
      .map(component => component.path);

    expect(listed).toEqual(expected);
    expect(expected.length).toBeGreaterThan(0);
  });

  it('links every catalogued component to its sheet', () => {
    const { container } = render(Page);

    for (const entry of CATALOG) {
      expect(
        container.querySelector(`a[href="/ds/${entry.slug}"]`),
        entry.slug
      ).not.toBeNull();
    }
  });

  it('shows the sheet, sibling-test and template-consumer facts as badges', () => {
    const { container } = render(Page);
    const catalogPaths = new Set(CATALOG.map(entry => entry.path));

    for (const component of inventory) {
      if (component.layer === 'unclassified') continue;
      const row = container.querySelector(
        `[data-component-path="${component.path}"]`
      );
      expect(row, component.path).not.toBeNull();
      expect(
        row
          ?.querySelector('[data-badge-sheet]')
          ?.getAttribute('data-badge-sheet')
      ).toBe(catalogPaths.has(component.path) ? 'yes' : 'no');
      expect(
        row?.querySelector('[data-badge-test]')?.getAttribute('data-badge-test')
      ).toBe(component.test ? 'yes' : 'no');
      expect(
        row
          ?.querySelector('[data-badge-inuse]')
          ?.getAttribute('data-badge-inuse')
      ).toBe(component.consumers.length > 0 ? 'yes' : 'no');
    }
  });
});
