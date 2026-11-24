# 092 — Fase 4: settings overlay, select DS y skill Atomic Design

Mover settings dentro de la app como overlay, ajustar select/combo al DS, y crear skill de Atomic Design.

## Surfaces

- `src/lib/components/settings/SettingsDialog.svelte`
- `src/lib/components/workbench/workbench.svelte.ts`
- `src/routes/+layout.svelte`
- `src/routes/+page.svelte`
- `src/routes/settings/+page.svelte`
- `src/lib/components/workbench/WorkbenchShell.svelte`
- `.agents/skills/atomic-design/SKILL.md`
- `odd/tasks/092-tauri-fase4-settings-overlay-atomic.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «por un lado, te hago aclaracion, los settings deben ser dentro de la app, no que tenga vida propia, me explico, ahora cuando abro los settings desaparece la main window, quedandose solo el background y la ventana de settings.»
- **S2 (pedido original, verbatim).** «por otro lado, hay que arreglar los estilos del componente selector/combo porque no son los que hay en su referencia.»
- **S3 (pedido original, verbatim).** «y por ultimo, pero no menos importante, debemos desarrollar la ui mediante atomic desing, asi que debemos crear una skill con la metodologia para que los agentes desarrollen de esa manera.»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Settings como overlay dentro de la app (no ruta propia) | S1 | inline | |
| T2 | Ajustar estilos de select/combo a referencia DS | S2 | inline | |
| T3 | Crear skill de Atomic Design para agentes | S3 | inline | |
| T4 | Verificar, RDD, ODD/Engram, commit y push | S1, S2, S3 | inline | |

## Log

- **L1 (pedido original, verbatim).** «por un lado, te hago aclaracion, los settings deben ser dentro de la app, no que tenga vida propia, me explico, ahora cuando abro los settings desaparece la main window, quedandose solo el background y la ventana de settings.»
- **L2 (pedido original, verbatim).** «por otro lado, hay que arreglar los estilos del componente selector/combo porque no son los que hay en su referencia.»
- **L3 (pedido original, verbatim).** «y por ultimo, pero no menos importante, debemos desarrollar la ui mediante atomic desing, asi que debemos crear una skill con la metodologia para que los agentes desarrollen de esa manera.»
- **L4 (ejecución, 2026-11-24).** SettingsDialog como overlay en layout/store; /settings redirige abriendo overlay. Select con estilo DS custom. Skill `.agents/skills/atomic-design/SKILL.md` creada. `pnpm check` OK.
