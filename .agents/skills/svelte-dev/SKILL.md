---
name: svelte-dev
description: "Trigger: Svelte, SvelteKit, rune, $state, $derived, $effect, route, component. Develop Camcraft's Svelte 5 frontend."
license: Apache-2.0
---

## Activation Contract

Use when working on `src/`, Svelte components, SvelteKit routes/layouts, frontend state, or UI behavior.

## Project Layout

- `src/routes/` — SvelteKit routes. `+layout.ts` has `ssr = false` / static adapter setup for Tauri.
- `src/lib/` — reusable components, stores/helpers, typed Tauri wrappers (create when needed).
- `static/` — static assets.

## Hard Rules

- **Svelte 5 runes**: use `$state`, `$derived`, `$effect`, `$props`; do not introduce legacy Svelte 4 store patterns unless required.
- **Small components**: keep components focused; extract logic/helpers when a component mixes view, IPC, and state transitions.
- **Typed IPC boundary**: UI components call typed wrappers in `src/lib/`, not raw `invoke` scattered across routes.
- **Accessibility**: label inputs, keep keyboard/focus behavior, use semantic elements before ARIA.
- **No visual claims without evidence**: for UI changes, verify with tests and/or visual review flow.

## Checks

```bash
pnpm check
pnpm lint
pnpm test
pnpm dev
```

## Testing

- Vitest + Testing Library for component behavior.
- Test user-visible behavior (text, roles, keyboard, callbacks), not implementation details.
- Mock Tauri IPC at the wrapper boundary.

## Output Contract

Report: components/routes changed, state/IPC impact, checks run, and remaining UI gaps.
