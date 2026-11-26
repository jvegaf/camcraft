import { describe, expect, it } from 'vitest';
import {
  buildBoard,
  CATALOG,
  type CatalogEntry,
  findCatalogEntry
} from './catalog';
import type { ComponentEntry } from './inventory';
import { layerForPath } from './inventory';
import { inventory } from './inventory.generated';

// Contract for the catalog manifest and the progress board (097 T2 + T3, S2). Two honesty rules are
// pinned here: the manifest covers exactly the components the inventory derived, and a component
// that cannot render without host IPC, a live store or a canvas is declared `renderable: false`
// with a reason instead of posing as covered.
describe('catalog manifest', () => {
  it('has a unique slug and one entry per atomic component', () => {
    const slugs = CATALOG.map(entry => entry.slug);
    expect(new Set(slugs).size).toBe(slugs.length);

    const catalogPaths = new Set(CATALOG.map(entry => entry.path));
    for (const component of inventory) {
      if (component.layer === 'unclassified') continue;
      expect(
        catalogPaths.has(component.path),
        `missing catalog entry: ${component.path}`
      ).toBe(true);
    }
  });

  it('declares the layer the inventory derived for every component', () => {
    for (const entry of CATALOG) {
      expect(entry.layer, entry.slug).toBe(layerForPath(entry.path));
    }
  });

  it('derives at least one state per component from its real props', () => {
    for (const entry of CATALOG) {
      expect(entry.states.length, entry.slug).toBeGreaterThan(0);
      const labels = entry.states.map(state => state.label);
      expect(new Set(labels).size, entry.slug).toBe(labels.length);
    }
  });

  it('gives every non-isolatable component a reason and no rendered states', () => {
    const notRenderable = CATALOG.filter(entry => !entry.renderable);

    expect(notRenderable.map(entry => entry.slug).sort()).toEqual([
      'cam-object-browser',
      'operation-inspector',
      'status-bar',
      'top-bar',
      'viewport-hud'
    ]);
    for (const entry of notRenderable) {
      expect(entry.reason, entry.slug).toBeTruthy();
      expect(entry.states.length, entry.slug).toBeGreaterThan(0);
    }
  });

  it('finds an entry by slug and returns undefined for anything else', () => {
    expect(findCatalogEntry('button')?.name).toBe('Button');
    expect(findCatalogEntry('nope')).toBeUndefined();
    expect(findCatalogEntry('')).toBeUndefined();
  });
});

describe('buildBoard', () => {
  const atomA: ComponentEntry = {
    path: 'src/lib/components/atoms/A.svelte',
    name: 'A',
    layer: 'atom',
    test: 'src/lib/components/atoms/A.test.ts',
    consumers: ['src/lib/components/templates/T.svelte']
  };
  const atomB: ComponentEntry = {
    path: 'src/lib/components/atoms/B.svelte',
    name: 'B',
    layer: 'atom',
    test: null,
    consumers: []
  };
  const molecule: ComponentEntry = {
    path: 'src/lib/components/molecules/M.svelte',
    name: 'M',
    layer: 'molecule',
    test: 'src/lib/components/molecules/M.test.ts',
    consumers: []
  };
  const unclassified: ComponentEntry = {
    path: 'src/lib/components/settings/Z.svelte',
    name: 'Z',
    layer: 'unclassified',
    test: null,
    consumers: []
  };
  const entries = [atomA, atomB, molecule, unclassified];

  const catalog = [
    { slug: 'a', path: atomA.path, layer: 'atom', renderable: true },
    { slug: 'm', path: molecule.path, layer: 'molecule', renderable: false }
  ] as unknown as CatalogEntry[];

  it('counts totals and the three coverage facts per layer', () => {
    const board = buildBoard(entries, catalog);
    const atom = board.layers.find(layer => layer.layer === 'atom');
    const moleculeLayer = board.layers.find(
      layer => layer.layer === 'molecule'
    );

    expect(atom).toMatchObject({
      total: 2,
      withSheet: 1,
      withTest: 1,
      inUse: 1
    });
    expect(moleculeLayer).toMatchObject({
      total: 1,
      withSheet: 1,
      withTest: 1,
      inUse: 0
    });
    expect(board.layers.map(layer => layer.layer)).toEqual([
      'atom',
      'molecule',
      'organism',
      'template',
      'unclassified'
    ]);
  });

  it('counts the renderable: false bucket separately', () => {
    const board = buildBoard(entries, catalog);
    const atom = board.layers.find(layer => layer.layer === 'atom');
    const moleculeLayer = board.layers.find(
      layer => layer.layer === 'molecule'
    );

    expect(atom).toMatchObject({ renderable: 1, notRenderable: 0 });
    expect(moleculeLayer).toMatchObject({ renderable: 0, notRenderable: 1 });
  });

  it('lists the unclassified components explicitly', () => {
    const board = buildBoard(entries, catalog);

    expect(board.unclassified.map(item => item.path)).toEqual([
      unclassified.path
    ]);
  });
});
