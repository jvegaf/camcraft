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

/** One module specifier and the names the importing file binds from it (102 T1, S2). */
export type ImportBinding = {
  /** The raw specifier, e.g. `$lib/components/atoms` or `./Button.svelte`. */
  specifier: string;
  /**
   * `null` when the whole module is reachable: a namespace import, a side-effect import, a default
   * import, or an `export * from`. Otherwise exactly the names the file binds, reported under their
   * exported name (so `import { Button as B }` contributes `Button`), never the local alias.
   */
  bindings: string[] | null;
  /** `import type` / `export type`: a type module exposes no component, so it credits nothing. */
  typeOnly: boolean;
};

/** A source file and the module bindings its imports point at. */
export type SourceFile = { path: string; imports: ImportBinding[] };

/** One re-export of a barrel: what it exposes and the module it comes from (102 T1, S2). */
export type BarrelExport = {
  /** The name the barrel exposes, or `null` for `export * from` (the whole module). */
  name: string | null;
  /** The name in the re-exported module, or `null` when the whole module is re-exported. */
  local: string | null;
  specifier: string;
  typeOnly: boolean;
};

/** An `index.ts` barrel and the names it re-exports, so a barrel import resolves to targets. */
export type BarrelFile = { path: string; exports: BarrelExport[] };

export type InventoryInput = {
  /** Component sources, repository-relative. */
  components: string[];
  /** Every source file the generator scanned, with the names its imports bind. */
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
 * Resolve one import to the component sources it reaches. The name the importer bound decides the
 * target (102 S2): a named binding only credits the member it names, while the whole-module forms
 * (`import * as`, side-effect, default, `export * from`) credit everything the module reaches,
 * because any member could be used through them. Type-only bindings never credit anything.
 */
export function resolveImportTargets(
  specifier: string,
  bindings: string[] | null,
  importerPath: string,
  components: readonly string[],
  barrels: readonly BarrelFile[]
): string[] {
  return resolveTargets(
    specifier,
    bindings,
    importerPath,
    new Set(components),
    new Map(barrels.map(barrel => [barrel.path, barrel])),
    new Set<string>()
  );
}

function resolveBarrel(
  barrel: BarrelFile,
  bindings: string[] | null,
  components: ReadonlySet<string>,
  barrels: ReadonlyMap<string, BarrelFile>,
  visitedBarrels: ReadonlySet<string>
): string[] {
  const resolveExport = (entry: BarrelExport): string[] => {
    const inner =
      entry.local === null || entry.local === 'default' ? null : [entry.local];
    return resolveTargets(
      entry.specifier,
      inner,
      barrel.path,
      components,
      barrels,
      visitedBarrels
    );
  };
  const reExportsWholeModule = barrel.exports.filter(
    entry => !entry.typeOnly && entry.name === null
  );

  if (bindings === null) {
    return unique(
      barrel.exports
        .filter(entry => !entry.typeOnly)
        .flatMap(entry => resolveExport(entry))
    );
  }

  const reached: string[] = [];
  for (const binding of bindings) {
    const exact = barrel.exports.find(
      entry => !entry.typeOnly && entry.name === binding
    );
    if (exact) {
      reached.push(...resolveExport(exact));
      continue;
    }
    // `export * from` exposes names this barrel never spells out, so a binding it does not map can
    // only be followed as the whole module — the pre-102 semantics, kept for that form alone.
    for (const wildcard of reExportsWholeModule) {
      reached.push(...resolveExport(wildcard));
    }
  }
  return unique(reached);
}

function resolveTargets(
  specifier: string,
  bindings: string[] | null,
  importerPath: string,
  components: ReadonlySet<string>,
  barrels: ReadonlyMap<string, BarrelFile>,
  visitedBarrels: ReadonlySet<string>
): string[] {
  const resolved = resolveSpecifier(specifier, importerPath);
  if (resolved === null) return [];

  const direct = [resolved, `${resolved}.svelte`];
  for (const candidate of direct) {
    if (!components.has(candidate)) continue;
    if (bindings === null) return [candidate];
    // A component file default-exports itself: `import Button from './Button.svelte'` binds it by
    // the `default` name, which the generator reports as the whole module. A named binding only
    // matches when the importer spelled the component name.
    if (bindings.includes(componentName(candidate))) return [candidate];
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
      return resolveBarrel(barrel, bindings, components, barrels, nextVisited);
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
      file.imports
        .filter(binding => !binding.typeOnly)
        .flatMap(binding =>
          resolveImportTargets(
            binding.specifier,
            binding.bindings,
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
