# 108 — CAM command surface: contexto del documento, operaciones y estado de simulación/export

Publicar por Tauri los tres comandos de backend que hoy bloquean la UI CAM ya implementada: el contexto del documento (capas, material y medidas del stock, origen del setup), las operaciones con su detalle y el estado de simulación y export/post. Es lo único que sobrevive del pipeline board retirado; el chrome y el inspector base siguen siendo [095-floating-studio-atomic.md](095-floating-studio-atomic.md) y la librería de herramientas es [107-settings-tool-library-ui.md](107-settings-tool-library-ui.md).
Estado: `open` — documento de planificación; nada de este alcance está implementado y `## Tasks` no tiene commits.

## Surfaces

- `src-tauri/src/lib.rs` (tres comandos nuevos y sus tests; hoy `invoke_handler` en `:496-506` no tiene ninguno de ellos)
- `src/lib/components/workbench/workbench.svelte.ts` (consumidores: `setOperations`, `:167`)
- `src/lib/components/organisms/CamObjectBrowser.svelte`, `src/lib/components/organisms/OperationInspector.svelte` (consumidores ya implementados)
- `odd/tasks/108-cam-command-surface.md`, `odd/tasks/INDEX.md`

## Specs

- **S1 (alcance heredado, verbatim de `odd/tasks/106-pipeline-board-ui.md:40-42`).** «Backend: comando que expone el contexto CAM del documento — nombres de capa, material y medidas del stock, origen del setup — serializando lo que hoy `canonical_job()` fija sin publicar»; «Backend: comando de operaciones con nombre, tipo, herramienta, Z cut, feed plunge, spindle, hits, herramientas y tiempo, para llenar el contexto de etapa y el inspector sin valores de muestra»; «Backend: comando que reporta el estado de simulación y de export/post, que es lo que hace verificables los pasos 4 y 5 del progreso y la etapa `Exportar`». Este documento restata esas tres tareas como S2, S3 y S4 y no agrega una cuarta capacidad.
- **S2 (contexto CAM del documento).** Un comando que expone nombres de capa, material y medidas del stock y origen del setup, serializando lo que hoy `canonical_job()` fija sin publicar (`src-tauri/src/lib.rs:372-380`: `OriginCorner::BottomLeft` en `:377` y `BoardSideUp::Top` en `:378`). Hoy `DocumentStatusDto` (`src-tauri/src/lib.rs:146-157`) sólo lleva `layerCount`/`toolpathCount` y las medidas del board; no hay nombres de capa, material ni origen.
- **S3 (operaciones).** Un comando que entrega las operaciones con nombre, tipo, herramienta, Z cut, feed plunge, spindle, hits, herramientas y tiempo, para que el browser y el inspector muestren datos reales en vez de valores de muestra.
- **S4 (simulación y export/post).** Un comando que reporta el estado de simulación y de export/post. Sólo publica ese estado: no construye el simulador ni el post.
- **S5 (por qué este documento existe).** Los tres comandos desbloquean (i) el CAM browser y el operation inspector del floating studio ya implementado — `setOperations` en `src/lib/components/workbench/workbench.svelte.ts:167` no tiene ningún llamador de producción (sólo `workbench.svelte.test.ts` y `OperationInspector.test.ts`), `CamObjectBrowser.svelte:112-119` muestra un estado vacío explícito para `Setup` y `:124-152` sólo puede mostrar el conteo de operaciones — y (ii) la página de librería de herramientas planificada en [107-settings-tool-library-ui.md](107-settings-tool-library-ui.md), que necesita el mismo contexto de documento para su master-detail. El backend real de hoy son `DocumentStatusDto` (`src-tauri/src/lib.rs:146-157`), `canonical_job()` con origen y side hardcodeados (`:372-380`) y `invoke_handler` (`:496-506`), que registra `document_status`, `new_document`, `open_document`, `save_document`, `list_recents`, `get_settings`, `update_settings` y `scene_snapshot`, ninguno de los tres comandos.
- **S6 (frontera con 107).** [107-settings-tool-library-ui.md](107-settings-tool-library-ui.md) declara únicamente sus comandos propios de librería de herramientas; no duplica S2-S4. Este documento no declara comandos de librería.
- **S7 (no regresión).** Los comandos existentes de `invoke_handler` (`src-tauri/src/lib.rs:496-506`) no cambian de contrato y `/`, `/workspace` y `/workbench` siguen funcionando.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                               | Specs          | Ruta   | Commit |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ------ | ------ |
| T1  | Comando que expone el contexto CAM del documento — nombres de capa, material y medidas del stock, origen del setup — serializando lo que hoy `canonical_job()` fija sin publicar, con su test de contrato en `src-tauri/src/lib.rs` | S1, S2, S5, S7 | worker |        |
| T2  | Comando de operaciones con nombre, tipo, herramienta, Z cut, feed plunge, spindle, hits, herramientas y tiempo, con su test de contrato en `src-tauri/src/lib.rs`                                                                   | S1, S3, S5, S7 | worker |        |
| T3  | Comando que reporta el estado de simulación y de export/post, con su test de contrato en `src-tauri/src/lib.rs`                                                                                                                     | S1, S4, S5, S7 | worker |        |

## Log

- **L1 (pedido original, verbatim).** «comprueba que falta de la implementacion de la ui».
- **L2 (decisiones del owner, verbatim).** «el artboard 01 eliminalo porque no lo vamos a implementar, nos quedamos con 02 floating»; «el pipeline board tampoco lo implementamos, tambien borralo»; «el bueno es 05». Las dos primeras retiran los artboards 01 y 03 —y con ellos los documentos 105 y 106—, de modo que este documento hereda sólo la superficie de comandos de 106. «el bueno es 05» fija el footer de preferencias y se registra en [107-settings-tool-library-ui.md](107-settings-tool-library-ui.md) L5, no acá.
- **L3 (evidencia medida al crear, 2026-11-27).** Leí `src-tauri/src/lib.rs`: `DocumentStatusDto` en `:146-157` (sólo conteos y medidas del board), `canonical_job()` en `:372-380` (`OriginCorner::BottomLeft` en `:377`, `BoardSideUp::Top` en `:378`, sin serializar), `invoke_handler` en `:496-506` (ocho comandos, ninguno de los tres). Leí los consumidores ya implementados: `setOperations` en `src/lib/components/workbench/workbench.svelte.ts:167` no tiene llamador de producción (sólo tests), `CamObjectBrowser.svelte:112-119` deja `Setup` vacío por falta de origen/material y `:124-152` sólo imprime el conteo de operaciones. Las dos decisiones sobre regla en milímetros y view cube de simulación no viven acá porque no son comandos de backend; quedan registradas en 107 L5.
