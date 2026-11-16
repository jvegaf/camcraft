# 060 — E1.7 Informe de importación Gerber/Excellon

E1.7: que un archivo Gerber con un constructo inválido importe **el resto** y reporte lo salteado con
su ubicación, sin que una capa con un trazo roto se pierda entera — y sin aflojar el rechazo entero
donde una feature salteada sería una placa mal taladrada.

## Surfaces

- `crates/cam-import/src/report.rs`
- `crates/cam-gerber/`
- `crates/cam-excellon/`

## Specs

- **S1 (pedido original, verbatim).** «me parece bien ese orden que sugieres, hazlo»
- **S2 (política por formato, verbatim, aceptada por el pedido de S1).** «**Gerber**: importar el resto
  y reportar `Skipped { construct, reason, location }` es defendible — perdés un trazo de una capa, lo
  sabés, y sabés dónde. — **Excellon**: no. Una feature salteada es un agujero que no se taladra: la
  placa llega a la máquina incompleta y nada la detiene. Ahí se sigue rechazando el archivo entero, como
  hoy.»
- **S3 (ROADMAP_CAMCRAFT.md E1.7, verbatim).** «| E1.7 | Informe de importación Gerber/Excellon sobre el
  patrón existente | **S** | Una placa con un arco inválido importa el resto y reporta `Skipped` con
  ubicación |»
- **S4 (estado real del patrón, verbatim del código).** `crates/cam-import/src/report.rs:7` ya declara
  `Skipped`, `:22` `ImportReport`, `:36` `pub skipped: Vec<Skipped>`, `:70` `fn skip(...)` y `:96`
  `fn render()`. Lo que falta no es la maquinaria: es que `cam-gerber` y `cam-excellon` la usen.
- **S5 (precedente que hay que igualar, verbatim del código).** El importador de carpeta ya reporta en
  vez de esconder: `crates/cam-import/src/folder.rs:67` documenta que se acumula «geometry this reader
  could not represent faithfully», y `:105` avisa `"{path}: could not be read: {error}"` dejando la capa
  fuera de `imported`. E1.7 baja ese mismo trato del nivel de **capa** al nivel de **constructo** dentro
  de un archivo, añadiendo la ubicación.
- **S6 (decisión que esta tarea NO revierte).** En Excellon, una feature que nombra una herramienta
  ausente de la tabla del archivo sigue rechazando el archivo entero, con el mensaje
  `line N: tool N is not in this file's table`. Decisión tomada y aprobada en el review de la
  049 (`odd/tasks/049-w2c-folder-import.md`): no se saltean features, porque un taladro que no se hace
  no se nota al mirar la pieza. S2 la confirma por formato; no la discute.
- **S7 (aceptación de tres partes, verbatim).** «La aceptación tiene que probar tres cosas: el reporte
  nombra constructo, razón y ubicación; el caso feliz no cambia ni un polígono; y el archivo con
  herramienta indefinida **sigue** siendo rechazo entero.»

## Tasks

| ID | Tarea | Specs | Ruta | Estado | Commit |
| --- | --- | --- | --- | --- | --- |
| T1 | Inventariar cada `GerberError::Invalid` de `cam-gerber` y clasificarlo: constructo salteable (un trazo, un flash, un macro que no se puede barrer) o archivo irremediable (formato, unidades, puntero fuera de orden). El inventario queda en el Log | S2, S3 | inline | pending | — |
| T2 | Enhebrar `ImportReport` en el lector Gerber: los constructos salteables se reportan con `skip(construct, reason, location)` y el resto de la capa se evalúa; los irremediables siguen devolviendo `Err` | S2, S3, S4, S5 | inline | pending | — |
| T3 | Excellon: dejar el rechazo entero como está y **probar que no se aflojó** (una feature con herramienta indefinida sigue siendo `Err`), más el reporte de lo que sí se puede contar | S6 | inline | pending | — |
| T4 | Aceptación S7 completa: (a) test de un Gerber con un arco inválido que devuelve geometría parcial **y** un `Skipped` con constructo/razón/ubicación; (b) test de que una capa válida produce exactamente los mismos polígonos que antes; (c) test de que el Excellon con herramienta indefinida sigue siendo rechazo. Documentar el cierre y actualizar la fila E1.7 del audit | S3, S7 | inline | pending | — |

## Log

- **L1 (pedido original, verbatim).** «me parece bien ese orden que sugieres, hazlo»
- **L2 (contexto y origen).** Sale del refresco del audit de la ola 4 (`dev` `766d931`), donde E1.7 quedó
  como el único `⬜` de F1 sin dueño ni ola. El encargo concreto fue decidir su destino; la recomendación
  aceptada fue: hacerla **antes** de la ola que abre F3/E6, porque no toca el kernel ni los goldens y por
  eso no depende de nada de `waves.md` §7. Tamaño declarado por el ROADMAP: **S**.
- **L3 (por qué no es «conectar un struct»).** `ImportReport`/`Skipped` existen y no los usa nadie fuera
  de los importadores DXF/SVG (`grep -rn ImportReport crates/cam-gerber crates/cam-excellon` → 0). El
  trabajo real está en la política: hoy **todos** los sitios `GerberError::Invalid` de
  `crates/cam-gerber/src/lib.rs` (`:198`, `:250`, `:336`, `:342`, `:370`, `:377`, `:418`, …) rechazan el
  archivo entero. Clasificarlos (T1) es la tarea, no el cableado.
- **L4 (riesgo declarado).** `Risk: item 1` — geometría descartada en silencio es un fallo que no se ve en
  la pieza. Por eso S7 exige que el caso feliz quede idéntico y que el rechazo de Excellon siga siendo
  rechazo: la tarea agrega visibilidad, no tolerancia.
