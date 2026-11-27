#!/usr/bin/env node
// Generates `src/lib/ds/inventory.generated.ts` from the repository sources (097 T1, S1 + S2 + S4).
//
// The script is a thin disk reader: it walks `src/`, collects component sources, sibling files,
// barrels and import bindings, and hands that plain data to the pure `buildInventory` in
// `src/lib/ds/inventory.ts`. Classification and cross-reference live there, where they are tested.
//
// `src/lib/components/atoms/icons.ts` is deliberately not listed as a component: it is the shape
// data the `Icon` atom renders, not a component source, so it has no layer of its own in the
// catalog. The board still reports it as unclassified only if it were a `.svelte` file.
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildInventory } from '../src/lib/ds/inventory.ts';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const componentsRoot = join(root, 'src/lib/components');
const outputPath = join(root, 'src/lib/ds/inventory.generated.ts');

function toRepositoryPath(absolutePath) {
  return relative(root, absolutePath).split(sep).join('/');
}

function walk(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))
    .flatMap(entry => {
      const absolute = join(directory, entry.name);
      return entry.isDirectory() ? walk(absolute) : [absolute];
    });
}

/**
 * Every import of the module, with the names it binds (102 T2, S2).
 *
 * Regex-based on purpose: no parser dependency. Statements are read only when the `import` /
 * `export` keyword starts a line (indentation allowed), which is how the formatter writes them and
 * what keeps markup like `<Icon name="import" />` from being mistaken for an import. The honest
 * limits of that choice:
 *
 * - Dynamic `import('./x')` and `require('./x')` are not read at all, so a module reached only that
 *   way contributes no consumer. It under-credits; it never invents one.
 * - A statement whose keyword does not start a line (e.g. `const a = 1; import b from 'x'`) is not
 *   read either.
 * - `export * as ns from './x'` (a namespace re-export) is not read: the `*` branch requires `from`
 *   right after it. It under-credits today.
 * - A default import is reported as the whole module (see `ImportBinding.bindings`), so a default
 *   import of a `.ts` barrel credits every member it re-exports. No such form exists in this
 *   repository; it is the one place where the pre-102 semantics survive beyond the forms the spec
 *   names.
 */
function importsOf(source) {
  const found = [];
  for (const match of source.matchAll(
    /^[ \t]*import\s+([^;'"]*?)\s*from\s*['"]([^'"]+)['"]/gm
  )) {
    found.push({ specifier: match[2], ...parseImportClause(match[1]) });
  }
  for (const match of source.matchAll(/^[ \t]*import\s*['"]([^'"]+)['"]/gm)) {
    found.push({ specifier: match[1], bindings: null, typeOnly: false });
  }
  return found.sort(
    (left, right) =>
      left.specifier.localeCompare(right.specifier) ||
      String(left.bindings).localeCompare(String(right.bindings))
  );
}

/** `type { A }`, `{ A as B }`, `D, { A }`, `* as ns`, `D`; aliases resolve to the exported name. */
function parseImportClause(rawClause) {
  let clause = rawClause.trim();
  let typeOnly = false;
  if (/^type\s/.test(clause)) {
    typeOnly = true;
    clause = clause.replace(/^type\s+/, '');
  }
  if (/^\*\s*as\s+[\w$]+$/.test(clause)) {
    return { bindings: null, typeOnly };
  }
  const open = clause.indexOf('{');
  if (open === -1) {
    // A default import binds `default`; the whole module stays reachable through it.
    return { bindings: null, typeOnly };
  }
  const close = clause.lastIndexOf('}');
  const inside = clause.slice(open + 1, close === -1 ? undefined : close);
  const bindings = [];
  let sawValue = false;
  for (const item of parseSpecifierList(inside)) {
    if (item.typeOnly) continue;
    sawValue = true;
    bindings.push(item.local);
  }
  // `import Default, { A } from 'x'` also binds the default, so the whole module is reachable.
  if (clause.slice(0, open).trim() !== '') {
    return { bindings: null, typeOnly };
  }
  // `import { type A } from 'x'` is a value import of no value at all.
  return { bindings, typeOnly: typeOnly || !sawValue };
}

/**
 * Parse a `{ ... }` specifier list. `local` is the name in the source module; `exported` is the
 * name the statement exposes, identical to `local` for imports.
 */
function parseSpecifierList(inside) {
  const items = [];
  for (const entry of inside.split(',')) {
    let text = entry.trim();
    if (text === '') continue;
    const typeOnly = /^type\s/.test(text);
    if (typeOnly) text = text.replace(/^type\s+/, '');
    const [local, exported = local] = text.split(/\s+as\s+/);
    items.push({ local: local.trim(), exported: exported.trim(), typeOnly });
  }
  return items;
}

/** The re-exports of a barrel, as `{ name, local, specifier, typeOnly }` (102 T1, S2). */
function exportsOf(source) {
  const found = [];
  for (const match of source.matchAll(
    /^[ \t]*export\s+(?:(type)\s+)?(\*|\{[^}]*\})\s*from\s*['"]([^'"]+)['"]/gm
  )) {
    const specifier = match[3];
    const keywordTypeOnly = match[1] === 'type';
    if (match[2] === '*') {
      // `export * from './x'` re-exports names this file never spells out: whole module.
      found.push({
        name: null,
        local: null,
        specifier,
        typeOnly: keywordTypeOnly
      });
      continue;
    }
    for (const item of parseSpecifierList(match[2].slice(1, -1))) {
      found.push({
        name: item.exported,
        local: item.local,
        specifier,
        typeOnly: keywordTypeOnly || item.typeOnly
      });
    }
  }
  return found;
}

const sourceFiles = walk(join(root, 'src')).filter(file =>
  /\.(svelte|ts)$/.test(file)
);
const existingFiles = sourceFiles.map(toRepositoryPath);

const parsed = new Map();
function moduleOf(path) {
  if (!parsed.has(path)) {
    const source = readFileSync(join(root, path), 'utf8');
    parsed.set(path, {
      imports: importsOf(source),
      exports: exportsOf(source)
    });
  }
  return parsed.get(path);
}

const files = existingFiles.map(path => ({
  path,
  imports: moduleOf(path).imports
}));

const components = walk(componentsRoot)
  .filter(file => file.endsWith('.svelte'))
  .map(toRepositoryPath);

const barrels = existingFiles
  .filter(
    path => path.startsWith('src/lib/components/') && path.endsWith('/index.ts')
  )
  .map(path => ({ path, exports: moduleOf(path).exports }));

const inventory = buildInventory({ components, files, barrels, existingFiles });

// ---------------------------------------------------------------------------------------------
// Deterministic, formatter-clean serialization. `JSON.stringify` is not used because the module is
// a TypeScript file Biome checks: the emitted literals use the project's single quotes and the
// arrays collapse only when they fit the 80-column line width.
const LINE_WIDTH = 80;

function tsString(value) {
  return `'${String(value).replace(/\\/g, '\\\\').replace(/'/g, "\\'")}'`;
}

function tsArray(values, propertyIndent, propertyLabel) {
  if (values.length === 0) return '[]';
  const inline = `[${values.map(tsString).join(', ')}]`;
  const lineLength =
    propertyIndent.length + propertyLabel.length + 2 + inline.length;
  if (lineLength <= LINE_WIDTH) return inline;
  const inner = `${propertyIndent}  `;
  return `[\n${values
    .map(value => `${inner}${tsString(value)}`)
    .join(',\n')}\n${propertyIndent}]`;
}

function entrySource(entry) {
  const indent = '    ';
  const properties = [
    `path: ${tsString(entry.path)}`,
    `name: ${tsString(entry.name)}`,
    `layer: ${tsString(entry.layer)}`,
    `test: ${entry.test === null ? 'null' : tsString(entry.test)}`,
    `consumers: ${tsArray(entry.consumers, indent, 'consumers')}`
  ];
  return `  {\n${properties.map(line => `${indent}${line}`).join(',\n')}\n  }`;
}

const header = `// @generated by \`pnpm ds:inventory\` from the repository sources.
// Do not edit by hand: run the generator instead.
// See \`scripts/ds-inventory.mjs\` and \`src/lib/ds/inventory.ts\`.

import type { ComponentEntry } from './inventory';

export const inventory: ComponentEntry[] = [${inventory.length === 0 ? '' : '\n'}`;

const body = inventory.map(entrySource).join(',\n');
const footer = `${inventory.length === 0 ? '' : '\n'}];\n`;

writeFileSync(outputPath, `${header}${body}${footer}`, 'utf8');
process.stdout.write(
  `ds:inventory wrote ${inventory.length} components to ${toRepositoryPath(outputPath)}\n`
);
