// Derived Atomic Design inventory (097 T1, S1 + S2 + S4).
//
// This module is pure on purpose: the generator (`scripts/ds-inventory.mjs`) reads the repository
// and hands plain path/import data to `buildInventory`, which classifies each component source,
// links its sibling test and cross-references who imports it. Nothing here touches the filesystem,
// so the rules are tested over synthetic fixtures and the committed output stays reproducible.

export type Layer =
  | 'atom'
  | 'molecule'
  | 'organism'
  | 'template'
  | 'unclassified';

/** Atomic order of the catalog; `unclassified` is reported last instead of hidden. */
export const LAYERS: readonly Layer[] = [
  'atom',
  'molecule',
  'organism',
  'template',
  'unclassified'
];

const LAYER_BY_DIRECTORY: Record<string, Layer> = {
  atoms: 'atom',
  molecules: 'molecule',
  organisms: 'organism',
  templates: 'template'
};

export type ComponentEntry = {
  /** Repository-relative path with POSIX separators. */
  path: string;
  /** File name without its extension, e.g. `Button`. */
  name: string;
  layer: Layer;
  /** Repository-relative sibling test path (`X.test.ts`), or `null` when there is none. */
  test: string | null;
  /** Repository-relative files that use the component, sorted. */
  consumers: string[];
};

/** A source file and the module specifiers its imports/re-exports point at. */
export type SourceFile = { path: string; imports: string[] };

/** An `index.ts` barrel and the specifiers it re-exports, so a barrel import resolves to targets. */
export type BarrelFile = { path: string; imports: string[] };

export type InventoryInput = {
  /** Component sources, repository-relative. */
  components: string[];
  /** Every source file the generator scanned, with its raw import specifiers. */
  files: SourceFile[];
  /** Barrels found next to the components. */
  barrels: BarrelFile[];
  /** Every existing file the generator found, used to resolve `X.test.ts`. */
  existingFiles: string[];
};

/** The layer named by the first directory below `components/`. */
export function layerForPath(path: string): Layer {
  const segments = path.split('/');
  const componentsIndex = segments.lastIndexOf('components');
  if (componentsIndex === -1) return 'unclassified';
  const directory = segments[componentsIndex + 1];
  return LAYER_BY_DIRECTORY[directory] ?? 'unclassified';
}

export function componentName(path: string): string {
  const file = path.split('/').pop() ?? path;
  return file.replace(/\.[^.]+$/, '');
}

/** `src/lib/components/atoms/Button.svelte` -> `src/lib/components/atoms/Button.test.ts`. */
export function siblingTestPath(path: string): string {
  return `${path.replace(/\.[^.]+$/, '')}.test.ts`;
}

function normalizePath(path: string): string {
  const absolute = path.startsWith('/');
  const segments: string[] = [];
  for (const segment of path.split('/')) {
    if (segment === '' || segment === '.') continue;
    if (segment === '..') {
      if (segments.length > 0 && segments[segments.length - 1] !== '..') {
        segments.pop();
      } else if (!absolute) {
        segments.push('..');
      }
      continue;
    }
    segments.push(segment);
  }
  return `${absolute ? '/' : ''}${segments.join('/')}`;
}

function directoryOf(path: string): string {
  return path.split('/').slice(0, -1).join('/');
}

/** Resolve a project-local specifier to a repository path; external packages resolve to `null`. */
function resolveSpecifier(
  specifier: string,
  importerPath: string
): string | null {
  if (specifier.startsWith('$lib/')) {
    return normalizePath(`src/lib/${specifier.slice('$lib/'.length)}`);
  }
  if (specifier.startsWith('.')) {
    return normalizePath(`${directoryOf(importerPath)}/${specifier}`);
  }
  return null;
}

function unique(values: string[]): string[] {
  return [...new Set(values)];
}

/**
 * Resolve one import specifier to the component sources it reaches. Direct file imports and barrel
 * imports (`$lib/components/atoms`, `../molecules`) are both followed, so a component consumed
 * through an `index.ts` is still credited to the file that imported the barrel.
 */
export function resolveImportTargets(
  specifier: string,
  importerPath: string,
  components: readonly string[],
  barrels: readonly BarrelFile[]
): string[] {
  return resolveTargets(
    specifier,
    importerPath,
    new Set(components),
    new Map(barrels.map(barrel => [barrel.path, barrel])),
    new Set<string>()
  );
}

function resolveTargets(
  specifier: string,
  importerPath: string,
  components: ReadonlySet<string>,
  barrels: ReadonlyMap<string, BarrelFile>,
  visitedBarrels: ReadonlySet<string>
): string[] {
  const resolved = resolveSpecifier(specifier, importerPath);
  if (resolved === null) return [];

  const direct = [resolved, `${resolved}.svelte`];
  for (const candidate of direct) {
    if (components.has(candidate)) return [candidate];
  }

  for (const candidate of [
    `${resolved}.ts`,
    `${resolved}/index.ts`,
    resolved
  ]) {
    const barrel = barrels.get(candidate);
    if (barrel && !visitedBarrels.has(barrel.path)) {
      const nextVisited = new Set(visitedBarrels);
      nextVisited.add(barrel.path);
      return unique(
        barrel.imports.flatMap(inner =>
          resolveTargets(inner, barrel.path, components, barrels, nextVisited)
        )
      );
    }
  }

  return [];
}

function isTestFile(path: string): boolean {
  return /\.(test|spec)\.[^.]+$/.test(path);
}

/**
 * The gallery inspects the components; it does not use them. `catalog.ts` imports every component
 * to hold its reference and the generated inventory only carries the derived data, so neither may
 * count as a consumer or every component would look "in use" by construction.
 */
const NON_CONSUMER_FILES: ReadonlySet<string> = new Set([
  'src/lib/ds/catalog.ts',
  'src/lib/ds/inventory.generated.ts'
]);

/**
 * Build the inventory. Component sources are sorted by path and every derived list is sorted too,
 * so reversing the input order yields the same serialized output (the generator relies on it).
 */
export function buildInventory(input: InventoryInput): ComponentEntry[] {
  const existingFiles = new Set(input.existingFiles);
  const barrelPaths = new Set(input.barrels.map(barrel => barrel.path));
  const consumers = new Map<string, Set<string>>(
    input.components.map(path => [path, new Set<string>()])
  );

  for (const file of input.files) {
    // A barrel re-exports the component; it is an indirection, not a use. The importer of the
    // barrel is credited directly by `resolveImportTargets`, so skipping the barrel loses nothing.
    if (barrelPaths.has(file.path)) continue;
    // A sibling test exercises the component; it is not a consumer of it.
    if (isTestFile(file.path)) continue;
    // The catalog and its generated data inspect the components; they are not usage.
    if (NON_CONSUMER_FILES.has(file.path)) continue;

    const targets = unique(
      file.imports.flatMap(specifier =>
        resolveImportTargets(
          specifier,
          file.path,
          input.components,
          input.barrels
        )
      )
    );
    for (const target of targets) {
      if (target === file.path) continue;
      consumers.get(target)?.add(file.path);
    }
  }

  return [...input.components].sort().map(path => {
    const consumerList = [...(consumers.get(path) ?? [])].sort();
    const test = siblingTestPath(path);
    return {
      path,
      name: componentName(path),
      layer: layerForPath(path),
      test: existingFiles.has(test) ? test : null,
      consumers: consumerList
    };
  });
}
