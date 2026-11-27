import { describe, expect, it } from 'vitest';
import {
  type BarrelFile,
  buildInventory,
  type ComponentEntry,
  componentName,
  type InventoryInput,
  layerForPath,
  type SourceFile,
  siblingTestPath
} from './inventory';

// Contract for the derived Atomic Design inventory (097 T1, S2). The classification and the
// cross-reference are pure: the generator reads the disk and hands plain path/import data in, so
// these assertions use a synthetic fixture and never touch the filesystem.

const A = 'src/lib/components/atoms/A.svelte';
const A_TEST = 'src/lib/components/atoms/A.test.ts';
const B = 'src/lib/components/atoms/B.svelte';
const M = 'src/lib/components/molecules/M.svelte';
const T = 'src/lib/components/templates/T.svelte';
const ROUTE = 'src/routes/+page.svelte';
const ATOM_INDEX = 'src/lib/components/atoms/index.ts';
const SETTINGS = 'src/lib/components/legacy/Z.svelte';
const CATALOG = 'src/lib/ds/catalog.ts';
const GENERATED = 'src/lib/ds/inventory.generated.ts';

const components = [A, B, M, T, SETTINGS];

const files: SourceFile[] = [
  { path: A, imports: [] },
  { path: B, imports: [] },
  // A molecule imports the atom by relative path, the way `PanelRow` imports `StatusDot`.
  { path: M, imports: ['../atoms/A.svelte'] },
  // A template imports the molecule through the `$lib` alias.
  { path: T, imports: ['$lib/components/molecules/M.svelte'] },
  // A route imports the atom through the atom barrel, the way routes import `organisms`.
  { path: ROUTE, imports: ['$lib/components/atoms'] },
  // The barrel itself re-exports the atom; it must not be reported as a consumer.
  { path: ATOM_INDEX, imports: ['./A.svelte'] },
  // A sibling test is not a consumer either: it exercises the component, it does not use it.
  { path: A_TEST, imports: ['./A.svelte'] },
  // The catalog imports every component to hold its reference; it is an inspector, not a user.
  { path: CATALOG, imports: ['$lib/components/atoms/A.svelte', 'B.svelte'] },
  // The generated inventory carries derived data only; it is never usage either.
  { path: GENERATED, imports: ['./inventory'] },
  { path: 'src/lib/components/legacy/Z.svelte', imports: [] }
];

const barrels: BarrelFile[] = [{ path: ATOM_INDEX, imports: ['./A.svelte'] }];

const existingFiles = [
  ...components,
  A_TEST,
  ROUTE,
  ATOM_INDEX,
  CATALOG,
  GENERATED,
  'src/lib/components/molecules/index.ts',
  'src/lib/components/templates/index.ts'
];

const input: InventoryInput = { components, files, barrels, existingFiles };

function entry(entries: ComponentEntry[], path: string): ComponentEntry {
  const found = entries.find(candidate => candidate.path === path);
  if (!found) throw new Error(`missing inventory entry: ${path}`);
  return found;
}

describe('layerForPath', () => {
  it('maps each atomic directory to its layer', () => {
    expect(layerForPath(A)).toBe('atom');
    expect(layerForPath(M)).toBe('molecule');
    expect(layerForPath('src/lib/components/organisms/StatusBar.svelte')).toBe(
      'organism'
    );
    expect(layerForPath(T)).toBe('template');
  });

  it('marks components outside the atomic directories as unclassified', () => {
    expect(layerForPath(SETTINGS)).toBe('unclassified');
    // The layer is the directory, never a declared claim (101 S3): a component that stays in a
    // feature folder keeps counting as unclassified no matter what a manifest says about it.
    expect(layerForPath('src/lib/components/legacy/BoardCanvas.svelte')).toBe(
      'unclassified'
    );
    expect(layerForPath('src/routes/+page.svelte')).toBe('unclassified');
  });
});

describe('path helpers', () => {
  it('derives the component name from the file name', () => {
    expect(componentName(A)).toBe('A');
    expect(componentName('src/lib/components/organisms/TopBar.svelte')).toBe(
      'TopBar'
    );
  });

  it('derives the sibling test path next to the component', () => {
    expect(siblingTestPath(A)).toBe(A_TEST);
  });
});

describe('buildInventory', () => {
  it('classifies every component source and links its sibling test', () => {
    const entries = buildInventory(input);

    expect(entry(entries, A)).toMatchObject({ name: 'A', layer: 'atom' });
    expect(entry(entries, A).test).toBe(A_TEST);
    expect(entry(entries, M).layer).toBe('molecule');
    expect(entry(entries, T).layer).toBe('template');
  });

  it('leaves a component without a sibling test with a null test', () => {
    expect(entry(buildInventory(input), B).test).toBeNull();
  });

  it('lists the real consumers and excludes the barrel and the sibling test', () => {
    const a = entry(buildInventory(input), A);

    expect(a.consumers).toEqual([
      'src/lib/components/molecules/M.svelte',
      'src/routes/+page.svelte'
    ]);
    expect(a.consumers).not.toContain(A);
    expect(a.consumers).not.toContain(ATOM_INDEX);
    expect(a.consumers).not.toContain(A_TEST);
  });

  it('never counts the catalog or its generated data as consumers', () => {
    const entries = buildInventory(input);

    expect(entry(entries, A).consumers).toEqual([
      'src/lib/components/molecules/M.svelte',
      'src/routes/+page.svelte'
    ]);
    expect(entry(entries, B).consumers).toEqual([]);
    expect('templateConsumers' in entry(entries, A)).toBe(false);
  });

  it('reports the unclassified component instead of hiding it', () => {
    const z = entry(buildInventory(input), SETTINGS);

    expect(z.layer).toBe('unclassified');
    expect(z.test).toBeNull();
  });

  it('is deterministic: same input, same output', () => {
    const first = buildInventory(input);
    const second = buildInventory({
      ...input,
      components: [...components].reverse(),
      files: [...files].reverse()
    });

    expect(second).toEqual(first);
    expect(JSON.stringify(second)).toBe(JSON.stringify(first));
  });
});
