# 070 — camui-floating-studio

Rediseño del shell de Camcraft en egui según la dirección `design/camcraft-workbench-v2/02-floating-studio.html`: top bar compacta con command search, canvas dominante, paneles flotantes (Browser, Inspector, Operaciones) y status bar sobria, con lenguaje visual de familia SolidCraft adaptado a Camcraft.

## Surfaces

- `crates/cam-ui/src/shell/`
- `crates/cam-ui/src/chrome/`
- `crates/cam-ui/src/theme.rs`
- `crates/cam-ui/src/start_screen.rs`
- `design/camcraft-workbench-v2/02-floating-studio.html`
- `design/system.md`

## Specs

- **S1 (pedido original, verbatim).** «Implement the design artboard design/camcraft-workbench-v2/02-floating-studio.html
  Artboard: design/camcraft-workbench-v2/02-floating-studio.html
  Title: Floating Studio
  Tradeoff: Máxima presencia del canvas y la sensación más cercana a SolidCraft con paneles flotantes. Requiere disciplina de layout para no tapar el trabajo ni generar ruido.

  Implement it in the project's real stack. Use the artboard as the visual spec: map its parts to the project's existing components and styles, and do not copy the artboard HTML verbatim.
  Take colors, typography, spacing, radius and shadows from design/system.md, the project's token source.

  Report progress with the gentle_design tool: action "handoff", target "camcraft-workbench-v2/02-floating-studio.html", status "in_progress" when you start; status "implemented" when it is done, with previewUrl set to the running app's loopback URL (localhost, 127.0.0.1 or [::1]) when one is running, so the canvas can compare it with the artboard.
  This design is not linked to an ODD feature document: size it under the project's ODD rules as usual.
  This handoff itself commits nothing.»
- **S2 (dirección de diseño, verbatim).** «necesitamos volver a disenar la ui por completo, tenemos que basarnos en los layouts y estilos de solidcraft https://github.com/tangency-labs/solidcraft»
- **S3 (decisiones del usuario, verbatim).** «1. el shell entero, porque no me convence nada 2. ambos, para que se siente de la familia de Solidcraft. 3. arrancamos con las direcciones de diseno 4. hay que replantear la arquitectura de la informacion»
- **S4 (design system, verbatim).** «el design system de la ui no lo tocaste? porque no se parece en nada al de solidcraft» — resuelto actualizando `design/system.md` y `design/system.html` a familia SolidCraft adaptada a Camcraft.
- **S5 (implementación, verbatim).** «pues de momento tienes de sobra para implementar en egui. los demas settings los iremos ajustando mas adelante»

## Tasks

| ID  | Tarea                                                                                                                                                                              | Specs    | Ruta   | Commit |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ------ | ------ |
| T1  | Mapear el shell actual y definir el layout Floating Studio en egui: top bar, canvas dominante, paneles flotantes, status bar, preservando UiAction/estado                          | S1,S2,S3 | inline | —      |
| T2  | Implementar top bar estilo SolidCraft (marca, documento, command search central, acciones de layout/tema/config) y quitar dependencia del layout docked anterior donde corresponda | S1,S3,S4 | inline | —      |
| T3  | Implementar paneles flotantes Browser/Inspector/Operaciones con jerarquía de bordes primary/secondary/tertiary y comportamiento dinámico de altura                                 | S1,S3,S4 | inline | —      |
| T4  | Adaptar start screen y status bar al nuevo shell, manteniendo flujos existentes (New/Open/recents/save/status)                                                                     | S1,S3    | inline | —      |
| T5  | Verificación focalizada (fmt/clippy/tests de cam-ui afectados), actualización de handoff gentle_design y notas de pendientes                                                       | S1,S5    | inline | —      |

## Log

- **L1 (pedido original, verbatim).** «Implement the design artboard design/camcraft-workbench-v2/02-floating-studio.html ... Implement it in the project's real stack. ...»
- **L2 (contexto).** Se crearon direcciones de diseño en `design/camcraft-workbench-v2/`, se actualizó `design/system.md`/`design/system.html` a familia SolidCraft y se refinó `02-floating-studio.html` con comentarios de canvas. Se inicia implementación egui en `cam-ui`.
- **L3 (primera pasada egui).** Layout Floating Studio en `shell/frame.rs`, top bar rediseñado en `chrome/app_bar.rs`, paneles flotantes browser/inspector en `shell/panels.rs`, paleta flotante en `chrome/palette.rs`. `cargo test -p cam-ui --lib` 34/0.
- **L4 (fix click-through + tests).** Worker: patrón de hit-test para paneles flotantes sin bloquear canvas; warm-up pass removido de `pass` (was corrupting one-shot reports); adaptaciones de tests por layout obsoleto. `--test input` 23/0, `--test shell` 138/0.
- **L5 (cierre final).** Todos los suites verdes: `cargo test -p cam-ui --lib` 34/0, `--test input` 23/0, `--test shell` 138/0, `--test tool_library` 55/0, `--test palette` 4/0. No commits. Handoff `gentle_design` de `camcraft-workbench-v2/02-floating-studio.html`: status="implemented" con previewUrl="http://127.0.0.1:46113/UDgxs4-PJL9CCnMl_-UAxyp1xMeErJHK/d/camcraft-workbench-v2/". El diseño ha sido mapeado al stack egui/Rust con fidelidad al artboard 02-floating-studio.html.
