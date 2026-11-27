import { describe, expect, it } from 'vitest';
import {
  type BarrelExport,
  type BarrelFile,
  buildInventory,
  type ComponentEntry,
  componentName,
  type ImportBinding,
  type InventoryInput,
  layerForPath,
  type SourceFile,
  siblingTestPath
} from './inventory';

/** `import { ...names } from specifier`; aliases are reported under their exported name. */
function named(specifier: string, ...bindings: string[]): ImportBinding {
  return { specifier, bindings, typeOnly: false };
}

/** `import * as ns`, `import 'x'` or a default import: the whole module is reachable. */
function wholeModule(specifier: string): ImportBinding {
  return { specifier, bindings: null, typeOnly: false };
}

/** `import type { ... }` / `export type ... from`: it never credits a component. */
function typeOnly(specifier: string, ...bindings: string[]): ImportBinding {
  return { specifier, bindings, typeOnly: true };
}

function reExport(name: string | null, specifier: string): BarrelExport {
  return {
    name,
    local: name === null ? null : 'default',
    specifier,
    typeOnly: false
  };
}

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
  { path: M, imports: [wholeModule('../atoms/A.svelte')] },
  // A template imports the molecule through the `$lib` alias.
  { path: T, imports: [wholeModule('$lib/components/molecules/M.svelte')] },
  // A route imports the atom through the atom barrel, the way routes import `organisms`.
  { path: ROUTE, imports: [named('$lib/components/atoms', 'A')] },
  // The barrel itself re-exports the atom; it must not be reported as a consumer.
  { path: ATOM_INDEX, imports: [wholeModule('./A.svelte')] },
  // A sibling test is not a consumer either: it exercises the component, it does not use it.
  { path: A_TEST, imports: [wholeModule('./A.svelte')] },
  // The catalog imports every component to hold its reference; it is an inspector, not a user.
  { path: CATALOG, imports: [wholeModule('$lib/components/atoms/A.svelte')] },
  // The generated inventory carries derived data only; it is never usage either.
  { path: GENERATED, imports: [wholeModule('./inventory')] },
  { path: 'src/lib/components/legacy/Z.svelte', imports: [] }
];

const barrels: BarrelFile[] = [
  { path: ATOM_INDEX, exports: [reExport('A', './A.svelte')] }
];

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

// --- Exact consumers (102 T1 + T3, S2) ---------------------------------------------------------
//
// The name the importer binds decides the target. Only the whole-module forms keep the pre-102
// semantics, where reachability is enough, because any member could be used through them.

describe('binding resolution', () => {
  const USER = 'src/routes/uses.svelte';
  const MOLECULE_BARREL = 'src/lib/components/molecules/index.ts';

  /** Inventory of A + B with a synthetic importer and the barrels declared by each test. */
  function consumersWith(
    imports: ImportBinding[],
    barrels: BarrelFile[] = [
      {
        path: ATOM_INDEX,
        exports: [
          reExport('A', './A.svelte'),
          reExport('B', './B.svelte'),
          { name: 'Name', local: 'Name', specifier: './types', typeOnly: true },
          reExport('Nested', '../molecules')
        ]
      }
    ]
  ): ComponentEntry[] {
    return buildInventory({
      components: [A, B, M],
      files: [
        { path: A, imports: [] },
        { path: B, imports: [] },
        { path: M, imports: [] },
        { path: USER, imports }
      ],
      barrels,
      existingFiles: [A, B, M, ATOM_INDEX, MOLECULE_BARREL, USER]
    });
  }

  function consumersOf(entries: ComponentEntry[], path: string): string[] {
    return entry(entries, path).consumers;
  }

  it('credits only the component the importer named', () => {
    const entries = consumersWith([named('$lib/components/atoms', 'A')]);

    expect(consumersOf(entries, A)).toEqual([USER]);
    expect(consumersOf(entries, B)).toEqual([]);
  });

  it('credits the aliased component when the import renames it', () => {
    // `import { B as Bee } from '../atoms'` binds `B`; the generator reports the exported name.
    const entries = consumersWith([named('$lib/components/atoms', 'B')]);

    expect(consumersOf(entries, A)).toEqual([]);
    expect(consumersOf(entries, B)).toEqual([USER]);
  });

  it('does not credit the siblings of the member a barrel was imported for', () => {
    const entries = consumersWith([named('$lib/components/atoms', 'A', 'B')]);

    expect(consumersOf(entries, A)).toEqual([USER]);
    expect(consumersOf(entries, B)).toEqual([USER]);
    // `M` is not exported by the atom barrel at all, so naming A and B cannot reach it.
    expect(consumersOf(entries, M)).toEqual([]);
  });

  it('credits the whole module through a namespace import', () => {
    const entries = consumersWith([wholeModule('$lib/components/atoms')]);

    expect(consumersOf(entries, A)).toEqual([USER]);
    expect(consumersOf(entries, B)).toEqual([USER]);
  });

  it('credits the whole module through a side-effect import of a component', () => {
    const entries = consumersWith([
      wholeModule('$lib/components/atoms/A.svelte')
    ]);

    expect(consumersOf(entries, A)).toEqual([USER]);
    expect(consumersOf(entries, B)).toEqual([]);
  });

  it('treats `export * from` as the whole module', () => {
    const entries = consumersWith(
      [named('$lib/components/atoms', 'Anything')],
      [
        {
          path: ATOM_INDEX,
          exports: [
            {
              name: null,
              local: null,
              specifier: './A.svelte',
              typeOnly: false
            }
          ]
        }
      ]
    );

    expect(consumersOf(entries, A)).toEqual([USER]);
    expect(consumersOf(entries, B)).toEqual([]);
  });

  it('credits a component imported as the default of a `.svelte` file', () => {
    const entries = consumersWith([
      wholeModule('$lib/components/atoms/B.svelte')
    ]);

    expect(consumersOf(entries, B)).toEqual([USER]);
    expect(consumersOf(entries, A)).toEqual([]);
  });

  it('never credits through a type-only import or re-export', () => {
    const entries = consumersWith([
      typeOnly('$lib/components/atoms', 'A', 'Name'),
      typeOnly('$lib/components/atoms/B.svelte')
    ]);

    expect(consumersOf(entries, A)).toEqual([]);
    expect(consumersOf(entries, B)).toEqual([]);
  });

  it('resolves a member re-exported by a nested barrel', () => {
    const entries = consumersWith(
      [named('$lib/components/atoms', 'Nested')],
      [
        {
          path: ATOM_INDEX,
          exports: [
            {
              name: 'Nested',
              local: 'M',
              specifier: '../molecules',
              typeOnly: false
            }
          ]
        },
        {
          path: MOLECULE_BARREL,
          exports: [reExport('M', './M.svelte')]
        }
      ]
    );

    expect(consumersOf(entries, M)).toEqual([USER]);
    expect(consumersOf(entries, A)).toEqual([]);
    expect(consumersOf(entries, B)).toEqual([]);
  });
});
