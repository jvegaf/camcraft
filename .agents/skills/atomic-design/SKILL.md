---

name: atomic-design
description: "Trigger: atomic design, atoms, molecules, organisms, templates, pages, component structure, UI decomposition. Build Camcraft UI with Atomic Design methodology."
---

## Activation Contract

Use when creating or refactoring UI components in Camcraft (Svelte 5 + Panda CSS + Tauri).

## Hard Rules

- Classify every UI piece before creating it: **Atom, Molecule, Organism, Template, Page**.
- Atoms have no business logic and no app state imports; they receive props/snippets.
- Molecules compose atoms; no direct Tauri/API calls.
- Organisms compose molecules/atoms and may own local UI state or read shared stores.
- Templates define layout/slots; Pages wire routes, data loading, and Tauri commands.
- Do not duplicate an existing component; extend or compose it.
- Keep styles in Panda CSS (`css()`, recipes, patterns); no ad-hoc global CSS for component internals.
- Match DS references in `design/` before inventing new visual patterns.

## Decision Gates

| Situation                                                    | Action                                       |
| ------------------------------------------------------------ | -------------------------------------------- |
| Single visual primitive (button, icon, input, badge)         | Atom in `src/lib/components/atoms/`          |
| Small reusable combo (field + label + hint, search box)      | Molecule in `src/lib/components/molecules/`  |
| Feature block with state (panel, dialog, toolbar, card list) | Organism in `src/lib/components/organisms/`  |
| Layout skeleton with slots                                   | Template in `src/lib/components/templates/`  |
| Route screen wiring data/actions                             | Page in `src/routes/`                        |
| Existing component fits                                      | Reuse/extend; do not create parallel variant |

## Execution Steps

1. Identify the DS reference (`design/camcraft-workbench-v2/*.html`) and the user flow.
2. Decompose UI into atoms → molecules → organisms; list props/events/state per piece.
3. Check existing components under `src/lib/components/` before creating new files.
4. Implement smallest units first; compose upward. Keep Tauri invokes in pages/organism controllers, not atoms.
5. Verify with `pnpm check` and visual smoke when UI changes.

## Output Contract

- Component classification (atom/molecule/organism/template/page).
- Files created/modified.
- DS reference used.
- Verification run (`pnpm check` and/or smoke note).
