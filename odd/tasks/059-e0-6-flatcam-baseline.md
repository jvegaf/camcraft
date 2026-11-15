# 059 — Baseline de FlatCAM sobre el corpus (E0.6)

Instrumento E0.6: ejecutar FlatCAM-Plus sobre las 6 placas del corpus en un entorno Python separado, medir tiempo y movimientos por placa y por operación, y guardar los números como goldens versionados reproducibles desde un script.

## Surfaces

- `scripts/baseline/`
- `fixtures/baseline/`
- `odd/reports/flatcam-baseline.md`
- `odd/tasks/059-e0-6-flatcam-baseline.md`
- `odd/roadmap/index.md`
- `justfile`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa EO.6, para ello tienes el repo en ~/Code/FlatCAM-Plus»
- **S2 (ROADMAP_CAMCRAFT.md:70, verbatim).** «| E0.6 | Baseline de performance de FlatCAM ejecutando el corpus (entorno Python separado; los números se guardan como goldens versionados) | **M** | Tabla de tiempos y conteo de movimientos por placa y por operación, reproducible desde un script |»
- **S3 (SYSTEM_DESIGN_CAMCRAFT.md:712-713, verbatim).** «4. **Medición desde el día uno**: `criterion` + baseline versionado + CI que falla ante regresión + baseline de FlatCAM ejecutado en entorno Python separado (los números son goldens versionados).»
- **S4 (SYSTEM_DESIGN_CAMCRAFT.md:697-699, verbatim).** «Cada una con `top/bottom/mask/silk/outline/drill` y su baseline de FlatCAM como golden. El corpus es **el instrumento de aceptación de todo el port**, y es lo primero que se crea (C0).»
- **S5 (ROADMAP_CAMCRAFT.md:103-105, verbatim).** «**Nota de tolerancia de arcos.** FlatCAM aplana todo al pasar por shapely; nosotros preservamos arcos. La comparación es de área y cobertura, nunca de conteo de vértices con FlatCAM. El aplanado propio es explícito y controlado por operación (E0.3).»
- **S6 (ROADMAP_CAMCRAFT.md:151-152, verbatim).** «**Nada de esto se escribe antes de que `E0.6`+`E0.7` estén en verde**, porque sin baseline la promesa no es medible.»
- **S7 (odd/roadmap/index.md:30, verbatim).** «| E0.6 FlatCAM baseline | ⬜ | No baseline table or script under `scripts/`, `odd/reports/`, `crates/` | Nothing measured to compare against |»

## Tasks

| ID | Tarea | Specs | Ruta | Estado | Commit |
| --- | --- | --- | --- | --- | --- |
| T1 | Explorar el entorno FlatCAM-Plus (arranque headless, superficie Tcl, objetos, `gcode_parsed`) y el arnés de paridad existente | S1-S4 | inline | done | — |
| T2 | Escribir `scripts/baseline/`: medidor por artefacto y por operación, colector, comparador, `run.sh` y README | S2, S3 | inline | pending | — |
| T3 | Medir las 6 placas (54 artefactos, operaciones `isolation` y `drill`) y fijar los goldens en `fixtures/baseline/` | S2, S4 | inline | pending | — |
| T4 | Escribir `odd/reports/flatcam-baseline.md`: tabla de tiempos y movimientos por placa y por operación, entorno fijado, límites y cómo reproducir | S2, S5, S6 | inline | pending | — |
| T5 | Cerrar el registro: receta `just baseline`, fila E0.6 del roadmap, índice y memoria Engram | S1, S7 | inline | pending | — |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa EO.6, para ello tienes el repo en ~/Code/FlatCAM-Plus»
- **L2 (contexto y entorno).** Worktree `camcraft/059-e0-6-flatcam-baseline` (`~/.herdr/worktrees/camcraft/059-e0-6-flatcam-baseline`, rama `feat/059-flatcam-baseline`), superficies reclamadas con `just claim 059`. El oráculo es `~/Code/FlatCAM-Plus` (FlatCAM-Plus 1.3.6) con su propio `.venv` (Python 3.14.4, PyQt6 6.11.0, shapely 2.2.0). Lo que se puede hacer sin GUI, verificado a mano antes de escribir nada: `QT_QPA_PLATFORM=offscreen` + `App(qapp=...)` arranca en ~5,7 s; el `App` construye un `multiprocessing.Pool` en el constructor, así que el script necesita guarda `if __name__ == "__main__":`; abrir un proyecto es `ui.activate_project_workspace()`, y toda la cadena de operaciones existe en el shell Tcl (`open_gerber`, `isolate`, `cncjob`, `open_excellon`, `drillcncjob`), que es la superficie que FlatCAM documenta para scripts. Los movimientos no se cuentan en el texto del G-code: `CNCjob.gcode_parsed` es la lista de trayectorias con `kind` (`T`/`C` × `F`/`S`), así que el conteo es una propiedad del objeto, no del dialecto del post-procesador. Sin `-toolchangez` FlatCAM perfora todo con la primera herramienta («use the first tool in the selection as the tool that we are going to use», `ToolDrilling.py`), así que la operación de perforación se mide con toolchange activo.
- **L3 (decisiones de instrumento).** (a) Un proceso por artefacto y por operación, igual que el arnés de paridad, para que un cuelgue quede acotado por `timeout` en vez de colgar la corrida; el costo de arranque se mide y se informa por separado, nunca se suma al tiempo de la operación. (b) `HOME`/`XDG_CONFIG_HOME` van a un directorio temporal, así cada corrida ve las opciones de fábrica de FlatCAM y no la configuración de la máquina de quien la corre. (c) Se apaga el chequeo de actualizaciones de arranque: el instrumento no llama a la red. (d) Se registran polígonos, área de contorno y vértices, pero **el conteo de vértices no se compara nunca con `points` de nuestro arnés** (S5: FlatCAM aplana todo en shapely y nosotros preservamos arcos); el área sí es comparable, con tolerancia relativa laxa, y el reporte la muestra como referencia, no como veredicto. (e) Los tiempos son acoplados al hardware y a la carga, como el `bench` de E0.7: se registran y se comparan sólo como aviso, nunca como fallo. (f) Operaciones medidas: `isolation` (capa `top`, `dia=0.2`, `passes=2`, `overlap=0.15`, `iso_type=full`) y `drill` (archivo `drill`, con toolchange). Non-copper clear queda fuera de esta tarea: es la operación de F4 y su costo en las placas con pours no está acotado todavía.
- **L4 (T1, exploración cerrada).** Spikes ejecutados sobre `substitute-easyeda`: `open_gerber` 1,6 s (165 polígonos), `isolate -passes 2` 0,6 s (dos objetos, 165 + 147 anillos), `cncjob` 0,5 s (331 movimientos, 2300,60 mm de trayectoria), `open_excellon` 0,2 s (7 herramientas, 165 features), `drillcncjob` con toolchange 0,2 s (338 movimientos repartidos en las 7 herramientas).
