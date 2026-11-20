# 062 — Reconstrucción del contorno de placa (outline) para paridad con FlatCAM

Tres de las seis placas del corpus tienen un `outline` que no es un lazo cerrado: son segmentos sueltos
con separaciones de 0.9 a 23.4 mm. FlatCAM deriva de ellos un polígono cerrado y mide su área; este árbol
mide el área de los trazos sueltos. Por eso `kicad-2l-simple`, `kicad-4l-pours` y `substitute-eagle` no
pueden pasar E0.4 en su capa de contorno, y E1.4 no cierra.

Esta tarea existe separada de la 061 porque no es una corrección: es una capacidad nueva (derivar el
contorno de la placa a partir de trazos sueltos), con decisiones propias que la 061 no debía decidir de
paso.

## Surfaces

- `crates/cam-gerber/`
- `crates/cam-import/`
- `scripts/parity/`
- `fixtures/corpus/`
- `odd/roadmap/index.md`

## Specs

- **S1 (pedido original, verbatim).** «1» — en respuesta a la elección «Reconstrucción de contorno como
  tarea propia (fuera de 061): definir la regla y con qué evidencia. Es el camino si E1.4 tiene que
  cerrar de verdad contra FlatCAM.»
- **S2 (medición que motiva la tarea, `fixtures/baseline/*.json` vs `fixtures/corpus/*/outline.gbr`,
  verbatim de la corrida).** `kicad-2l-simple` FlatCAM 1210.5793 / árbol 10.2607; `kicad-4l-pours`
  16790.5017 / 35.3535; `substitute-eagle` 13410.6590 / 29.9195. En las tres el árbol produce 8, 8 y 20
  polígonos donde FlatCAM produce 2.
- **S3 (las tres que ya coinciden, verbatim).** `kicad-large-pours` 31703.1803 vs 31703.1803,
  `substitute-easyeda` 16114.8988 vs 16114.8987, `substitute-altium` 28800.2018 vs 28800.2016. En las
  tres, el hueco máximo entre segmentos del contorno es **0.0 mm**.
- **S4 (regla de medición vigente, `scripts/parity/README.md:27`, verbatim).** «`contour_area_mm2` is the
  **sum of the absolute contour areas** … a region with a hole contributes both boundaries, so a board
  outline reads as roughly twice the area it encloses.»
- **S5 (relación medida entre el oráculo y el bounding box, verbatim de la corrida).**
  FlatCAM/bbox = 2.000 en `kicad-large-pours`, `substitute-easyeda` y `substitute-altium`; 1.834, 1.984 y
  1.873 en las tres que divergen. El factor 2 es la regla de S4 con frontera exterior + interior.
- **S6 (tamaño real de los huecos, verbatim de la corrida).** `kicad-2l-simple`: máximo 11.5 mm, típicos
  0.9–1.77 mm, 12 segmentos. `kicad-4l-pours`: máximo 23.4229 mm, típicos 4.2426 mm, 8 segmentos.
  `substitute-eagle`: máximo 10.0 mm, típicos 3.0–8.2 mm, 38 segmentos.
- **S7 (límite que esta tarea no mueve).** No se retunea la tolerancia de área de la comparación
  (`1e-3` relativo, decisión de la 061) ni el límite de 30 s del arnés. La tarea arregla la geometría, no
  el criterio con el que se la juzga.
- **S8 (frontera con la 061).** La 061 mide y compara; esta tarea produce el contorno. Nada de lo que
  aquí se agregue cambia la semántica de las capas de cobre, máscara o silk.

## Tasks

- **T1** — Decidir la regla con evidencia: qué significa "el contorno de la placa" cuando los trazos
  están separados hasta 23 mm, y si un hueco es un corte de contorno (cutout) o un segmento faltante.
  La decisión se escribe con los números de S6, no con una intuición. Specs: S1, S2, S5, S6.
- **T2** — Implementar la derivación del contorno con esa regla, expuesta por rol y no por nombre de
  archivo. Specs: S1, S2, S3.
- **T3** — Medir las seis placas contra el oráculo con `scripts/parity/compare-flatcam.py` y demostrar
  que las tres de S2 pasan y las tres de S3 no se mueven. Specs: S2, S3, S4, S7.
- **T4** — Re-pinear los goldens de `outline` que la regla cambie y actualizar la fila E1.4 (y la de esta
  tarea) del roadmap con los números reales. Specs: S2, S3, S7.

## Log

- **L1 (pedido original, verbatim).** «1» — eligiendo, entre las tres opciones presentadas, la
  reconstrucción de contorno como tarea propia fuera de la 061.
- **L2 (por qué separada).** La 061 se propuso "que las seis placas midan"; al comparar contra el oráculo
  apareció que el `outline` de tres placas no es un lazo cerrado, y que cerrarlo no es una tolerancia de
  unión sino una reconstrucción (huecos de 0.9 a 23.4 mm, S6). Cambiar el alcance de la 061 para incluir
  una capacidad nueva habría mezclado una corrección con un feature.
- **L3 (estado de la 061 al abrir esta).** La 061 dejó el instrumento de comparación
  (`scripts/parity/compare-flatcam.py`, commit `275c5e1`) y la lista de divergencias reales; quedan allí
  las de silk/máscara, que sí son de su alcance.
- **L4 (evidencia de apertura).** Corrida de `scripts/parity/compare-flatcam.py` sobre el corpus el
  2026-11-18: 18 artefactos coinciden, 36 divergen; de las divergencias, 29 son artefactos que el árbol
  todavía no puede medir (`timeout`) y 6 son geometría real, de las cuales **3 son de outline** y son las
  que esta tarea toma. Las otras 3 (silk y máscara) quedan en la 061.
- **L5 (registro de entrega parcial, commit de referencia `957692c`).** Se registra el cierre **parcial
  por presupuesto** de 062 para entrega: el índice la marcará `delivered` con alcance parcial explícito
  (decisión del responsable). Hecho: tarea abierta con evidencia/regla motivadora (S2–S6) y comparación
  instrumentada disponible. Pendiente: T1–T4 (definir regla de contorno para huecos 0.9–23.4 mm, derivar
  contorno por rol, medir las 6 placas contra FlatCAM y re-pinear outlines/roadmap). `Risk: none`
  (registro documental).
