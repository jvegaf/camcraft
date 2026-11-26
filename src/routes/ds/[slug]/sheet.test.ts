import { render } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CATALOG, findCatalogEntry } from '$lib/ds/catalog';
import { inventory } from '$lib/ds/inventory.generated';
import Sheet from './+page.svelte';

// Component sheet (097 T2, S2 + S4). The route parameter is mocked at the SvelteKit boundary; the
// sheet renders the real component instances, one per declared state, and an unknown slug must
// resolve to SvelteKit's 404 instead of rendering an empty frame.
const { pageMock } = vi.hoisted(() => ({
  pageMock: { params: { slug: 'button' } }
}));

vi.mock('$app/state', () => ({ page: pageMock }));

function statesFor(slug: string) {
  const entry = findCatalogEntry(slug);
  if (!entry) throw new Error(`missing catalog entry: ${slug}`);
  return entry.states;
}

describe('ds component sheet', () => {
  beforeEach(() => {
    pageMock.params.slug = 'button';
  });

  afterEach(() => {
    delete document.documentElement.dataset.theme;
  });

  it('renders one real instance per declared state', () => {
    const { container } = render(Sheet);
    const states = statesFor('button');

    const frames = Array.from(container.querySelectorAll('[data-ds-state]'));
    expect(frames).toHaveLength(states.length);
    expect(
      frames.map(frame => frame.getAttribute('data-ds-state-label'))
    ).toEqual(states.map(state => state.label));
    for (const frame of frames) {
      expect(frame.querySelector('[data-ds-instance]')).not.toBeNull();
      expect(frame.querySelector('[data-ds-props]')?.textContent).toBeTruthy();
    }
  });

  it('renders every catalogued component sheet with its state count', () => {
    for (const entry of CATALOG) {
      pageMock.params.slug = entry.slug;
      const { container, unmount } = render(Sheet);

      expect(
        container.querySelectorAll('[data-ds-state]'),
        entry.slug
      ).toHaveLength(entry.states.length);

      const component = inventory.find(item => item.path === entry.path);
      expect(component, entry.slug).toBeDefined();
      expect(
        container.querySelector('[data-ds-consumer-count]')?.textContent,
        entry.slug
      ).toBe(String(component?.consumers.length));
      if ((component?.consumers.length ?? 0) > 0) {
        const firstName = component?.consumers[0].split('/').pop() ?? '';
        expect(
          container.querySelector('[data-ds-consumer-names]')?.textContent,
          entry.slug
        ).toContain(firstName);
      }
      if (entry.renderable) {
        expect(
          container.querySelectorAll('[data-ds-instance]'),
          entry.slug
        ).toHaveLength(entry.states.length);
      } else {
        expect(
          container.querySelectorAll('[data-ds-instance]'),
          entry.slug
        ).toHaveLength(0);
        expect(
          container.querySelector('[data-ds-render-note]')?.textContent,
          entry.slug
        ).toContain(entry.reason ?? '');
      }
      unmount();
    }
  });

  it('links back to the catalog index', () => {
    const { container } = render(Sheet);

    expect(container.querySelector('a[href="/ds"]')).not.toBeNull();
  });

  it('answers an unknown slug with a 404', () => {
    pageMock.params.slug = 'not-a-component';

    try {
      render(Sheet);
      throw new Error('the sheet rendered an unknown slug');
    } catch (thrown) {
      expect(thrown).toMatchObject({ status: 404 });
    }
  });

  it('switches the html[data-theme] condition without persisting it', async () => {
    const user = userEvent.setup();
    const { unmount } = render(Sheet);

    expect(document.documentElement.dataset.theme).toBeUndefined();

    await user.click(
      document.querySelector('[data-theme-option="light"]') as HTMLElement
    );
    expect(document.documentElement.dataset.theme).toBe('light');

    await user.click(
      document.querySelector('[data-theme-option="dark"]') as HTMLElement
    );
    expect(document.documentElement.dataset.theme).toBe('dark');

    unmount();
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });
});
