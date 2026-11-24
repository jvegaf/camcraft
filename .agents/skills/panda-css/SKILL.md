---

name: panda-css
description: "Trigger: Panda CSS, styled-system, recipe, pattern, token, design system styles. Style Camcraft UI with Panda CSS."
license: Apache-2.0
---

## Activation Contract

Use when changing styles, tokens, recipes, patterns, or Panda CSS config in the Camcraft frontend.

## Project Layout

- `panda.config.ts` — Panda config, presets, theme extensions, include paths.
- `styled-system/` — generated output; do not hand-edit.
- `src/` — components consuming Panda `css()`, recipes, patterns.

## Hard Rules

- **Tokens first**: use design tokens/theme values before ad-hoc CSS values; map to `design/system.md` where applicable.
- **Codegen aware**: after config/token/recipe changes, run `pnpm codegen` (or `pnpm prepare` if wired) and verify generated output is current.
- **No generated-file edits**: never edit `styled-system/` by hand.
- **Consistent recipes**: prefer recipes/patterns for repeated UI primitives (buttons, panels, inputs) instead of copying style objects.
- **Dark/light parity**: when touching theme tokens or component styles, check both themes.

## Checks

```bash
pnpm codegen
pnpm check
pnpm lint
```

## Output Contract

Report: tokens/recipes/components changed, codegen run, visual/theme impact, and any follow-up needed.
